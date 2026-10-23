// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-edit-view-scene: the face and edge counts skip definitions nothing uses.

use glam::{DMat4, DVec3};
use modeler::document::Document;
use modeler_kernel::model::{BodyId, DefinitionInfo, Instance, ModelEdit};
use modeler_kernel::topology::{EdgeUse, Face, Loop};

/// Put a unit square into a new definition; add an instance of it to the root when
/// `instanced`.
fn add_square_definition(doc: &mut Document, instanced: bool) {
    let model = doc.kernel.as_mut().expect("kernel model");
    let mut log = ModelEdit::new();
    let def = model.add_definition(
        DefinitionInfo {
            name: "tile".to_owned(),
            group: false,
        },
        &mut log,
    );
    let pts = [
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::new(1.0, 1.0, 0.0),
        DVec3::new(0.0, 1.0, 0.0),
    ];
    model
        .edit_geometry(BodyId::Definition(def), &mut log, |ctx, log| {
            let vs: Vec<_> = pts.iter().map(|&q| ctx.insert_vertex(q, log)).collect();
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
    if instanced {
        model
            .add_instance(
                BodyId::Root,
                Instance {
                    definition: def,
                    transform: DMat4::IDENTITY,
                    name: String::new(),
                    material: None,
                },
                &mut log,
            )
            .unwrap();
    }
}

#[test]
fn a_definition_with_an_instance_counts() {
    let mut doc = Document::new_empty();
    add_square_definition(&mut doc, true);
    doc.refresh();
    assert_eq!(doc.stats.faces, 1);
    assert_eq!(doc.stats.edges, 4);
}

#[test]
fn a_definition_nothing_uses_does_not_count() {
    let mut doc = Document::new_empty();
    add_square_definition(&mut doc, false);
    doc.refresh();
    assert_eq!(doc.stats.faces, 0);
    assert_eq!(doc.stats.edges, 0);
}

#[test]
fn two_instances_of_one_definition_count_it_once() {
    let mut doc = Document::new_empty();
    add_square_definition(&mut doc, true);
    {
        // A second instance of the same definition.
        let model = doc.kernel.as_mut().unwrap();
        let (def, _) = model
            .definitions()
            .next()
            .map(|(d, i)| (d, i.clone()))
            .unwrap();
        model
            .add_instance(
                BodyId::Root,
                Instance {
                    definition: def,
                    transform: DMat4::from_translation(DVec3::new(3.0, 0.0, 0.0)),
                    name: String::new(),
                    material: None,
                },
                &mut ModelEdit::new(),
            )
            .unwrap();
    }
    doc.refresh();
    assert_eq!(doc.stats.faces, 1);
    assert_eq!(doc.stats.edges, 4);
}
