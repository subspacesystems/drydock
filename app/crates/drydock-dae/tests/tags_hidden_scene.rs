// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-tags-hidden: instances on a hidden tag are left out of the drawn scene, with
//! everything inside them (`docs/specs/m10-tags-hidden.md`).

use drydock_dae::{FlatScene, RestView, kernel_scene, kernel_scene_editing};
use drydock_kernel::editpath::EditPath;
use drydock_kernel::model::*;
use drydock_kernel::tags::TagId;
use glam::{DMat4, DVec3};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// A new definition holding a 1 m square (one face, 4 edges) at the origin.
fn square_def(m: &mut Model) -> DefinitionId {
    let d = m.add_definition(
        DefinitionInfo {
            name: "part".into(),
            group: false,
        },
        &mut ModelEdit::new(),
    );
    let pts = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(0.0, 1.0, 0.0),
    ];
    m.edit_geometry(BodyId::Definition(d), &mut ModelEdit::new(), |ctx, edit| {
        for i in 0..4 {
            ctx.add_edge(pts[i], pts[(i + 1) % 4], edit).unwrap();
        }
    })
    .unwrap();
    d
}

fn empty_def(m: &mut Model) -> DefinitionId {
    m.add_definition(
        DefinitionInfo {
            name: "group".into(),
            group: true,
        },
        &mut ModelEdit::new(),
    )
}

fn place(m: &mut Model, body: BodyId, d: DefinitionId, x: f64) -> InstanceId {
    m.add_instance(
        body,
        Instance {
            definition: d,
            transform: DMat4::from_translation(v(x, 0.0, 0.0)),
            name: String::new(),
            material: None,
        },
        &mut ModelEdit::new(),
    )
    .unwrap()
}

fn tag(m: &mut Model, name: &str) -> TagId {
    m.add_tag(name, &mut ModelEdit::new()).unwrap()
}

fn assign(m: &mut Model, i: InstanceId, t: TagId) {
    m.assign_tag(i, Some(t), &mut ModelEdit::new()).unwrap();
}

fn show(m: &mut Model, t: TagId, visible: bool) {
    m.set_tag_visible(t, visible, &mut ModelEdit::new())
        .unwrap();
}

fn max_x(s: &FlatScene) -> f32 {
    s.bounds.expect("something is drawn").max[0]
}

#[test]
fn a_visible_tag_draws_like_no_tag() {
    let mut m = Model::new();
    let d = square_def(&mut m);
    let i = place(&mut m, BodyId::Root, d, 0.0);
    let plain = kernel_scene(&m);
    let t = tag(&mut m, "Walls");
    assign(&mut m, i, t);
    let tagged = kernel_scene(&m);
    assert_eq!(plain.triangle_count(), 2);
    assert_eq!(tagged.triangle_count(), 2);
    assert_eq!(tagged.line_count(), plain.line_count());
    assert!(tagged.line_count() > 0);
}

#[test]
fn a_hidden_instance_draws_nothing() {
    let mut m = Model::new();
    let d = square_def(&mut m);
    let i = place(&mut m, BodyId::Root, d, 0.0);
    let t = tag(&mut m, "Walls");
    assign(&mut m, i, t);
    show(&mut m, t, false);
    let s = kernel_scene(&m);
    assert_eq!(s.triangle_count(), 0);
    assert_eq!(s.line_count(), 0);
    assert!(s.bounds.is_none(), "bounds ignore hidden instances");
}

#[test]
fn showing_the_tag_again_brings_it_back() {
    let mut m = Model::new();
    let d = square_def(&mut m);
    let i = place(&mut m, BodyId::Root, d, 0.0);
    let t = tag(&mut m, "Walls");
    assign(&mut m, i, t);
    show(&mut m, t, false);
    show(&mut m, t, true);
    assert_eq!(kernel_scene(&m).triangle_count(), 2);
}

