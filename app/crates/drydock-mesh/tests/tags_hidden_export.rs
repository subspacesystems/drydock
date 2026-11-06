// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-tags-hidden: the print mesh leaves out instances on a hidden tag, with everything
//! inside them (`docs/specs/m10-tags-hidden.md`).

use drydock_kernel::model::*;
use drydock_mesh::print_mesh::*;
use glam::DMat4;

mod print_common;
use print_common::*;

fn export(model: &Model, scope: PrintScope) -> PrintMesh {
    print_mesh(
        model,
        scope,
        &PrintOptions {
            chord_tolerance_mm: 0.01,
        },
    )
}

fn cube(model: &mut Model, name: &str, x: f64) -> InstanceId {
    let (def, _, _) = box_group(model, name, v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    place(
        model,
        BodyId::Root,
        def,
        DMat4::from_translation(v(x, 0.0, 0.0)),
        name,
    )
}

fn hide(model: &mut Model, instances: &[InstanceId]) {
    let tag = model.add_tag("Hidden", &mut ModelEdit::new()).unwrap();
    for i in instances {
        model
            .assign_tag(*i, Some(tag), &mut ModelEdit::new())
            .unwrap();
    }
    model
        .set_tag_visible(tag, false, &mut ModelEdit::new())
        .unwrap();
}

fn names(mesh: &PrintMesh) -> Vec<String> {
    mesh.objects.iter().map(|o| o.name.clone()).collect()
}

#[test]
fn a_hidden_instance_is_not_exported() {
    let mut m = Model::new();
    let _a = cube(&mut m, "A", 0.0);
    let b = cube(&mut m, "B", 3.0);
    assert_eq!(export(&m, PrintScope::Model).objects.len(), 2);
    hide(&mut m, &[b]);
    let out = export(&m, PrintScope::Model);
    assert_eq!(names(&out), vec!["A".to_string()]);
    assert!(out.skipped.is_empty(), "hidden is not 'skipped'");
}

#[test]
fn a_model_with_only_hidden_instances_exports_nothing() {
    let mut m = Model::new();
    let a = cube(&mut m, "A", 0.0);
    hide(&mut m, &[a]);
    let out = export(&m, PrintScope::Model);
    assert!(out.objects.is_empty());
    assert!(out.skipped.is_empty());
}

#[test]
fn naming_a_hidden_instance_in_the_scope_exports_nothing_for_it() {
    let mut m = Model::new();
    let a = cube(&mut m, "A", 0.0);
    let b = cube(&mut m, "B", 3.0);
    hide(&mut m, &[b]);
    let out = export(&m, PrintScope::Instances(&[a, b]));
    assert_eq!(names(&out), vec!["A".to_string()]);
    let out = export(&m, PrintScope::Instances(&[b]));
    assert!(out.objects.is_empty());
}

#[test]
fn showing_the_tag_again_exports_it_again() {
    let mut m = Model::new();
    let a = cube(&mut m, "A", 0.0);
    hide(&mut m, &[a]);
    let tag = m.instance_tag(a).unwrap();
    m.set_tag_visible(tag, true, &mut ModelEdit::new()).unwrap();
    assert_eq!(names(&export(&m, PrintScope::Model)), vec!["A".to_string()]);
}

#[test]
fn a_hidden_child_goes_and_its_visible_sibling_stays() {
    let mut m = Model::new();
    let (parent, _) = definition(&mut m, "Parent", true);
    let (def, _, _) = box_group(&mut m, "Cube", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    let keep = place(
        &mut m,
        BodyId::Definition(parent),
        def,
        DMat4::IDENTITY,
        "Keep",
    );
    let gone = place(
        &mut m,
        BodyId::Definition(parent),
        def,
        DMat4::from_translation(v(3.0, 0.0, 0.0)),
        "Gone",
    );
    let _ = keep;
    place(&mut m, BodyId::Root, parent, DMat4::IDENTITY, "Parent");
    assert_eq!(export(&m, PrintScope::Model).objects.len(), 2);
    hide(&mut m, &[gone]);
    let out = export(&m, PrintScope::Model);
    assert_eq!(out.objects.len(), 1);
    assert!(out.objects[0].name.ends_with("Keep"), "{:?}", names(&out));
}

#[test]
fn everything_inside_a_hidden_parent_goes_with_it() {
    let mut m = Model::new();
    let (parent, _) = definition(&mut m, "Parent", true);
    let (def, _, _) = box_group(&mut m, "Cube", v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0));
    place(
        &mut m,
        BodyId::Definition(parent),
        def,
        DMat4::IDENTITY,
        "Inner",
    );
    let outer = place(&mut m, BodyId::Root, parent, DMat4::IDENTITY, "Parent");
    assert_eq!(export(&m, PrintScope::Model).objects.len(), 1);
    hide(&mut m, &[outer]);
    assert!(export(&m, PrintScope::Model).objects.is_empty());
}
