// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The installer tells the user about an installed older version before replacing it
//! (`docs/specs/m8-installer-notice.md`). Text checks on `packaging/windows/drydock.nsi`, so
//! they run on any OS; the build itself is checked by `installer_script.rs` and the oracle.

use std::path::{Path, PathBuf};

fn script_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/windows/drydock.nsi")
}

/// Instruction lines (comments and blank lines removed, trimmed) of `Function .onInit`.
fn on_init() -> Vec<String> {
    let text = std::fs::read_to_string(script_path()).expect("drydock.nsi exists");
    let mut inside = false;
    let mut out = Vec::new();
    for raw in text.lines() {
        let mut quoted = false;
        let mut end = raw.len();
        for (i, c) in raw.char_indices() {
            match c {
                '"' => quoted = !quoted,
                ';' | '#' if !quoted => {
                    end = i;
                    break;
                }
                _ => {}
            }
        }
        let line = raw[..end].trim();
        let low = line.to_lowercase();
        if low.starts_with("function ") && low.contains(".oninit") && !low.contains("un.") {
            inside = true;
        } else if low == "functionend" {
            inside = false;
        } else if inside && !line.is_empty() {
            out.push(line.to_string());
        }
    }
    assert!(!out.is_empty(), "Function .onInit not found");
    out
}

fn first(lines: &[String], word: &str) -> Option<usize> {
    lines.iter().position(|l| {
        l.split_whitespace()
            .next()
            .is_some_and(|w| w.eq_ignore_ascii_case(word))
    })
}

fn boxes(lines: &[String]) -> Vec<usize> {
    (0..lines.len())
        .filter(|&i| lines[i].to_lowercase().starts_with("messagebox"))
        .collect()
}

#[test]
fn reads_the_installed_version() {
    let ls = on_init();
    assert!(
        ls.iter().any(|l| l.starts_with("ReadRegStr")
            && l.contains(r"Uninstall\SubspaceDrydock")
            && l.contains("DisplayVersion")),
        "no ReadRegStr of DisplayVersion in .onInit"
    );
}

#[test]
fn notice_is_only_for_an_existing_install() {
    // The install folder is read first; an empty value skips the notice.
    let ls = on_init();
    let loc = ls
        .iter()
        .position(|l| l.starts_with("ReadRegStr") && l.contains("InstallLocation"))
        .expect("InstallLocation read in .onInit");
    let skip = ls
        .iter()
        .position(|l| l.starts_with("StrCmp") && l.contains("\"\""))
        .expect("StrCmp on an empty value in .onInit");
    let first_box = boxes(&ls)[0];
    assert!(loc < skip && skip < first_box);
}

#[test]
fn silent_installs_show_no_dialog() {
    let ls = on_init();
    let silent = first(&ls, "IfSilent").expect("IfSilent in .onInit");
    let bs = boxes(&ls);
    assert!(!bs.is_empty(), "no MessageBox in .onInit");
    assert!(
        silent < bs[0],
        "IfSilent must come before the first MessageBox"
    );
}

#[test]
fn dialog_names_old_folder_and_new_version() {
    let ls = on_init();
    let bs = boxes(&ls);
    let with_version: Vec<&String> = bs
        .iter()
        .map(|&i| &ls[i])
        .filter(|l| l.contains("is installed in") && l.contains("will remove it and install"))
        .collect();
    assert!(
        !with_version.is_empty(),
        "no 'is installed in ... will remove it and install' text"
    );
    for l in &with_version {
        assert!(l.contains("Subspace Drydock"), "{l}");
        assert!(l.contains("${VERSION}"), "new version missing: {l}");
        assert!(l.to_uppercase().contains("MB_OKCANCEL"), "{l}");
        assert!(l.to_uppercase().contains("MB_ICONINFORMATION"), "{l}");
    }
}

#[test]
fn same_version_says_reinstall() {
    let ls = on_init();
    let found = boxes(&ls).iter().any(|&i| {
        let l = ls[i].to_lowercase();
        l.contains("reinstall") && l.contains("${version}")
    });
    assert!(
        found,
        "no MessageBox saying 'reinstall' for the same version"
    );
    assert!(
        ls.iter()
            .any(|l| l.starts_with("StrCmp") && l.contains("${VERSION}")),
        "no StrCmp comparing the old version with ${{VERSION}}"
    );
}

#[test]
fn missing_version_leaves_it_out_of_the_text() {
    // One dialog that names the folder but has no old-version variable: DisplayVersion empty.
    let ls = on_init();
    let ver_var = ls
        .iter()
        .find(|l| l.starts_with("ReadRegStr") && l.contains("DisplayVersion"))
        .and_then(|l| l.split_whitespace().nth(1))
        .expect("DisplayVersion variable")
        .to_string();
    let without = boxes(&ls)
        .iter()
        .any(|&i| ls[i].contains("is installed in") && !ls[i].contains(ver_var.as_str()));
    assert!(
        without,
        "no dialog text that omits the old version ({ver_var})"
    );
    assert!(
        ls.iter()
            .any(|l| l.starts_with("StrCmp") && l.contains(ver_var.as_str()) && l.contains("\"\"")),
        "the empty-version case is not tested"
    );
}

#[test]
fn cancel_aborts_before_anything_is_removed() {
    let ls = on_init();
    let abort = first(&ls, "Abort").expect("Abort in .onInit");
    let last_box = *boxes(&ls).last().unwrap();
    assert!(abort > boxes(&ls)[0], "Abort must follow the dialog");
    assert!(
        abort <= last_box + 3,
        "Abort must directly follow the last dialog"
    );
    assert!(
        ls.iter().any(|l| l.to_lowercase().starts_with("messagebox")
            && (l.contains("IDOK") || l.contains("IDCANCEL"))),
        "the MessageBox must branch on the user's choice"
    );
}

#[test]
fn removal_stays_in_the_install_section() {
    let ls = on_init();
    assert!(
        !ls.iter().any(|l| l.to_lowercase().starts_with("execwait")),
        ".onInit only asks; the old version is removed where it is now"
    );
    let text = std::fs::read_to_string(script_path()).unwrap();
    assert!(text.contains("Call RemoveOldVersion"));
}
