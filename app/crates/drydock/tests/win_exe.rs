// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m8-win-exe: version-resource helpers, per-OS config folders, and checks that the sources
//! carry the Windows build settings. Pure; runs on any OS. See docs/specs/m8-win-exe.md.

use std::ffi::OsString;
use std::path::PathBuf;

use drydock::look::Os;
use drydock::winmeta::{file_version, packed_version, version_strings};
use drydock::winpaths::config_dir_on;

fn crate_file(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| OsString::from(*v))
    }
}

// ---- file_version / packed_version ----

#[test]
fn file_version_three_parts_gets_a_zero_fourth() {
    assert_eq!(file_version("0.1.2"), Some([0, 1, 2, 0]));
    assert_eq!(file_version("10.20.30"), Some([10, 20, 30, 0]));
}

#[test]
fn file_version_keeps_a_fourth_part() {
    assert_eq!(file_version("1.2.3.4"), Some([1, 2, 3, 4]));
}

#[test]
fn file_version_ignores_prerelease_and_build_suffixes() {
    assert_eq!(file_version("1.2.3-beta.1"), Some([1, 2, 3, 0]));
    assert_eq!(file_version("1.2.3+abc123"), Some([1, 2, 3, 0]));
    assert_eq!(file_version("1.2.3.4-rc1"), Some([1, 2, 3, 4]));
}

#[test]
fn file_version_rejects_bad_input() {
    for bad in [
        "",
        "1",
        "1.2",
        "1.2.3.4.5",
        "a.b.c",
        "1.2.x",
        "1..3",
        "1.2.65536",
        "-1.2.3",
        "1.2.3.",
    ] {
        assert_eq!(file_version(bad), None, "{bad:?}");
    }
}

#[test]
fn file_version_accepts_the_largest_part() {
    assert_eq!(file_version("65535.0.0"), Some([65535, 0, 0, 0]));
}

#[test]
fn packed_version_puts_the_first_part_on_top() {
    assert_eq!(packed_version([0, 0, 0, 0]), 0);
    assert_eq!(packed_version([0, 0, 0, 1]), 1);
    assert_eq!(packed_version([0, 0, 1, 0]), 1 << 16);
    assert_eq!(packed_version([0, 1, 0, 0]), 1 << 32);
    assert_eq!(packed_version([1, 0, 0, 0]), 1 << 48);
    assert_eq!(
        packed_version([0xFFFF, 0x1234, 0x00AB, 7]),
        0xFFFF_1234_00AB_0007
    );
}

// ---- version_strings ----

fn strings(version: &str) -> std::collections::HashMap<&'static str, String> {
    version_strings(version).into_iter().collect()
}

#[test]
fn version_strings_lists_the_eight_entries_in_order() {
    let keys: Vec<_> = version_strings("0.1.2").iter().map(|(k, _)| *k).collect();
    assert_eq!(
        keys,
        [
            "CompanyName",
            "FileDescription",
            "FileVersion",
            "InternalName",
            "LegalCopyright",
            "OriginalFilename",
            "ProductName",
            "ProductVersion"
        ]
    );
}

#[test]
fn version_strings_values() {
    let s = strings("0.1.2-beta");
    assert_eq!(s["CompanyName"], "Subspace Systems");
    assert_eq!(s["FileDescription"], "Subspace Drydock");
    assert_eq!(s["FileVersion"], "0.1.2.0");
    assert_eq!(s["InternalName"], "drydock");
    assert_eq!(s["LegalCopyright"], "Copyright (C) 2026 Subspace Systems");
    assert_eq!(s["OriginalFilename"], "drydock.exe");
    assert_eq!(s["ProductName"], "Subspace Drydock");
    assert_eq!(s["ProductVersion"], "0.1.2-beta");
}

#[test]
fn version_strings_with_an_unparsable_version_use_zeros() {
    let s = strings("nonsense");
    assert_eq!(s["FileVersion"], "0.0.0.0");
    assert_eq!(s["ProductVersion"], "nonsense");
}

#[test]
fn version_strings_say_nothing_about_other_products() {
    for (_, value) in version_strings("1.0.0") {
        let lower = value.to_lowercase();
        assert!(
            !lower.contains("modeler") && !lower.contains("sketchup"),
            "{value}"
        );
    }
}

// ---- config_dir_on ----

#[test]
fn override_wins_on_every_os() {
    for os in [Os::Windows, Os::Mac, Os::Linux] {
        let vars = [
            ("DRYDOCK_CONFIG_DIR", "/custom"),
            ("APPDATA", "/roaming"),
            ("XDG_CONFIG_HOME", "/xdg"),
            ("HOME", "/home/a"),
        ];
        assert_eq!(
            config_dir_on(os, env(&vars)),
            Some(PathBuf::from("/custom"))
        );
    }
}

#[test]
fn empty_override_is_ignored() {
    let vars = [("DRYDOCK_CONFIG_DIR", ""), ("HOME", "/home/a")];
    assert_eq!(
        config_dir_on(Os::Linux, env(&vars)),
        Some(PathBuf::from("/home/a/.config/drydock"))
    );
}

#[test]
fn old_override_variable_is_ignored() {
    let vars = [("MODELER_CONFIG_DIR", "/old"), ("HOME", "/home/a")];
    assert_eq!(
        config_dir_on(Os::Linux, env(&vars)),
        Some(PathBuf::from("/home/a/.config/drydock"))
    );
}

