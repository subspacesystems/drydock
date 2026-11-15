// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Windows installer script `packaging/windows/drydock.nsi`
//! (`docs/specs/m8-installer.md`). The script is checked as text, so these tests run on any
//! OS without `makensis`; one more test builds it when `makensis` is on the PATH.

use std::path::{Path, PathBuf};
use std::process::Command;

fn script_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/windows/drydock.nsi")
}

fn raw() -> String {
    std::fs::read_to_string(script_path()).expect("app/packaging/windows/drydock.nsi exists")
}

/// The script's instruction lines: comments (`;` or `#` to end of line, not inside quotes)
/// and blank lines removed, whitespace trimmed, continuation lines not used.
fn lines() -> Vec<String> {
    raw()
        .lines()
        .map(strip_comment)
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ';' | '#' if !quoted => return &line[..i],
            _ => {}
        }
    }
    line
}

/// Index of the first line that starts with `word` (case-insensitive, whole word) and
/// contains every fragment (case-insensitive).
fn find(word: &str, fragments: &[&str]) -> Option<usize> {
    find_from(0, word, fragments)
}

fn find_from(start: usize, word: &str, fragments: &[&str]) -> Option<usize> {
    lines().iter().enumerate().skip(start).find_map(|(i, l)| {
        let low = l.to_lowercase();
        let first = low.split_whitespace().next().unwrap_or("");
        (first == word.to_lowercase() && fragments.iter().all(|f| low.contains(&f.to_lowercase())))
            .then_some(i)
    })
}

fn must(word: &str, fragments: &[&str]) -> usize {
    find(word, fragments)
        .unwrap_or_else(|| panic!("no `{word}` line containing {fragments:?} in drydock.nsi"))
}

const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\SubspaceDrydock";

#[test]
fn starts_with_spdx_and_copyright() {
    let text = raw();
    let mut it = text.lines();
    assert_eq!(
        it.next(),
        Some("; SPDX-License-Identifier: GPL-3.0-or-later")
    );
    assert_eq!(it.next(), Some("; Copyright (C) 2026 Subspace Systems"));
}

#[test]
fn version_comes_from_the_command_line() {
    must("!ifndef", &["VERSION"]);
    must("!error", &[]);
    must("!endif", &[]);
    // The error is inside the guard.
    assert!(must("!ifndef", &["VERSION"]) < must("!error", &[]));
    assert!(must("!error", &[]) < must("!endif", &[]));
}

#[test]
fn names_and_install_dir() {
    must("Name", &["\"Subspace Drydock\""]);
    let out = must("OutFile", &[]);
    let l = &lines()[out];
    assert!(
        l.contains("${OUTFILE}") || l.contains("${VERSION}"),
        "OutFile should be set from /D values: {l}"
    );
    must("InstallDir", &["$PROGRAMFILES64", r"\Subspace Drydock"]);
    must("RequestExecutionLevel", &["admin"]);
    must("SetRegView", &["64"]);
    must("Unicode", &["true"]);
}

#[test]
fn old_version_is_removed_only_after_the_new_files_are_in_place() {
    let read_loc = must("ReadRegStr", &[UNINSTALL_KEY, "InstallLocation"]);
    let run = must("ExecWait", &["$R1", "uninstall.exe", "/S", "_?="]);
    assert!(
        read_loc < run,
        "read the key before running the uninstaller"
    );
    // The removal happens in a function that the install section calls ...
    let call = must("Call", &["RemoveOldVersion"]);
    // ... after the new program and the guide were copied ...
    let program = must("File", &["${STAGE}", "drydock.exe"]);
    let guide = must("File", &["/nonfatal", "${STAGE}", "guide"]);
    assert!(program < call && guide < call, "copy first, remove after");
    // ... and the leftovers of the old uninstaller are cleaned up after the wait.
    let del = find_from(run, "Delete", &["uninstall"]).expect("Delete of the old uninstaller");
    assert!(del > run);
}

