// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m9-linux-desktop-entry.md`. All on a temp dir.

use std::fs;
use std::path::{Path, PathBuf};

use drydock::desktop_entry::{
    ICON_SIZES, data_dir_from, desktop_entry_text, exec_line, files_for, install,
};
use drydock::window_icon::{APP_ID, ICON_PNG};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("drydock-de-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn exe() -> PathBuf {
    PathBuf::from("/opt/drydock/bin/drydock")
}

#[test]
fn data_dir_prefers_absolute_xdg() {
    assert_eq!(
        data_dir_from(Some("/x/data"), Some("/home/u")),
        Some(PathBuf::from("/x/data"))
    );
}

#[test]
fn data_dir_falls_back_to_home() {
    let want = Some(PathBuf::from("/home/u/.local/share"));
    assert_eq!(data_dir_from(None, Some("/home/u")), want);
    assert_eq!(data_dir_from(Some(""), Some("/home/u")), want);
    assert_eq!(data_dir_from(Some("relative/dir"), Some("/home/u")), want);
}

#[test]
fn data_dir_none_without_home() {
    assert_eq!(data_dir_from(None, None), None);
    assert_eq!(data_dir_from(Some(""), Some("")), None);
}

#[test]
fn exec_plain_path_is_quoted_with_file_code() {
    assert_eq!(
        exec_line(Path::new("/opt/drydock/bin/drydock")),
        "\"/opt/drydock/bin/drydock\" %f"
    );
}

#[test]
fn exec_path_with_spaces_stays_one_argument() {
    assert_eq!(
        exec_line(Path::new("/home/a b/drydock")),
        "\"/home/a b/drydock\" %f"
    );
}

#[test]
fn exec_escapes_quote_dollar_backtick_backslash_and_percent() {
    // Quoting rule: \" \` \$ \\ inside quotes; then the file's string rule doubles each
    // backslash; a literal % becomes %%.
    assert_eq!(exec_line(Path::new("/x/a\"b")), "\"/x/a\\\\\"b\" %f");
    assert_eq!(exec_line(Path::new("/x/a$b")), "\"/x/a\\\\$b\" %f");
    assert_eq!(exec_line(Path::new("/x/a`b")), "\"/x/a\\\\`b\" %f");
    assert_eq!(exec_line(Path::new("/x/a\\b")), "\"/x/a\\\\\\\\b\" %f");
    assert_eq!(exec_line(Path::new("/x/100%/d")), "\"/x/100%%/d\" %f");
}

#[test]
fn entry_has_required_keys() {
    let text = desktop_entry_text(&exe());
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "[Desktop Entry]");
    assert!(text.ends_with('\n'));
    for want in [
        "Type=Application".to_owned(),
        "Name=Drydock".to_owned(),
        format!("Exec={}", exec_line(&exe())),
        format!("Icon={APP_ID}"),
        "Terminal=false".to_owned(),
        "Categories=Graphics;3DGraphics;".to_owned(),
        format!("StartupWMClass={APP_ID}"),
    ] {
        assert!(
            lines.contains(&want.as_str()),
            "missing {want:?} in\n{text}"
        );
    }
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("Comment=") && l.len() > 8)
    );
}

#[test]
fn entry_changes_with_exe() {
    assert_ne!(
        desktop_entry_text(&exe()),
        desktop_entry_text(Path::new("/other/drydock"))
    );
}

#[test]
fn file_list_names_and_order() {
    let dir = Path::new("/data");
    let files = files_for(dir, &exe());
    let paths: Vec<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    let mut want = vec![dir.join(format!("applications/{APP_ID}.desktop"))];
    for n in ICON_SIZES {
        want.push(dir.join(format!("icons/hicolor/{n}x{n}/apps/{APP_ID}.png")));
    }
    assert_eq!(paths, want);
    assert_eq!(ICON_SIZES, [48, 128, 256]);
    assert_eq!(files[0].bytes, desktop_entry_text(&exe()).into_bytes());
}

