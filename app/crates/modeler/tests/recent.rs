// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-ui-menus: the recent files list and where it is kept. See docs/specs/m4-ui-menus.md.

use modeler::recent::{MAX_RECENT, RecentFiles, config_dir, recent_file};
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + use<> {
    let map: HashMap<String, OsString> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), OsString::from(*v)))
        .collect();
    move |name| map.get(name).cloned()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "modeler-recent-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn abs(path: &str) -> PathBuf {
    std::path::absolute(path).unwrap()
}

#[test]
fn new_list_is_empty() {
    assert!(RecentFiles::default().paths().is_empty());
}

#[test]
fn add_puts_the_newest_first() {
    let mut list = RecentFiles::default();
    list.add(Path::new("/m/a.modeler"));
    list.add(Path::new("/m/b.dae"));
    list.add(Path::new("/m/c.modeler"));
    assert_eq!(
        list.paths(),
        [
            PathBuf::from("/m/c.modeler"),
            PathBuf::from("/m/b.dae"),
            PathBuf::from("/m/a.modeler"),
        ]
    );
}

#[test]
fn adding_again_moves_to_the_front_without_a_duplicate() {
    let mut list = RecentFiles::default();
    list.add(Path::new("/m/a.modeler"));
    list.add(Path::new("/m/b.dae"));
    list.add(Path::new("/m/a.modeler"));
    assert_eq!(
        list.paths(),
        [PathBuf::from("/m/a.modeler"), PathBuf::from("/m/b.dae")]
    );
}

#[test]
fn add_makes_relative_paths_absolute() {
    let mut list = RecentFiles::default();
    list.add(Path::new("models/desk.dae"));
    assert_eq!(list.paths(), [abs("models/desk.dae")]);
    assert!(list.paths()[0].is_absolute());
    // The same file given absolutely is the same entry.
    list.add(&abs("models/desk.dae"));
    assert_eq!(list.paths().len(), 1);
}

#[test]
fn list_keeps_at_most_max_recent_dropping_the_oldest() {
    assert_eq!(MAX_RECENT, 8);
    let mut list = RecentFiles::default();
    for i in 0..MAX_RECENT + 3 {
        list.add(&PathBuf::from(format!("/m/{i}.modeler")));
    }
    assert_eq!(list.paths().len(), MAX_RECENT);
    assert_eq!(
        list.paths()[0],
        PathBuf::from(format!("/m/{}.modeler", MAX_RECENT + 2))
    );
    assert_eq!(list.paths()[MAX_RECENT - 1], PathBuf::from("/m/3.modeler"));
}

#[test]
fn remove_drops_only_that_entry() {
    let mut list = RecentFiles::default();
    list.add(Path::new("/m/a.modeler"));
    list.add(Path::new("/m/b.dae"));
    list.add(Path::new("/m/c.modeler"));
    list.remove(Path::new("/m/b.dae"));
    assert_eq!(
        list.paths(),
        [PathBuf::from("/m/c.modeler"), PathBuf::from("/m/a.modeler")]
    );
    list.remove(Path::new("/m/missing.dae"));
    assert_eq!(list.paths().len(), 2);
}

#[test]
fn remove_matches_a_relative_path_to_its_absolute_entry() {
    let mut list = RecentFiles::default();
    list.add(Path::new("models/desk.dae"));
    list.remove(Path::new("models/desk.dae"));
    assert!(list.paths().is_empty());
}

#[test]
fn parse_reads_one_path_per_line() {
    let list = RecentFiles::parse("/m/a.modeler\n/m/b.dae\r\n\n/m/c.modeler");
    assert_eq!(
        list.paths(),
        [
            PathBuf::from("/m/a.modeler"),
            PathBuf::from("/m/b.dae"),
            PathBuf::from("/m/c.modeler"),
        ]
    );
}

#[test]
fn parse_skips_duplicates_and_keeps_max_recent() {
    let mut text = String::from("/m/a.modeler\n/m/a.modeler\n");
    for i in 0..20 {
        text.push_str(&format!("/m/{i}.dae\n"));
    }
    let list = RecentFiles::parse(&text);
    assert_eq!(list.paths().len(), MAX_RECENT);
    assert_eq!(list.paths()[0], PathBuf::from("/m/a.modeler"));
    assert_eq!(list.paths()[1], PathBuf::from("/m/0.dae"));
    assert_eq!(list.paths()[MAX_RECENT - 1], PathBuf::from("/m/6.dae"));
}

#[test]
fn parse_of_empty_text_is_empty() {
    assert!(RecentFiles::parse("").paths().is_empty());
    assert!(RecentFiles::parse("\n\r\n\n").paths().is_empty());
}

#[test]
fn to_text_writes_one_line_per_path_and_parses_back() {
    let mut list = RecentFiles::default();
    list.add(Path::new("/m/a.modeler"));
    list.add(Path::new("/m/with space/b.dae"));
    assert_eq!(list.to_text(), "/m/with space/b.dae\n/m/a.modeler\n");
    assert_eq!(RecentFiles::parse(&list.to_text()), list);
    assert_eq!(RecentFiles::default().to_text(), "");
}