#[test]
fn new_program_is_swapped_in_so_a_failed_copy_keeps_the_old_one() {
    // Copied under another name next to the old program, then moved over it in one step.
    let staged = must("File", &["/oname=", "drydock.exe.new", "${STAGE}"]);
    let swap = must(
        "System::Call",
        &["MoveFileEx", "drydock.exe.new", "drydock.exe"],
    );
    let call = must("Call", &["RemoveOldVersion"]);
    assert!(staged < swap && swap < call);
    // Giving up deletes the staged copy and aborts; nothing else was changed before.
    let abort = find_from(swap, "Abort", &[]).expect("Abort after the swap");
    let del = find_from(swap, "Delete", &["drydock.exe.new"]).expect("staged copy is deleted");
    assert!(swap < del && del < abort && abort < call);
    // Nothing is written to the registry or the Start menu before the swap.
    for (i, l) in lines().iter().enumerate() {
        let low = l.to_lowercase();
        if low.starts_with("writereg") || low.starts_with("createshortcut") {
            assert!(i > call, "written before the old version was handled: {l}");
        }
    }
    let first_exec = must("ExecWait", &[]);
    assert!(
        first_exec < staged,
        "RemoveOldVersion is defined above the section"
    );
}

#[test]
fn the_uninstaller_registry_and_shortcut_come_after_the_removal() {
    // The old uninstaller deletes the shortcut and keys; the new ones are written afterwards.
    let call = must("Call", &["RemoveOldVersion"]);
    for (word, frags) in [
        ("WriteUninstaller", vec!["uninstall.exe"]),
        ("CreateShortcut", vec!["Subspace Drydock.lnk"]),
        ("WriteRegStr", vec![UNINSTALL_KEY, "InstallLocation"]),
        ("WriteRegStr", vec![r"Classes\.drydock"]),
    ] {
        assert!(
            must(word, &frags) > call,
            "`{word}` {frags:?} before the removal"
        );
    }
}

#[test]
fn the_folder_being_installed_into_is_not_uninstalled() {
    // The old uninstaller would delete the files just copied: skip it for the same folder.
    let ls = lines();
    let read_loc = must("ReadRegStr", &[UNINSTALL_KEY, "InstallLocation"]);
    let same = ls
        .iter()
        .position(|l| l.starts_with("StrCmp") && l.contains("$R1") && l.contains("$INSTDIR"))
        .expect("StrCmp of the old folder with $INSTDIR");
    let run = must("ExecWait", &["/S", "_?="]);
    assert!(read_loc < same && same < run);
}