#[test]
fn only_the_hidden_instance_goes() {
    let mut m = Model::new();
    let d = square_def(&mut m);
    let near = place(&mut m, BodyId::Root, d, 0.0);
    let far = place(&mut m, BodyId::Root, d, 50.0);
    let t = tag(&mut m, "Far");
    assign(&mut m, far, t);
    let _ = near;
    assert!(max_x(&kernel_scene(&m)) > 50.0);
    show(&mut m, t, false);
    let s = kernel_scene(&m);
    assert_eq!(s.triangle_count(), 2);
    assert!(max_x(&s) <= 1.0 + 1e-6, "bounds cover only what is drawn");
}

#[test]
fn an_instance_whose_tag_was_removed_is_drawn() {
    let mut m = Model::new();
    let d = square_def(&mut m);
    let i = place(&mut m, BodyId::Root, d, 0.0);
    let t = tag(&mut m, "Walls");
    assign(&mut m, i, t);
    show(&mut m, t, false);
    m.assign_tag(i, None, &mut ModelEdit::new()).unwrap();
    assert_eq!(kernel_scene(&m).triangle_count(), 2);
}

#[test]
fn everything_inside_a_hidden_instance_goes_with_it() {
    let mut m = Model::new();
    let sq = square_def(&mut m);
    let outer = empty_def(&mut m);
    let _inner = place(&mut m, BodyId::Definition(outer), sq, 0.0);
    let o = place(&mut m, BodyId::Root, outer, 0.0);
    let t = tag(&mut m, "Outer");
    assign(&mut m, o, t);
    assert_eq!(kernel_scene(&m).triangle_count(), 2);
    show(&mut m, t, false);
    assert_eq!(kernel_scene(&m).triangle_count(), 0);
}

#[test]
fn a_hidden_instance_inside_a_visible_one_goes_alone() {
    let mut m = Model::new();
    let sq = square_def(&mut m);
    let outer = square_def(&mut m);
    let inner = place(&mut m, BodyId::Definition(outer), sq, 5.0);
    let _o = place(&mut m, BodyId::Root, outer, 0.0);
    assert_eq!(kernel_scene(&m).triangle_count(), 4);
    let t = tag(&mut m, "Inner");
    assign(&mut m, inner, t);
    show(&mut m, t, false);
    let s = kernel_scene(&m);
    assert_eq!(s.triangle_count(), 2, "the outer square stays");
    assert!(max_x(&s) <= 1.0 + 1e-6);
}

#[test]
fn two_instances_of_one_definition_hide_separately() {
    let mut m = Model::new();
    let d = square_def(&mut m);
    let a = place(&mut m, BodyId::Root, d, 0.0);
    let _b = place(&mut m, BodyId::Root, d, 3.0);
    let t = tag(&mut m, "A");
    assign(&mut m, a, t);
    show(&mut m, t, false);
    let s = kernel_scene(&m);
    assert_eq!(s.triangle_count(), 2);
    assert!(s.bounds.unwrap().min[0] >= 3.0 - 1e-6);
}

#[test]
fn editing_views_leave_hidden_instances_out_too() {
    let mut m = Model::new();
    let sq = square_def(&mut m);
    let outer = square_def(&mut m);
    let inside = place(&mut m, BodyId::Definition(outer), sq, 5.0);
    let open = place(&mut m, BodyId::Root, outer, 0.0);
    let aside = place(&mut m, BodyId::Root, sq, 20.0);
    let t = tag(&mut m, "Hidden");
    assign(&mut m, inside, t);
    assign(&mut m, aside, t);
    show(&mut m, t, false);
    let path = EditPath::root().open(&m, open).unwrap();
    // Open part: the outer square only (the nested one is hidden). Rest: the hidden
    // `aside` is not drawn either.
    assert_eq!(
        kernel_scene_editing(&m, &path, RestView::Hidden).triangle_count(),
        2
    );
    let faded = kernel_scene_editing(&m, &path, RestView::Faded);
    assert_eq!(faded.triangle_count(), 2);
    assert!(max_x(&faded) <= 1.0 + 1e-6);
}