#[test]
fn save_then_load_round_trips_and_creates_folders() {
    let dir = scratch("roundtrip");
    let file = dir.join("deep").join("er").join("recent-files");
    let mut list = RecentFiles::default();
    list.add(Path::new("/m/a.modeler"));
    list.add(Path::new("/m/b.dae"));
    list.save(&file).unwrap();
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "/m/b.dae\n/m/a.modeler\n"
    );
    assert_eq!(RecentFiles::load(&file), list);
    // Saving again replaces the file and leaves no temporary files behind.
    list.remove(Path::new("/m/b.dae"));
    list.save(&file).unwrap();
    assert_eq!(RecentFiles::load(&file), list);
    let names: Vec<_> = std::fs::read_dir(file.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, [OsString::from("recent-files")]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_of_a_missing_or_bad_file_is_empty() {
    let dir = scratch("bad");
    assert!(RecentFiles::load(&dir.join("nope")).paths().is_empty());
    let bad = dir.join("binary");
    std::fs::write(&bad, [0xff, 0xfe, 0x00, 0x41]).unwrap();
    assert!(RecentFiles::load(&bad).paths().is_empty());
    // A folder can't be read as a file.
    assert!(RecentFiles::load(&dir).paths().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_reports_errors() {
    let dir = scratch("err");
    let blocker = dir.join("file");
    std::fs::write(&blocker, "x").unwrap();
    // The folder can't be created under a plain file.
    let list = RecentFiles::parse("/m/a.dae");
    assert!(list.save(&blocker.join("recent-files")).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn config_dir_override_wins() {
    let var = env(&[
        ("DRYDOCK_CONFIG_DIR", "/tmp/cfg"),
        ("XDG_CONFIG_HOME", "/x"),
        ("HOME", "/home/u"),
        ("APPDATA", "C:\\Users\\u\\AppData\\Roaming"),
    ]);
    assert_eq!(config_dir(var), Some(PathBuf::from("/tmp/cfg")));
}

#[test]
fn empty_override_is_ignored() {
    let var = env(&[("DRYDOCK_CONFIG_DIR", ""), ("HOME", "/home/u")]);
    let dir = config_dir(var).unwrap();
    assert_ne!(dir, PathBuf::from(""));
}

#[cfg(not(windows))]
#[test]
fn the_old_override_is_not_read() {
    let var = env(&[("MODELER_CONFIG_DIR", "/tmp/old"), ("HOME", "/home/u")]);
    assert_eq!(
        config_dir(var),
        Some(PathBuf::from("/home/u/.config/drydock"))
    );
    assert_eq!(config_dir(env(&[("MODELER_CONFIG_DIR", "/tmp/old")])), None);
    assert_eq!(
        recent_file(env(&[("MODELER_CONFIG_DIR", "/tmp/old"), ("HOME", "/h")])),
        Some(PathBuf::from("/h/.config/drydock/recent-files"))
    );
}

#[cfg(not(windows))]
#[test]
fn config_dir_uses_xdg_then_home() {
    assert_eq!(
        config_dir(env(&[("XDG_CONFIG_HOME", "/x/cfg"), ("HOME", "/home/u")])),
        Some(PathBuf::from("/x/cfg/drydock"))
    );
    assert_eq!(
        config_dir(env(&[("HOME", "/home/u")])),
        Some(PathBuf::from("/home/u/.config/drydock"))
    );
    // Empty or relative XDG_CONFIG_HOME falls back to HOME.
    assert_eq!(
        config_dir(env(&[("XDG_CONFIG_HOME", ""), ("HOME", "/home/u")])),
        Some(PathBuf::from("/home/u/.config/drydock"))
    );
    assert_eq!(
        config_dir(env(&[("XDG_CONFIG_HOME", "rel/cfg"), ("HOME", "/home/u")])),
        Some(PathBuf::from("/home/u/.config/drydock"))
    );
    assert_eq!(config_dir(env(&[("HOME", "")])), None);
    assert_eq!(config_dir(env(&[])), None);
    // APPDATA is a Windows variable.
    assert_eq!(config_dir(env(&[("APPDATA", "/appdata")])), None);
}

#[cfg(windows)]
#[test]
fn config_dir_uses_appdata_on_windows() {
    assert_eq!(
        config_dir(env(&[("APPDATA", "C:\\Users\\u\\AppData\\Roaming")])),
        Some(PathBuf::from("C:\\Users\\u\\AppData\\Roaming").join("Drydock"))
    );
}

#[test]
fn recent_file_is_in_the_config_dir() {
    assert_eq!(
        recent_file(env(&[("DRYDOCK_CONFIG_DIR", "/tmp/cfg")])),
        Some(PathBuf::from("/tmp/cfg/recent-files"))
    );
    assert_eq!(recent_file(env(&[])), None);
}
