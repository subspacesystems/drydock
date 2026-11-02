// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Native files in the document: open, save, save as (`docs/specs/m1-app-native.md`).

use std::path::{Path, PathBuf};

use modeler::document::{Document, FileKind, native_path};
use modeler_dae::kernel_scene;
use modeler_kernel::format;
use modeler_kernel::model::{BodyId, Model};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn tables() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../modeler-dae/tests/fixtures/tables.dae")
}

/// A fresh, empty temp directory for one test.
fn temp_dir(test: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("modeler-native-test-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn open(path: &Path) -> Document {
    match Document::open(path) {
        Ok(doc) => doc,
        Err(err) => panic!("{} should open: {err}", path.display()),
    }
}

fn open_err(path: &Path) -> String {
    match Document::open(path) {
        Ok(_) => panic!("{} should not open", path.display()),
        Err(err) => err,
    }
}

fn text(model: &Model) -> String {
    format::to_text(model).unwrap()
}

fn kernel_text(doc: &Document) -> String {
    text(doc.kernel.as_ref().expect("document has a kernel model"))
}

/// Faces and edges over the root and every definition body.
fn counts(model: &Model) -> (usize, usize) {
    let mut bodies = vec![model.root()];
    for (id, _) in model.definitions() {
        bodies.push(model.body(BodyId::Definition(id)).unwrap());
    }
    let faces = bodies.iter().map(|b| b.geometry().face_count()).sum();
    let edges = bodies.iter().map(|b| b.geometry().edge_count()).sum();
    (faces, edges)
}

fn assert_one_line_naming(err: &str, name: &str) {
    assert!(
        err.starts_with(&format!("{name}: ")),
        "{err:?} should start with {name:?}"
    );
    assert!(!err.contains('\n') && !err.contains('\r'), "{err:?}");
}

fn assert_close(got: [f64; 3], want: [f64; 3], tol: f64) {
    for i in 0..3 {
        assert!((got[i] - want[i]).abs() <= tol, "{got:?} vs {want:?}");
    }
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

// ---- native_path ----

#[test]
fn native_path_keeps_modeler_extension() {
    assert_eq!(
        native_path(Path::new("a/house.modeler")),
        PathBuf::from("a/house.modeler")
    );
    assert_eq!(
        native_path(Path::new("house.MODELER")),
        PathBuf::from("house.MODELER")
    );
    assert_eq!(
        native_path(Path::new("/x/y/House.Modeler")),
        PathBuf::from("/x/y/House.Modeler")
    );
}

#[test]
fn native_path_replaces_dae() {
    assert_eq!(
        native_path(Path::new("a/house.dae")),
        PathBuf::from("a/house.modeler")
    );
    assert_eq!(
        native_path(Path::new("a/house.DAE")),
        PathBuf::from("a/house.modeler")
    );
    assert_eq!(
        native_path(Path::new("/m/05-house/model.Dae")),
        PathBuf::from("/m/05-house/model.modeler")
    );
}

#[test]
fn native_path_appends_otherwise() {
    assert_eq!(
        native_path(Path::new("house")),
        PathBuf::from("house.modeler")
    );
    assert_eq!(
        native_path(Path::new("house.v2")),
        PathBuf::from("house.v2.modeler")
    );
    assert_eq!(
        native_path(Path::new("dir.d/house")),
        PathBuf::from("dir.d/house.modeler")
    );
    assert_eq!(
        native_path(Path::new("a/house.dae.bak")),
        PathBuf::from("a/house.dae.bak.modeler")
    );
    assert_eq!(
        native_path(Path::new("a/house.modelerx")),
        PathBuf::from("a/house.modelerx.modeler")
    );
    assert_eq!(
        native_path(Path::new(".hidden")),
        PathBuf::from(".hidden.modeler")
    );
}

#[test]
fn native_path_without_file_name_is_unchanged() {
    for p in ["", "/", ".."] {
        assert_eq!(native_path(Path::new(p)), PathBuf::from(p), "{p:?}");
    }
}

// ---- open ----

#[test]
fn dae_document_has_a_kernel_model() {
    let doc = open(&fixture("slab.dae"));
    assert_eq!(doc.kind, FileKind::Dae);
    assert_eq!(doc.path, fixture("slab.dae"));
    assert!(doc.kernel_error.is_none(), "{:?}", doc.kernel_error);
    let kernel = doc.kernel.as_ref().expect("slab converts");
    // The scene is the kernel model's: the quad's four edges and the post.
    assert_eq!(doc.stats.triangles, 2);
    assert_eq!(doc.stats.lines, 5);
    assert_eq!(doc.stats.materials, 1);
    assert_close(doc.stats.size_m, [2.0, 3.0, 4.0], 1e-5);
    // One quad face and its four edges, plus the post.
    assert_eq!((doc.stats.faces, doc.stats.edges), (1, 5));
    assert_eq!(counts(kernel), (1, 5));
    let fixture_text = std::fs::read_to_string(fixture("slab.modeler")).unwrap();
    assert_eq!(text(kernel), fixture_text);
}

#[test]
fn native_document_draws_the_kernel_model() {
    let path = fixture("slab.modeler");
    let doc = open(&path);
    assert_eq!(doc.kind, FileKind::Native);
    assert_eq!(doc.path, path);
    assert!(doc.kernel_error.is_none());
    assert_eq!(doc.title(), "slab.modeler - Drydock");
    let kernel = doc.kernel.as_ref().expect("native file has a kernel model");
    assert_eq!(text(kernel), std::fs::read_to_string(&path).unwrap());

    let want = kernel_scene(kernel);
    assert_eq!(doc.scene.triangle_count(), want.triangle_count());
    assert_eq!(doc.scene.line_count(), want.line_count());
    assert_eq!(doc.scene.materials.len(), want.materials.len());
    assert_eq!(doc.scene.bounds, want.bounds);

    assert_eq!(doc.stats.triangles, doc.scene.triangle_count());
    assert_eq!(doc.stats.lines, doc.scene.line_count());
    assert_eq!(doc.stats.materials, doc.scene.materials.len());
    assert_eq!(doc.stats.triangles, 2);
    assert_eq!(doc.stats.lines, 5);
    assert_eq!(doc.stats.materials, 1);
    assert_close(doc.stats.size_m, [2.0, 3.0, 4.0], 1e-5);
    assert_eq!((doc.stats.faces, doc.stats.edges), (1, 5));
}

#[test]
fn native_extension_is_case_insensitive() {
    let dir = temp_dir("upper");
    let path = dir.join("SLAB.MODELER");
    std::fs::copy(fixture("slab.modeler"), &path).unwrap();
    let doc = open(&path);
    assert_eq!(doc.kind, FileKind::Native);
    assert_eq!((doc.stats.faces, doc.stats.edges), (1, 5));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dae_named_file_is_never_read_as_native() {
    let dir = temp_dir("dae-name");
    let path = dir.join("slab.dae");
    std::fs::copy(fixture("slab.modeler"), &path).unwrap();
    // Native text under a .dae name goes to the DAE reader and fails there.
    let err = open_err(&path);
    assert_one_line_naming(&err, "slab.dae");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn empty_native_model_opens() {
    let dir = temp_dir("empty");
    let path = dir.join("empty.modeler");
    std::fs::write(&path, text(&Model::new())).unwrap();
    let doc = open(&path);
    assert_eq!(doc.kind, FileKind::Native);
    assert_eq!(doc.stats.triangles, 0);
    assert_eq!(doc.stats.lines, 0);
    assert_eq!(doc.stats.size_m, [0.0, 0.0, 0.0]);
    assert_eq!((doc.stats.faces, doc.stats.edges), (0, 0));
    assert!(doc.scene.bounds.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_native_file_is_an_error() {
    let err = open_err(Path::new("/nonexistent/dir/lost.modeler"));
    assert_one_line_naming(&err, "lost.modeler");
}

#[test]
fn native_file_without_header_is_an_error() {
    let dir = temp_dir("no-header");
    for (name, content) in [
        ("hello.modeler", "hello\n".as_bytes()),
        ("empty.modeler", b""),
        ("future.modeler", b"modeler-model 99\nend\n"),
        (
            "collada.modeler",
            std::fs::read(fixture("slab.dae")).unwrap().leak(),
        ),
    ] {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        let err = open_err(&path);
        assert_one_line_naming(&err, name);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn native_parse_error_names_the_line() {
    let dir = temp_dir("bad-line");
    let good = std::fs::read_to_string(fixture("slab.modeler")).unwrap();
    let mut lines: Vec<&str> = good.lines().collect();
    // Line 12 (1-based) becomes a record nobody knows.
    lines[11] = "bogus 1 2 3";
    let path = dir.join("bad.modeler");
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    let err = open_err(&path);
    assert_one_line_naming(&err, "bad.modeler");
    assert!(err.contains("line 12"), "{err}");

    // A record the model refuses: an edge to a vertex that doesn't exist.
    let mut lines: Vec<&str> = good.lines().collect();
    let edge = lines.iter().position(|l| l.starts_with("edge ")).unwrap();
    lines[edge] = "edge 0 0 77";
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    let err = open_err(&path);
    assert_one_line_naming(&err, "bad.modeler");
    assert!(err.contains(&format!("line {}", edge + 1)), "{err}");

    // Truncated: no `end`.
    let cut: String = good.lines().take(5).map(|l| format!("{l}\n")).collect();
    std::fs::write(&path, cut).unwrap();
    let err = open_err(&path);
    assert_one_line_naming(&err, "bad.modeler");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn native_file_that_is_not_utf8_is_an_error() {
    let dir = temp_dir("not-utf8");
    let path = dir.join("binary.modeler");
    let mut bytes = b"modeler-model 1\nmaterial 0 0 0 1 1 \"".to_vec();
    bytes.extend_from_slice(&[0xff, 0xfe, 0x80]);
    bytes.extend_from_slice(b"\" -\nend\n");
    std::fs::write(&path, bytes).unwrap();
    let err = open_err(&path);
    assert_one_line_naming(&err, "binary.modeler");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- save_as / save ----

#[test]
fn save_as_from_dae_writes_a_native_file() {
    let dir = temp_dir("save-as");
    let mut doc = open(&fixture("slab.dae"));
    let before_stats = doc.stats.clone();
    let before_text = kernel_text(&doc);

    let written = doc.save_as(&dir.join("copy.dae")).expect("save_as");
    let want = dir.join("copy.modeler");
    assert_eq!(written, want);
    assert_eq!(doc.path, want);
    assert_eq!(doc.kind, FileKind::Native);
    assert_eq!(doc.title(), "copy.modeler - Drydock");
    // Nothing else about the document changes.
    assert_eq!(doc.stats, before_stats);
    assert_eq!(kernel_text(&doc), before_text);
    assert_eq!(doc.scene.triangle_count(), 2);

    assert_eq!(std::fs::read_to_string(&want).unwrap(), before_text);
    assert_eq!(files_in(&dir), vec!["copy.modeler".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_as_appends_the_extension() {
    let dir = temp_dir("append");
    let mut doc = open(&fixture("slab.dae"));
    let written = doc.save_as(&dir.join("plain")).unwrap();
    assert_eq!(written, dir.join("plain.modeler"));
    assert!(written.is_file());
    assert_eq!(doc.path, written);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reopened_native_file_matches_the_dae() {
    let dir = temp_dir("reopen");
    let mut doc = open(&tables());
    let dae_stats = doc.stats.clone();
    let dae_text = kernel_text(&doc);
    let written = doc.save_as(&dir.join("tables.modeler")).unwrap();

    let native = open(&written);
    assert_eq!(native.kind, FileKind::Native);
    assert_eq!(kernel_text(&native), dae_text);
    assert_eq!(native.stats.faces, dae_stats.faces);
    assert_eq!(native.stats.edges, dae_stats.edges);
    assert_close(native.stats.size_m, dae_stats.size_m, 1e-4);
    assert!(native.stats.triangles > 0);
    let kernel = native.kernel.as_ref().unwrap();
    assert_eq!(native.stats.materials, kernel.materials().count());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn definitions_are_counted_once() {
    let doc = open(&tables());
    let kernel = doc.kernel.as_ref().expect("tables converts");
    assert!(kernel.definitions().count() >= 2);
    assert!(
        kernel
            .definitions()
            .any(|(id, _)| kernel.instances_of(id).len() > 1),
        "fixture should reuse a definition"
    );
    assert_eq!((doc.stats.faces, doc.stats.edges), counts(kernel));
    assert!(doc.stats.faces > 0 && doc.stats.edges > doc.stats.faces);
}

#[test]
fn save_rewrites_a_native_file() {
    let dir = temp_dir("save");
    let path = dir.join("slab.modeler");
    // Same model, different (but valid) layout: an extra blank line after `end`.
    let original = std::fs::read_to_string(fixture("slab.modeler")).unwrap();
    std::fs::write(&path, format!("{original}\n")).unwrap();
    let mut doc = open(&path);
    assert_eq!(doc.save().expect("save"), path);
    assert_eq!(doc.path, path);
    assert_eq!(doc.kind, FileKind::Native);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    assert_eq!(files_in(&dir), vec!["slab.modeler".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_after_save_as_writes_the_new_file() {
    let dir = temp_dir("save-after");
    let mut doc = open(&fixture("slab.dae"));
    let written = doc.save_as(&dir.join("a")).unwrap();
    std::fs::remove_file(&written).unwrap();
    assert_eq!(doc.save().unwrap(), written);
    assert!(written.is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_refuses_a_dae_document() {
    let dir = temp_dir("save-dae");
    let path = dir.join("slab.dae");
    std::fs::copy(fixture("slab.dae"), &path).unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut doc = open(&path);
    let err = doc
        .save()
        .expect_err("a DAE document can't be saved in place");
    assert_one_line_naming(&err, "slab.dae");
    assert_eq!(doc.kind, FileKind::Dae);
    assert_eq!(doc.path, path);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(files_in(&dir), vec!["slab.dae".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_as_failure_leaves_the_document_alone() {
    let mut doc = open(&fixture("slab.dae"));
    let err = doc
        .save_as(Path::new("/nonexistent/dir/out.dae"))
        .expect_err("folder doesn't exist");
    assert_one_line_naming(&err, "out.modeler");
    assert_eq!(doc.path, fixture("slab.dae"));
    assert_eq!(doc.kind, FileKind::Dae);
}

#[test]
fn save_as_onto_a_folder_fails() {
    let dir = temp_dir("onto-dir");
    std::fs::create_dir(dir.join("taken.modeler")).unwrap();
    let mut doc = open(&fixture("slab.dae"));
    let err = doc
        .save_as(&dir.join("taken"))
        .expect_err("a folder is in the way");
    assert_one_line_naming(&err, "taken.modeler");
    assert_eq!(doc.kind, FileKind::Dae);
    assert!(dir.join("taken.modeler").is_dir());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_failure_keeps_the_native_document() {
    let dir = temp_dir("save-fail");
    let path = dir.join("gone").join("slab.modeler");
    std::fs::create_dir(dir.join("gone")).unwrap();
    std::fs::copy(fixture("slab.modeler"), &path).unwrap();
    let mut doc = open(&path);
    std::fs::remove_dir_all(dir.join("gone")).unwrap();
    let err = doc.save().expect_err("folder is gone");
    assert_one_line_naming(&err, "slab.modeler");
    assert_eq!(doc.path, path);
    assert_eq!(doc.kind, FileKind::Native);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn save_as_replaces_an_existing_file() {
    let dir = temp_dir("replace");
    let target = dir.join("old.modeler");
    std::fs::write(&target, "modeler-model 1\nend\n").unwrap();
    let mut doc = open(&fixture("slab.dae"));
    doc.save_as(&target).unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), kernel_text(&doc));
    assert_eq!(files_in(&dir), vec!["old.modeler".to_string()]);
    let _ = std::fs::remove_dir_all(&dir);
}