#[test]
fn update_mode_flags_are_documented_and_parsed() {
    let text = raw();
    let header: String = text
        .lines()
        .take_while(|l| !l.starts_with("!ifndef"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        header.contains("/S") && header.contains("/UPDATE"),
        "{header}"
    );
    must("!include", &["FileFunc.nsh"]);
    let opt = must("${GetOptions}", &["/UPDATE"]);
    assert_eq!(enclosing(opt).as_deref(), Some("function .oninit"));
    // The only include is NSIS's own; no plugins beyond the built-in System one.
    for l in lines() {
        let low = l.to_lowercase();
        if low.starts_with("!include") {
            assert!(low.contains("filefunc.nsh"), "unexpected include: {l}");
        }
        assert!(!low.contains("!addplugindir"), "{l}");
        if low.starts_with("system::") {
            continue;
        }
        assert!(
            !low.contains("::") || low.starts_with("system::"),
            "plugin call: {l}"
        );
    }
}

#[test]
fn update_installs_into_the_installed_folder() {
    // In update mode $INSTDIR is taken from InstallLocation (still inside .onInit).
    let set = must("StrCpy", &["$INSTDIR", "$R1"]);
    assert_eq!(enclosing(set).as_deref(), Some("function .oninit"));
    let read_loc = must("ReadRegStr", &[UNINSTALL_KEY, "InstallLocation"]);
    assert!(read_loc < set);
}

#[test]
fn update_restarts_drydock_without_elevation() {
    let ex = must("Exec", &["explorer.exe", "$INSTDIR", "drydock.exe"]);
    assert_eq!(enclosing(ex).as_deref(), Some("function .oninstsuccess"));
    // Only after a successful, silent update; never ExecShell/runas or a direct launch.
    let block: Vec<String> = lines()
        .into_iter()
        .skip_while(|l| !l.to_lowercase().starts_with("function .oninstsuccess"))
        .take_while(|l| !l.eq_ignore_ascii_case("FunctionEnd"))
        .collect();
    assert!(block.iter().any(|l| l.starts_with("IfSilent")), "{block:?}");
    assert!(block.iter().any(|l| l.contains("UpdateMode")), "{block:?}");
    for l in lines() {
        let low = l.to_lowercase();
        assert!(!low.contains("runas"), "{l}");
        if low.starts_with("exec") && low.contains("drydock.exe") {
            assert!(low.contains("explorer.exe"), "launched directly: {l}");
        }
    }
}

#[test]
fn install_never_touches_settings() {
    for l in lines() {
        let low = l.to_lowercase();
        assert!(!low.contains("appdata") && !low.contains("$profile"), "{l}");
    }
}

#[test]
fn old_uninstaller_is_found_by_the_install_folder() {
    // UninstallString is stored in quotes, so `IfFileExists` on it is never true.
    let ls = lines();
    let read_loc = must("ReadRegStr", &[UNINSTALL_KEY, "InstallLocation"]);
    let reg_var = ls[read_loc].split_whitespace().nth(1).unwrap().to_string();
    for (i, l) in ls.iter().enumerate() {
        if l.to_lowercase().starts_with("iffileexists") {
            let target = l.split_whitespace().nth(1).unwrap_or("").trim_matches('"');
            let quoted_string_var = ls.iter().take(i).any(|r| {
                r.to_lowercase().starts_with("readregstr")
                    && r.contains("UninstallString")
                    && r.split_whitespace().nth(1) == Some(target)
            });
            assert!(
                !quoted_string_var,
                "IfFileExists on the quoted UninstallString: {l}"
            );
        }
    }
    // Nothing to remove when the install folder is unknown.
    let skip =
        find("StrCmp", &[reg_var.as_str(), "\"\""]).expect("StrCmp on an empty InstallLocation");
    let exists = must("IfFileExists", &[reg_var.as_str(), "uninstall.exe"]);
    let run = must("ExecWait", &["/S", "_?="]);
    assert!(read_loc < skip && skip < exists && exists < run);
}

/// The block enclosing instruction line `index`: `Some("function .oninit")`, `Some("section
/// install")`, or `None` at the top level.
fn enclosing(index: usize) -> Option<String> {
    let mut open: Option<String> = None;
    for (i, l) in lines().iter().enumerate() {
        if i == index {
            return open;
        }
        let low = l.to_lowercase();
        let first = low.split_whitespace().next().unwrap_or("");
        match first {
            "function" | "section" => {
                let name = low.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
                open = Some(format!("{first} {}", name.trim_matches('"')));
            }
            "functionend" | "sectionend" => open = None,
            _ => {}
        }
    }
    None
}

#[test]
fn registry_view_and_shell_context_are_set_inside_functions() {
    // makensis rejects `SetRegView` and `SetShellVarContext` at the top level.
    for (i, l) in lines().iter().enumerate() {
        let low = l.to_lowercase();
        if low.starts_with("setregview") || low.starts_with("setshellvarcontext") {
            assert!(enclosing(i).is_some(), "outside a Function or Section: {l}");
        }
    }
    // Both are set at start-up of the installer and of the uninstaller.
    for func in ["function .oninit", "function un.oninit"] {
        for (word, arg) in [("setregview", "64"), ("setshellvarcontext", "all")] {
            let found = lines().iter().enumerate().any(|(i, l)| {
                let low = l.to_lowercase();
                low.starts_with(word) && low.contains(arg) && enclosing(i).as_deref() == Some(func)
            });
            assert!(found, "`{word} {arg}` missing in `{func}`");
        }
    }
}

#[test]
fn installs_the_program_and_writes_the_uninstaller() {
    must("SetOutPath", &["$INSTDIR"]);
    must("File", &["${STAGE}", "drydock.exe"]);
    // The guide is optional (not built yet when this task lands).
    must("File", &["/nonfatal", "/r", "${STAGE}", "guide"]);
    must("WriteUninstaller", &["$INSTDIR", "uninstall.exe"]);
}

#[test]
fn start_menu_shortcut() {
    must(
        "CreateShortcut",
        &[
            "$SMPROGRAMS",
            "Subspace Drydock.lnk",
            "$INSTDIR",
            "drydock.exe",
        ],
    );
    must("Delete", &["$SMPROGRAMS", "Subspace Drydock.lnk"]);
}

#[test]
fn add_remove_programs_entry() {
    for (name, expect) in [
        ("DisplayName", "\"Subspace Drydock\""),
        ("Publisher", "\"Subspace Systems\""),
        ("DisplayVersion", "${VERSION}"),
        ("UninstallString", "uninstall.exe"),
        ("InstallLocation", "$INSTDIR"),
        ("DisplayIcon", "drydock.exe"),
    ] {
        must("WriteRegStr", &["HKLM", UNINSTALL_KEY, name, expect]);
    }
    must("WriteRegDWORD", &["HKLM", UNINSTALL_KEY, "NoModify", "1"]);
    must("WriteRegDWORD", &["HKLM", UNINSTALL_KEY, "NoRepair", "1"]);
}

#[test]
fn registers_only_the_drydock_extension() {
    // Any registered extension key (`...\Classes\.xyz` or `HKCR ".xyz"`) is .drydock.
    let mut seen = 0;
    for l in lines() {
        let low = l.to_lowercase();
        if !(low.starts_with("writeregstr") || low.starts_with("writeregdword")) {
            continue;
        }
        let ext_key = low.contains(r"classes\.") || low.contains("hkcr \".");
        if ext_key {
            seen += 1;
            assert!(
                low.contains(".drydock"),
                "only .drydock may be registered: {l}"
            );
        }
    }
    assert!(seen >= 1, "the .drydock extension is registered");
    must(
        "WriteRegStr",
        &[r"Classes\.drydock", "SubspaceDrydock.Model"],
    );
    // The file type opens with the program, passing the file.
    let open = must(
        "WriteRegStr",
        &[r"SubspaceDrydock.Model\shell\open\command"],
    );
    let l = &lines()[open];
    assert!(
        l.contains("$INSTDIR") && l.contains("drydock.exe") && l.contains("%1"),
        "{l}"
    );
}

#[test]
fn uninstaller_removes_what_the_installer_added() {
    assert!(
        find("Section", &["Uninstall"]).is_some(),
        "an `Section \"Uninstall\"`"
    );
    let sec = must("Section", &["Uninstall"]);
    for (word, frags) in [
        ("Delete", vec!["$INSTDIR", "drydock.exe"]),
        ("Delete", vec!["$INSTDIR", "uninstall.exe"]),
        ("RMDir", vec!["$INSTDIR"]),
        ("DeleteRegKey", vec!["HKLM", UNINSTALL_KEY]),
        ("DeleteRegKey", vec![r"Classes\.drydock"]),
        ("DeleteRegKey", vec![r"Classes\SubspaceDrydock.Model"]),
    ] {
        assert!(
            find_from(sec, word, &frags).is_some(),
            "uninstall section lacks `{word}` with {frags:?}"
        );
    }
}

#[test]
fn uninstaller_leaves_settings_and_models_alone() {
    for l in lines() {
        let low = l.to_lowercase();
        for bad in [
            "appdata",
            "$documents",
            "$profile",
            "$mydocuments",
            "*.drydock",
            "$desktop",
        ] {
            assert!(!low.contains(bad), "`{bad}` in script line: {l}");
        }
        // No recursive delete of anything but a subfolder we install.
        if low.starts_with("rmdir") && low.contains("/r") {
            assert!(
                !low.trim_end().ends_with("$instdir\"") && !low.contains("$programfiles"),
                "recursive delete too broad: {l}"
            );
        }
    }
}

#[test]
fn pages_for_install_and_uninstall() {
    must("Page", &["directory"]);
    must("Page", &["instfiles"]);
    must("UninstPage", &["uninstConfirm"]);
    must("UninstPage", &["instfiles"]);
}

#[test]
fn names_no_other_product() {
    let low = raw().to_lowercase();
    for bad in ["modeler", "sketchup", "trimble", "@last", "last software"] {
        assert!(!low.contains(bad), "drydock.nsi mentions `{bad}`");
    }
}

#[test]
fn builds_when_makensis_is_available() {
    if Command::new("makensis").arg("-VERSION").output().is_err() {
        eprintln!("makensis not installed: skipping");
        return;
    }
    let dir = std::env::temp_dir().join(format!("drydock-nsi-{}", std::process::id()));
    let stage = dir.join("stage");
    std::fs::create_dir_all(&stage).unwrap();
    std::fs::write(stage.join("drydock.exe"), b"MZ").unwrap();
    let out = dir.join("setup.exe");
    let run = Command::new("makensis")
        .arg("-V2")
        .arg("-DVERSION=1.2.3")
        .arg(format!("-DSTAGE={}", stage.display()))
        .arg(format!("-DOUTFILE={}", out.display()))
        .arg(script_path())
        .output()
        .unwrap();
    let text =
        String::from_utf8_lossy(&run.stdout).to_string() + &String::from_utf8_lossy(&run.stderr);
    let ok = run.status.success() && out.exists();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(ok, "makensis failed:\n{text}");
}

#[test]
fn version_is_required_when_building() {
    if Command::new("makensis").arg("-VERSION").output().is_err() {
        return;
    }
    let run = Command::new("makensis")
        .arg("-V1")
        .arg("-DSTAGE=/nonexistent")
        .arg(script_path())
        .output()
        .unwrap();
    assert!(
        !run.status.success(),
        "building without /DVERSION must fail"
    );
}
