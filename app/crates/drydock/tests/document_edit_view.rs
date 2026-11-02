// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-app-edit-view: `Document::refresh_editing` draws the scene for editing inside an
//! instance. See docs/specs/m4-app-edit-view.md.

use drydock::document::Document;
use drydock_dae::{RestView, kernel_scene};
use drydock_kernel::editpath::EditPath;
use drydock_kernel::model::{BodyId, DefinitionInfo, Instance, InstanceId, Model, ModelEdit};
use drydock_kernel::topology::{EdgeUse, Face, Loop};
use glam::{DMat4, DVec3};

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

#[test]
fn the_root_path_draws_the_normal_scene_for_every_rest_view() {
    let (mut doc, _) = document();
    let normal = kernel_scene(doc.kernel.as_ref().unwrap());
    for rest in [RestView::Normal, RestView::Faded, RestView::Hidden] {
        doc.refresh_editing(&EditPath::root(), rest);
        assert_eq!(
            doc.scene.triangle_count(),
            normal.triangle_count(),
            "{rest:?}"
        );
        assert_eq!(doc.scene.line_count(), normal.line_count(), "{rest:?}");
        assert_eq!(doc.scene.bounds, normal.bounds, "{rest:?}");
    }
}

#[test]
fn hidden_draws_only_the_open_instance() {
    let (mut doc, instance) = document();
    let path = open_path(&doc, instance);
    doc.refresh_editing(&path, RestView::Hidden);
    assert_eq!(doc.scene.triangle_count(), 2);
    assert_eq!(doc.scene.line_count(), 4);
    let bounds = doc.scene.bounds.expect("bounds");
    assert!(bounds.min[0] >= 5.0 - 1e-4, "{bounds:?}");
}

#[test]
fn faded_keeps_the_rest_faces_but_not_its_edges() {
    let (mut doc, instance) = document();
    let path = open_path(&doc, instance);
    doc.refresh_editing(&path, RestView::Faded);
    assert_eq!(doc.scene.triangle_count(), 4);
    assert_eq!(doc.scene.line_count(), 4);
}

#[test]
fn normal_draws_everything() {
    let (mut doc, instance) = document();
    let path = open_path(&doc, instance);
    doc.refresh_editing(&path, RestView::Normal);
    assert_eq!(doc.scene.triangle_count(), 4);
    assert_eq!(doc.scene.line_count(), 8);
}

#[test]
fn a_stale_path_draws_the_normal_scene() {
    let (mut doc, _) = document();
    let path = EditPath {
        steps: vec![InstanceId(9999)],
    };
    doc.refresh_editing(&path, RestView::Hidden);
    assert_eq!(doc.scene.triangle_count(), 4);
    assert_eq!(doc.scene.line_count(), 8);
}

#[test]
fn stats_describe_the_whole_model_whatever_is_drawn() {
    let (mut doc, instance) = document();
    let whole = doc.stats.clone();
    let path = open_path(&doc, instance);
    doc.refresh_editing(&path, RestView::Hidden);
    assert_eq!(doc.stats.faces, whole.faces);
    assert_eq!(doc.stats.edges, whole.edges);
    assert_eq!(doc.stats.triangles, whole.triangles);
    assert_eq!(doc.stats.lines, whole.lines);
    assert_eq!(doc.stats.size_m, whole.size_m);
}

#[test]
fn refresh_editing_marks_the_document_modified_like_refresh() {
    let (mut doc, instance) = document();
    doc.mark_saved_at(0);
    assert!(!doc.modified);
    let path = open_path(&doc, instance);
    doc.refresh_editing(&path, RestView::Faded);
    assert!(doc.modified);
}
