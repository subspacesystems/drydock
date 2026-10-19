// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The `modeler` binary's `--convert` and native-file `--screenshot`
//! (`docs/specs/m1-app-native.md`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use modeler_kernel::format;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("modeler-cli-test-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_modeler"))
        .args(args)
        .output()
        .expect("modeler binary runs")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn s(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn is_empty_dir(dir: &Path) -> bool {
    std::fs::read_dir(dir).unwrap().next().is_none()
}

#[test]
fn convert_writes_a_native_file() {
    let dir = temp_dir("convert");
    let out = run(&[
        "--convert",
        s(&dir.join("slab.dae")),
        s(&fixture("slab.dae")),
    ]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let want = dir.join("slab.modeler");
    assert_eq!(stdout(&out), format!("wrote {}\n", want.display()));
    let written = std::fs::read_to_string(&want).unwrap();
    assert_eq!(
        written,
        std::fs::read_to_string(fixture("slab.modeler")).unwrap()
    );
    assert!(format::load(&want).is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn convert_option_may_come_after_the_file() {
    let dir = temp_dir("convert-after");
    let out = run(&[
        s(&fixture("slab.dae")),
        "--convert",
        s(&dir.join("x.modeler")),
    ]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(dir.join("x.modeler").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn convert_native_to_native() {
    let dir = temp_dir("convert-native");
    let out = run(&[
        "--convert",
        s(&dir.join("copy")),
        s(&fixture("slab.modeler")),
    ]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let want = dir.join("copy.modeler");
    assert_eq!(stdout(&out), format!("wrote {}\n", want.display()));
    assert_eq!(
        std::fs::read(&want).unwrap(),
        std::fs::read(fixture("slab.modeler")).unwrap()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn convert_of_a_missing_file_fails() {
    let dir = temp_dir("convert-missing");
    let out = run(&[
        "--convert",
        s(&dir.join("out.modeler")),
        s(&dir.join("lost.dae")),
    ]);
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(err.starts_with("modeler: lost.dae: "), "{err}");
    assert!(stdout(&out).is_empty());
    assert!(is_empty_dir(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn convert_into_a_missing_folder_fails() {
    let dir = temp_dir("convert-nofolder");
    let out = run(&[
        "--convert",
        s(&dir.join("nope").join("out.dae")),
        s(&fixture("slab.dae")),
    ]);
    assert_eq!(out.status.code(), Some(1));
    let err = stderr(&out);
    assert!(err.starts_with("modeler: out.modeler: "), "{err}");
    assert!(is_empty_dir(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn convert_usage_errors() {
    let dir = temp_dir("convert-usage");
    let out_path = dir.join("out.modeler");
    let png = dir.join("shot.png");
    let slab = fixture("slab.dae");
    for args in [
        vec!["--convert"],
        vec!["--convert", s(&out_path)],
        vec![s(&slab), "--convert"],
        vec!["--convert", s(&out_path), "--screenshot", s(&png), s(&slab)],
        vec!["--screenshot", s(&png), "--convert", s(&out_path), s(&slab)],
    ] {
        let out = run(&args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(stderr(&out).starts_with("modeler: "), "{args:?}");
        assert!(is_empty_dir(&dir), "{args:?} wrote something");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn help_mentions_native_files() {
    let out = run(&["--help"]);
    assert_eq!(out.status.code(), Some(0));
    let help = stdout(&out);
    assert!(help.contains("--convert"), "{help}");
    assert!(help.contains(".modeler"), "{help}");
    assert!(help.contains("--screenshot"), "{help}");
}

#[test]
fn screenshot_of_a_native_file() {
    let dir = temp_dir("shot");
    let png = dir.join("slab.png");
    let out = run(&["--screenshot", s(&png), s(&fixture("slab.modeler"))]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let bytes = std::fs::read(&png).unwrap();
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    // IHDR: width and height, big-endian, at bytes 16..24.
    assert_eq!(&bytes[16..20], &1280u32.to_be_bytes());
    assert_eq!(&bytes[20..24], &800u32.to_be_bytes());
    let _ = std::fs::remove_dir_all(&dir);
}