#[test]
fn icons_are_png_of_the_named_size_and_256_is_the_embedded_file() {
    let files = files_for(Path::new("/data"), &exe());
    for (file, n) in files[1..].iter().zip(ICON_SIZES) {
        let img =
            image::load_from_memory_with_format(&file.bytes, image::ImageFormat::Png).expect("png");
        assert_eq!((img.width(), img.height()), (n, n));
    }
    assert_eq!(files[3].bytes, ICON_PNG);
}

#[test]
fn first_install_writes_everything() {
    let dir = temp("first");
    let report = install(&dir, &exe());
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    let files = files_for(&dir, &exe());
    assert_eq!(
        report.written,
        files.iter().map(|f| f.path.clone()).collect::<Vec<_>>()
    );
    for f in &files {
        assert_eq!(fs::read(&f.path).unwrap(), f.bytes);
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn second_install_writes_nothing() {
    let dir = temp("second");
    install(&dir, &exe());
    let entry = dir.join(format!("applications/{APP_ID}.desktop"));
    let before = fs::metadata(&entry).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let report = install(&dir, &exe());
    assert!(
        report.written.is_empty() && report.failed.is_empty(),
        "{report:?}"
    );
    assert_eq!(fs::metadata(&entry).unwrap().modified().unwrap(), before);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn moved_binary_rewrites_only_the_entry() {
    let dir = temp("moved");
    install(&dir, &exe());
    let other = PathBuf::from("/home/u/drydock/target/release/drydock");
    let report = install(&dir, &other);
    assert_eq!(
        report.written,
        vec![dir.join(format!("applications/{APP_ID}.desktop"))]
    );
    let text = fs::read_to_string(&report.written[0]).unwrap();
    assert!(text.contains(&exec_line(&other)));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn changed_or_deleted_icon_is_restored() {
    let dir = temp("icon");
    install(&dir, &exe());
    let a = dir.join(format!("icons/hicolor/48x48/apps/{APP_ID}.png"));
    let b = dir.join(format!("icons/hicolor/128x128/apps/{APP_ID}.png"));
    fs::write(&a, b"junk").unwrap();
    fs::remove_file(&b).unwrap();
    let report = install(&dir, &exe());
    assert_eq!(report.written, vec![a.clone(), b.clone()]);
    assert_eq!(fs::read(&a).unwrap(), files_for(&dir, &exe())[1].bytes);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn other_files_are_left_alone() {
    let dir = temp("others");
    let other = dir.join("applications/other.desktop");
    fs::create_dir_all(other.parent().unwrap()).unwrap();
    fs::write(&other, b"mine").unwrap();
    install(&dir, &exe());
    assert_eq!(fs::read(&other).unwrap(), b"mine");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn unwritable_target_is_reported_and_the_rest_still_written() {
    let dir = temp("blocked");
    // A plain file where the 48 px folder must go blocks that icon only.
    let blocker = dir.join("icons/hicolor/48x48");
    fs::create_dir_all(blocker.parent().unwrap()).unwrap();
    fs::write(&blocker, b"in the way").unwrap();
    let report = install(&dir, &exe());
    assert_eq!(report.failed.len(), 1, "{report:?}");
    assert!(
        report.failed[0]
            .0
            .ends_with(format!("48x48/apps/{APP_ID}.png"))
    );
    assert!(!report.failed[0].1.is_empty());
    assert_eq!(report.written.len(), 3);
    assert!(dir.join(format!("applications/{APP_ID}.desktop")).is_file());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn user_guide_mentions_the_files() {
    let guide = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/guide");
    let mut all = String::new();
    for entry in fs::read_dir(guide).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().is_some_and(|e| e == "md") {
            all.push_str(&fs::read_to_string(p).unwrap());
        }
    }
    assert!(
        all.contains(".desktop"),
        "guide should explain the desktop entry"
    );
    assert!(all.contains("XDG_DATA_HOME"));
    assert!(all.contains("hicolor"));
}
