// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Native files are `.drydock`; `.modeler` still opens and keeps its path on Save
//! (`docs/specs/m8-rename-extension.md`).

use std::path::{Path, PathBuf};

use drydock::document::{
    Document, FileKind, NATIVE_EXTENSION, OPEN_EXTENSIONS, is_native_file, is_openable_file,
    native_path,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("modeler-ext-test-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_native_extension_is_drydock() {
    assert_eq!(NATIVE_EXTENSION, "drydock");
}

#[test]
fn the_open_list_has_both_native_extensions_and_the_imports() {
    for ext in ["dae", "drydock", "modeler", "stl", "3mf"] {
        assert!(OPEN_EXTENSIONS.contains(&ext), "{ext}");
    }
    assert_eq!(OPEN_EXTENSIONS.len(), 5);
}

#[test]
fn native_files_are_drydock_or_modeler_in_any_case() {
    for name in [
        "a.drydock",
        "a.DRYDOCK",
        "dir.d/B.Drydock",
        "a.modeler",
        "a.MODELER",
    ] {
        assert!(is_native_file(Path::new(name)), "{name}");
    }
    for name in [
        "a.dae",
        "a.stl",
        "a.3mf",
        "a",
        "a.drydockx",
        "a.modelerx",
        "drydock",
    ] {
        assert!(!is_native_file(Path::new(name)), "{name}");
    }
}

#[test]
fn the_open_dialog_lists_the_native_and_import_files() {
    for name in [
        "a.dae",
        "a.DAE",
        "a.drydock",
        "a.Drydock",
        "a.modeler",
        "a.stl",
        "a.STL",
        "a.3mf",
    ] {
        assert!(is_openable_file(Path::new(name)), "{name}");
    }
    for name in ["a.txt", "a.png", "a", "a.drydock.bak", ".drydock", "a.obj"] {
        assert!(!is_openable_file(Path::new(name)), "{name}");
    }
}

#[test]
fn save_paths_for_every_kind_of_name() {
    let cases = [
        ("a/house.dae", "a/house.drydock"),
        ("a/house.DAE", "a/house.drydock"),
        ("a/house.drydock", "a/house.drydock"),
        ("a/house.DRYDOCK", "a/house.DRYDOCK"),
        ("a/house.modeler", "a/house.modeler"),
        ("a/house.Modeler", "a/house.Modeler"),
        ("a/house", "a/house.drydock"),
        ("a/house.v2", "a/house.v2.drydock"),
        ("a/house.stl", "a/house.stl.drydock"),
    ];
    for (given, want) in cases {
        assert_eq!(
            native_path(Path::new(given)),
            PathBuf::from(want),
            "{given}"
        );
    }
}

#[test]
fn an_untitled_model_is_a_drydock_file() {
    let doc = Document::new_empty();
    assert_eq!(doc.path, PathBuf::from("untitled.drydock"));
}

#[test]
fn a_dae_saves_as_drydock_with_the_new_header() {
    let dir = temp_dir("dae");
    let mut doc = Document::open(&fixture("slab.dae")).unwrap();
    let written = doc.save_as(&dir.join("slab.dae")).unwrap();
    assert_eq!(written, dir.join("slab.drydock"));
    let text = std::fs::read_to_string(&written).unwrap();
    assert!(text.starts_with("drydock-model "), "{text}");
    let again = Document::open(&written).unwrap();
    assert_eq!(again.kind, FileKind::Native);
    assert_eq!(again.stats, doc.stats);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_opened_modeler_file_keeps_its_path_on_save_and_gets_the_new_header() {
    let dir = temp_dir("keep");
    let path = dir.join("old.modeler");
    std::fs::copy(fixture("slab.modeler"), &path).unwrap();
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("modeler-model ")
    );
    let mut doc = Document::open(&path).unwrap();
    assert_eq!(doc.kind, FileKind::Native);
    assert_eq!(doc.save().unwrap(), path);
    assert_eq!(doc.path, path);
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("drydock-model ")
    );
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["old.modeler".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_drydock_file_opens_in_any_case_and_matches_the_old_one() {
    let dir = temp_dir("case");
    let upper = dir.join("SLAB.DRYDOCK");
    std::fs::copy(fixture("slab.drydock"), &upper).unwrap();
    let new = Document::open(&upper).unwrap();
    let old = Document::open(&fixture("slab.modeler")).unwrap();
    assert_eq!(new.kind, FileKind::Native);
    assert_eq!(new.stats, old.stats);
    assert_eq!(new.scene.triangle_count(), old.scene.triangle_count());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_with_a_wrong_first_line_says_not_a_drydock_model_file() {
    let dir = temp_dir("bad");
    for name in ["bad.drydock", "bad.modeler"] {
        let path = dir.join(name);
        std::fs::write(&path, "hello\nend\n").unwrap();
        let err = Document::open(&path).err().expect("not a model file");
        assert!(err.contains("not a Drydock model file"), "{err}");
        assert!(err.contains(name), "{err}");
        assert!(!err.to_lowercase().contains("modeler-model"), "{err}");
        assert!(!err.contains('\n'), "{err}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
