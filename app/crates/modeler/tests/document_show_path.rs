// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-fix-refresh-speed: `Document::show_path` redraws for a new open path without
//! touching stats or the modified flag. See docs/specs/m4-fix-refresh-speed.md.

use glam::{DMat4, DVec3};
use modeler::document::Document;
use modeler_dae::{RestView, kernel_scene, kernel_scene_editing};
use modeler_kernel::editpath::EditPath;
use modeler_kernel::model::{BodyId, DefinitionInfo, Instance, InstanceId, Model, ModelEdit};
use modeler_kernel::topology::{EdgeUse, Face, Loop};

fn add_square(model: &mut Model, body: BodyId, offset: DVec3, log: &mut ModelEdit) {
    let pts = [
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::new(1.0, 1.0, 0.0),
        DVec3::new(0.0, 1.0, 0.0),
    ];
    model
        .edit_geometry(body, log, |ctx, log| {
            let vs: Vec<_> = pts
                .iter()
                .map(|&q| ctx.insert_vertex(q + offset, log))
                .collect();
            let uses = (0..4)
                .map(|i| EdgeUse::forward(ctx.insert_edge(vs[i], vs[(i + 1) % 4], log).unwrap()))
                .collect();
            ctx.insert_face(
                Face {
                    outer: Loop { uses },
                    inner: vec![],
                },
                log,
            )
            .unwrap();
        })
        .unwrap();
}

/// A document whose root has one square and one instance of a definition holding another
/// square, placed 5 m along red. Returns the document and the instance.
fn document() -> (Document, InstanceId) {
    let mut doc = Document::new_empty();
    let model = doc.kernel.as_mut().expect("kernel model");
    let mut log = ModelEdit::new();
    add_square(model, BodyId::Root, DVec3::ZERO, &mut log);
    let def = model.add_definition(
        DefinitionInfo {
            name: "tile".to_owned(),
            group: false,
        },
        &mut log,
    );
    add_square(model, BodyId::Definition(def), DVec3::ZERO, &mut log);
    let instance = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: def,
                transform: DMat4::from_translation(DVec3::new(5.0, 0.0, 0.0)),
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    doc.refresh();
    (doc, instance)
}

fn open_path(doc: &Document, instance: InstanceId) -> EditPath {
    EditPath::root()
        .open(doc.kernel.as_ref().unwrap(), instance)
        .expect("instance opens")
}

fn same_scene(doc: &Document, want: &modeler_dae::FlatScene) {
    assert_eq!(doc.scene.triangle_count(), want.triangle_count());
    assert_eq!(doc.scene.line_count(), want.line_count());
    assert_eq!(doc.scene.bounds, want.bounds);
    assert_eq!(doc.scene.materials.len(), want.materials.len());
}

#[test]
fn show_path_draws_the_editing_scene() {
    let (mut doc, instance) = document();
    let path = open_path(&doc, instance);
    for rest in [RestView::Faded, RestView::Hidden] {
        doc.show_path(&path, rest);
        let want = kernel_scene_editing(doc.kernel.as_ref().unwrap(), &path, rest);
        same_scene(&doc, &want);
    }
}

#[test]
fn show_path_at_the_root_draws_the_normal_scene() {
    let (mut doc, instance) = document();
    let path = open_path(&doc, instance);
    doc.show_path(&path, RestView::Hidden);
    doc.show_path(&EditPath::root(), RestView::Hidden);
    let want = kernel_scene(doc.kernel.as_ref().unwrap());
    same_scene(&doc, &want);
}

#[test]
fn show_path_keeps_stats() {
    let (mut doc, instance) = document();
    let before = doc.stats.clone();
    let path = open_path(&doc, instance);
    doc.show_path(&path, RestView::Hidden);
    assert_eq!(doc.stats, before);
    doc.show_path(&path, RestView::Faded);
    assert_eq!(doc.stats, before);
    doc.show_path(&EditPath::root(), RestView::Faded);
    assert_eq!(doc.stats, before);
}

#[test]
fn show_path_keeps_the_modified_flag() {
    let (mut doc, instance) = document();
    let path = open_path(&doc, instance);
    doc.mark_saved_at(0);
    doc.show_path(&path, RestView::Faded);
    assert!(!doc.modified, "opening an instance is not an edit");
    doc.show_path(&EditPath::root(), RestView::Faded);
    assert!(!doc.modified);
    doc.modified = true;
    doc.show_path(&path, RestView::Faded);
    assert!(doc.modified);
}

#[test]
fn refresh_editing_still_counts_the_whole_model() {
    let (mut doc, instance) = document();
    let full = doc.stats.clone();
    let path = open_path(&doc, instance);
    doc.refresh_editing(&path, RestView::Hidden);
    assert_eq!(doc.stats, full);
    doc.refresh_editing(&EditPath::root(), RestView::Faded);
    assert_eq!(doc.stats, full);
}

#[test]
fn show_path_without_a_kernel_does_nothing() {
    let (mut doc, instance) = document();
    let path = open_path(&doc, instance);
    doc.kernel = None;
    let triangles = doc.scene.triangle_count();
    doc.show_path(&path, RestView::Hidden);
    assert_eq!(doc.scene.triangle_count(), triangles);
}
