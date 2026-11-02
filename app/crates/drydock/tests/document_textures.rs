// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use std::path::{Path, PathBuf};

use drydock::document::{Document, resolve_texture_path, texture_path_for_file};
use drydock_kernel::format;
use drydock_kernel::model::{Material, Model, ModelEdit, Texture};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("modeler-tex-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn texture_of(model: &Model) -> Option<String> {
    model
        .materials()
        .find_map(|(_, m)| m.texture.as_ref().map(|t| t.path.clone()))
}

fn write_model(path: &Path, texture: &str) {
    let mut model = Model::new();
    let mut material = Material::new("wood", [1.0, 1.0, 1.0, 1.0]);
    material.texture = Some(Texture::from(texture));
    model.add_material(material, &mut ModelEdit::new());
    model.add_material(
        Material::new("plain", [0.5, 0.5, 0.5, 1.0]),
        &mut ModelEdit::new(),
    );
    format::save(&model, path).unwrap();
}

#[test]
fn resolve_keeps_absolute_and_joins_relative() {
    assert_eq!(
        resolve_texture_path("/a/b.png", Path::new("/x/y")),
        "/a/b.png"
    );
    assert_eq!(
        resolve_texture_path("tex/b.png", Path::new("/x/y")),
        "/x/y/tex/b.png"
    );
    assert_eq!(resolve_texture_path("b.png", Path::new("")), "b.png");
}

#[test]
fn relative_path_from_the_target_folder() {
    let f = texture_path_for_file;
    assert_eq!(f("/a/tex/w.png", Path::new("/a")), "tex/w.png");
    assert_eq!(f("/a/tex/w.png", Path::new("/a/b/c")), "../../tex/w.png");
    assert_eq!(f("/a/tex/w.png", Path::new("/z")), "../a/tex/w.png");
    assert_eq!(f("/a/w.png", Path::new("/a")), "w.png");
    assert_eq!(
        f("/a/b/../tex/./w.png", Path::new("/a/c/../d")),
        "../tex/w.png"
    );
    assert_eq!(f("/a/tex/w.png", Path::new("/a/tex/")), "w.png");
}

#[test]
fn relative_inputs_use_the_current_directory() {
    let cwd = std::env::current_dir().unwrap();
    let got = texture_path_for_file("tex/w.png", &cwd.join("out"));
    assert_eq!(got, "../tex/w.png");
}

#[test]
fn opening_a_native_file_resolves_texture_paths() {
    let dir = scratch("open");
    let file = dir.join("m.modeler");
    write_model(&file, "tex/wood.png");
    let doc = Document::open(&file).unwrap();
    let want = dir.join("tex/wood.png").to_string_lossy().into_owned();
    assert_eq!(texture_of(doc.kernel.as_ref().unwrap()), Some(want));
}

#[test]
fn save_in_the_same_folder_writes_the_same_path() {
    let dir = scratch("same");
    let file = dir.join("m.modeler");
    write_model(&file, "tex/wood.png");
    let mut doc = Document::open(&file).unwrap();
    doc.save().unwrap();
    let back = format::load(&file).unwrap();
    assert_eq!(texture_of(&back).as_deref(), Some("tex/wood.png"));
}

#[test]
fn save_as_elsewhere_keeps_the_image_reachable() {
    let a = scratch("a");
    let b = scratch("b");
    let file = a.join("m.modeler");
    write_model(&file, "tex/wood.png");
    let mut doc = Document::open(&file).unwrap();
    let before = texture_of(doc.kernel.as_ref().unwrap());

    let target = b.join("sub").join("n.modeler");
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    doc.save_as(&target).unwrap();

    // The document still holds the resolved path.
    assert_eq!(texture_of(doc.kernel.as_ref().unwrap()), before);

    // The file holds a path that leads to the same image from its own folder.
    let back = format::load(&target).unwrap();
    let written = texture_of(&back).unwrap();
    assert!(written.starts_with("../.."), "{written}");
    assert!(!written.contains('\\'));
    let reopened = Document::open(&target).unwrap();
    let want = texture_path_for_file(&before.unwrap(), target.parent().unwrap());
    assert_eq!(written, want);
    let resolved = texture_of(reopened.kernel.as_ref().unwrap()).unwrap();
    assert_eq!(
        texture_path_for_file(&resolved, &a),
        "tex/wood.png",
        "{resolved}"
    );
}

#[test]
fn a_second_save_as_is_still_right() {
    let a = scratch("a2");
    let b = scratch("b2");
    let c = scratch("c2");
    let file = a.join("m.modeler");
    write_model(&file, "wood.png");
    let mut doc = Document::open(&file).unwrap();
    doc.save_as(&b.join("m.modeler")).unwrap();
    doc.save_as(&c.join("m.modeler")).unwrap();
    let back = format::load(&c.join("m.modeler")).unwrap();
    let written = texture_of(&back).unwrap();
    assert_eq!(
        written,
        texture_path_for_file(&a.join("wood.png").to_string_lossy(), &c)
    );
}

#[test]
fn materials_without_a_texture_are_unchanged() {
    let dir = scratch("plain");
    let file = dir.join("m.modeler");
    write_model(&file, "w.png");
    let mut doc = Document::open(&file).unwrap();
    let other = scratch("plain2");
    doc.save_as(&other.join("m.modeler")).unwrap();
    let back = format::load(&other.join("m.modeler")).unwrap();
    let plain = back
        .materials()
        .filter(|(_, m)| m.texture.is_none())
        .count();
    assert_eq!(plain, 1);
    assert_eq!(back.materials().count(), 2);
}

#[test]
fn open_does_not_make_the_document_modified() {
    let dir = scratch("clean");
    let file = dir.join("m.modeler");
    write_model(&file, "w.png");
    let doc = Document::open(&file).unwrap();
    assert!(!doc.modified);
}