#[test]
fn windows_uses_appdata_and_a_capitalised_folder() {
    let vars = [("APPDATA", r"C:\Users\a\AppData\Roaming")];
    assert_eq!(
        config_dir_on(Os::Windows, env(&vars)),
        Some(PathBuf::from(r"C:\Users\a\AppData\Roaming").join("Drydock"))
    );
}

#[test]
fn windows_falls_back_to_userprofile() {
    let expected = PathBuf::from(r"C:\Users\a")
        .join("AppData")
        .join("Roaming")
        .join("Drydock");
    let missing = [("USERPROFILE", r"C:\Users\a")];
    assert_eq!(
        config_dir_on(Os::Windows, env(&missing)),
        Some(expected.clone())
    );
    let empty = [("APPDATA", ""), ("USERPROFILE", r"C:\Users\a")];
    assert_eq!(config_dir_on(Os::Windows, env(&empty)), Some(expected));
}

#[test]
fn windows_ignores_unix_variables() {
    let vars = [("XDG_CONFIG_HOME", "/xdg"), ("HOME", "/home/a")];
    assert_eq!(config_dir_on(Os::Windows, env(&vars)), None);
}

#[test]
fn windows_with_nothing_set_has_no_folder() {
    assert_eq!(config_dir_on(Os::Windows, env(&[])), None);
    assert_eq!(
        config_dir_on(Os::Windows, env(&[("APPDATA", ""), ("USERPROFILE", "")])),
        None
    );
}

#[test]
fn linux_and_mac_use_xdg_then_home() {
    for os in [Os::Linux, Os::Mac] {
        let xdg = [("XDG_CONFIG_HOME", "/xdg"), ("HOME", "/home/a")];
        assert_eq!(
            config_dir_on(os, env(&xdg)),
            Some(PathBuf::from("/xdg/drydock"))
        );
        let relative = [("XDG_CONFIG_HOME", "rel"), ("HOME", "/home/a")];
        assert_eq!(
            config_dir_on(os, env(&relative)),
            Some(PathBuf::from("/home/a/.config/drydock"))
        );
        assert_eq!(config_dir_on(os, env(&[])), None);
    }
}

#[test]
fn linux_ignores_appdata_and_userprofile() {
    let vars = [("APPDATA", "/roaming"), ("USERPROFILE", "/profile")];
    assert_eq!(config_dir_on(Os::Linux, env(&vars)), None);
}

#[test]
fn recent_config_dir_goes_through_config_dir_on() {
    let source = crate_file("src/recent.rs");
    assert!(
        source.contains("config_dir_on"),
        "recent.rs must call winpaths::config_dir_on"
    );
    assert!(
        !source.contains("cfg!(windows)"),
        "recent.rs must not keep its own Windows branch"
    );
}

// ---- main.rs, build.rs, Cargo.toml, icon ----

#[test]
fn main_hides_the_console_in_release_builds_on_windows() {
    let main = crate_file("src/main.rs");
    assert!(
        main.lines().any(|l| l.trim()
            == r#"#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]"#),
        "main.rs needs the windows_subsystem attribute"
    );
}

#[test]
fn build_script_embeds_icon_and_version_for_windows_targets_only() {
    let build = crate_file("build.rs");
    assert!(
        build.contains("CARGO_CFG_TARGET_OS"),
        "decide by the target, not the host"
    );
    assert!(build.contains("\"windows\""));
    assert!(build.contains("winresource"));
    assert!(build.contains("assets/drydock.ico"));
    assert!(
        build.contains("winmeta"),
        "use the winmeta helpers for the version entries"
    );
    assert!(build.contains("rerun-if-changed"));
    // The git-hash step stays.
    assert!(build.contains("DRYDOCK_GIT_HASH"));
}

#[test]
fn winresource_is_a_build_dependency() {
    let toml = crate_file("Cargo.toml");
    let after = toml
        .split("[build-dependencies]")
        .nth(1)
        .expect("a [build-dependencies] table");
    let table = after.split("\n[").next().unwrap();
    assert!(table.contains("winresource"));
    let deps = toml.split("[dependencies]").nth(1).unwrap();
    let deps = deps.split("\n[").next().unwrap();
    assert!(!deps.contains("winresource"), "not a runtime dependency");
}

#[test]
fn icon_is_a_valid_ico_with_the_usual_sizes() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/drydock.ico");
    let bytes = std::fs::read(&path).expect("assets/drydock.ico");
    assert_eq!(&bytes[..4], &[0, 0, 1, 0], "ICO header");
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    let mut sizes = Vec::new();
    for i in 0..count {
        let e = &bytes[6 + 16 * i..6 + 16 * (i + 1)];
        sizes.push(if e[0] == 0 { 256 } else { u32::from(e[0]) });
        let len = u32::from_le_bytes([e[8], e[9], e[10], e[11]]) as usize;
        let off = u32::from_le_bytes([e[12], e[13], e[14], e[15]]) as usize;
        assert!(off + len <= bytes.len());
        assert_eq!(
            &bytes[off..off + 8],
            b"\x89PNG\r\n\x1a\n",
            "image {i} is a PNG"
        );
    }
    for want in [16, 32, 48, 256] {
        assert!(sizes.contains(&want), "missing {want}px image in {sizes:?}");
    }
}
