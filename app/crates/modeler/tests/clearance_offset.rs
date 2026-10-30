// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-clearance-offset-ui: what the selection holds, the scale of an instance, the status
//! line and `Editor::offset_instances` on groups and components
//! (`docs/specs/m7-clearance-offset-ui.md`).

use std::collections::BTreeMap;

use glam::{DMat4, DVec3};
use modeler::clearance_offset::*;
use modeler::editor::Editor;
use modeler::fit_check::FitCheck;
use modeler::input::EditKey;
use modeler_kernel::editpath::EditPath;
use modeler_kernel::model::*;
use modeler_kernel::offset::OffsetError;
use modeler_kernel::select::Entity;
use modeler_kernel::solid::{SolidReport, check_body};
use modeler_kernel::topology::*;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

const FACES: [[usize; 4]; 6] = [
    [0, 3, 2, 1],
    [4, 5, 6, 7],
    [0, 1, 5, 4],
    [2, 3, 7, 6],
    [1, 2, 6, 5],
    [3, 0, 4, 7],
];

/// A group in `body` holding a box of side `s` (without the faces in `skip`), named
/// `name` (the instance's name), placed by `transform`.
fn add_box(
    model: &mut Model,
    body: BodyId,
    s: f64,
    skip: &[usize],
    name: &str,
    transform: DMat4,
) -> InstanceId {
    let mut log = ModelEdit::new();
    let definition = model.add_definition(
        DefinitionInfo {
            name: "Box".into(),
            group: true,
        },
        &mut log,
    );
    let points = [
        v(0.0, 0.0, 0.0),
        v(s, 0.0, 0.0),
        v(s, s, 0.0),
        v(0.0, s, 0.0),
        v(0.0, 0.0, s),
        v(s, 0.0, s),
        v(s, s, s),
        v(0.0, s, s),
    ];
    model
        .edit_geometry(BodyId::Definition(definition), &mut log, |ctx, edit| {
            let vs: Vec<VertexId> = points.iter().map(|&p| ctx.insert_vertex(p, edit)).collect();
            let mut edges = BTreeMap::new();
            for (n, quad) in FACES.iter().enumerate() {
                if skip.contains(&n) {
                    continue;
                }
                let mut uses = Vec::new();
                for i in 0..4 {
                    let (a, b) = (quad[i], quad[(i + 1) % 4]);
                    let key = (a.min(b), a.max(b));
                    let e = *edges
                        .entry(key)
                        .or_insert_with(|| ctx.insert_edge(vs[key.0], vs[key.1], edit).unwrap());
                    uses.push(if a < b {
                        EdgeUse::forward(e)
                    } else {
                        EdgeUse::backward(e)
                    });
                }
                ctx.insert_face(
                    Face {
                        outer: Loop { uses },
                        inner: vec![],
                    },
                    edit,
                )
                .unwrap();
            }
        })
        .unwrap();
    add_instance(model, body, definition, name, transform)
}

fn add_instance(
    model: &mut Model,
    body: BodyId,
    definition: DefinitionId,
    name: &str,
    transform: DMat4,
) -> InstanceId {
    model
        .add_instance(
            body,
            Instance {
                definition,
                transform,
                name: name.into(),
                material: None,
            },
            &mut ModelEdit::new(),
        )
        .unwrap()
}

fn definition_of(model: &Model, body: BodyId, id: InstanceId) -> DefinitionId {
    model.body(body).unwrap().instance(id).unwrap().definition
}

/// The volume of the instance's definition, cubic meters.
fn volume(model: &Model, body: BodyId, id: InstanceId) -> f64 {
    let definition = definition_of(model, body, id);
    check_body(model, BodyId::Definition(definition))
        .unwrap()
        .volume
        .expect("a solid")
}

fn cube(s: f64) -> f64 {
    s * s * s
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1e-12)
}

const S: f64 = 0.02; // a 20 mm box
const D: f64 = 0.0002; // 0.2 mm

fn select(editor: &mut Editor, ids: &[InstanceId]) {
    editor.select(ids.iter().map(|&id| Entity::Instance(id)).collect());
}

// ---- selected_instances ----

#[test]
fn selected_instances_keeps_order_and_skips_the_rest() {
    let a = InstanceId(7);
    let b = InstanceId(3);
    let selection = [
        Entity::Instance(a),
        Entity::Face(FaceId(1)),
        Entity::Instance(b),
        Entity::Edge(EdgeId(2)),
        Entity::Instance(a),
    ];
    assert_eq!(selected_instances(&selection), vec![a, b]);
    assert!(selected_instances(&[]).is_empty());
    assert!(selected_instances(&[Entity::Face(FaceId(1))]).is_empty());
}

// ---- uniform_scale ----

#[test]
fn uniform_scale_of_similarities() {
    assert_eq!(uniform_scale(&DMat4::IDENTITY), Some(1.0));
    let m = DMat4::from_translation(v(1.0, 2.0, 3.0))
        * DMat4::from_rotation_z(0.7)
        * DMat4::from_rotation_x(-0.3)
        * DMat4::from_scale(DVec3::splat(2.5));
    assert!(near(uniform_scale(&m).unwrap(), 2.5));
    let half = DMat4::from_scale(DVec3::splat(0.5));
    assert!(near(uniform_scale(&half).unwrap(), 0.5));
    // A mirror is still a similarity.
    let mirror = DMat4::from_scale(v(-1.0, 1.0, 1.0));
    assert!(near(uniform_scale(&mirror).unwrap(), 1.0));
    // Float noise from composing transforms is fine.
    let noisy = DMat4::from_scale(v(2.0, 2.0 * (1.0 + 1e-12), 2.0));
    assert!(near(uniform_scale(&noisy).unwrap(), 2.0));
}

#[test]
fn uniform_scale_refuses_the_rest() {
    let uneven = DMat4::from_scale(v(1.0, 2.0, 1.0));
    assert_eq!(uniform_scale(&uneven), None);
    let slight = DMat4::from_scale(v(1.0, 1.0 + 1e-6, 1.0));
    assert_eq!(uniform_scale(&slight), None);
    let mut shear = DMat4::IDENTITY;
    shear.y_axis.x = 0.5;
    assert_eq!(uniform_scale(&shear), None);
    assert_eq!(uniform_scale(&DMat4::from_scale(DVec3::ZERO)), None);
    let mut nan = DMat4::IDENTITY;
    nan.x_axis.x = f64::NAN;
    assert_eq!(uniform_scale(&nan), None);
    // Rotated and unevenly scaled.
    let both = DMat4::from_rotation_z(0.4) * DMat4::from_scale(v(3.0, 1.0, 1.0));
    assert_eq!(uniform_scale(&both), None);
}

// ---- reason, Direction ----

#[test]
fn directions_have_labels() {
    assert_eq!(Direction::Grow.label(), "Grow by Clearance");
    assert_eq!(Direction::Shrink.label(), "Shrink by Clearance");
}

#[test]
fn reasons_for_each_refusal() {
    use Direction::*;
    let not_solid = OffsetError::NotSolid(Box::<SolidReport>::default());
    assert_eq!(reason(&not_solid, Grow), "not a solid (try Repair)");
    assert_eq!(
        reason(&OffsetError::Curved(4), Shrink),
        "has arcs or circles"
    );
    assert_eq!(
        reason(&OffsetError::Unsupported(vec![VertexId(1)]), Grow),
        "1 corner where the faces don't meet"
    );
    assert_eq!(
        reason(
            &OffsetError::Unsupported(vec![VertexId(1), VertexId(2), VertexId(5)]),
            Shrink
        ),
        "3 corners where the faces don't meet"
    );
    let collapse = OffsetError::Collapse {
        edges: vec![],
        faces: vec![],
    };
    assert_eq!(reason(&collapse, Shrink), "too thin for this clearance");
    assert_eq!(reason(&collapse, Grow), "a hole or cavity would close");
    assert_eq!(
        reason(&OffsetError::SelfIntersect, Grow),
        "a gap would close"
    );
    assert_eq!(
        reason(&OffsetError::SelfIntersect, Shrink),
        "a gap would close"
    );
    assert_eq!(
        reason(&OffsetError::BadDistance, Grow),
        "the clearance is not usable"
    );
    assert_eq!(
        reason(&OffsetError::MissingBody(BodyId::Root), Grow),
        "not found"
    );
    assert_eq!(UNEVEN_SCALE, "scaled unevenly");
}

// ---- OffsetOutcome::message ----

fn refused(name: &str, reason: &str) -> Refused {
    Refused {
        name: name.into(),
        reason: reason.into(),
    }
}

#[test]
fn the_message_says_what_changed_and_what_did_not() {
    let mut outcome = OffsetOutcome {
        direction: Direction::Grow,
        clearance_m: D,
        changed: 2,
        refused: vec![],
    };
    assert_eq!(outcome.message(), "Grew 2 parts by 0.2 mm");
    outcome.changed = 1;
    assert_eq!(outcome.message(), "Grew 1 part by 0.2 mm");
    outcome.direction = Direction::Shrink;
    outcome.clearance_m = 0.00025;
    assert_eq!(outcome.message(), "Shrank 1 part by 0.25 mm");
    outcome.refused = vec![
        refused("Lid", "has arcs or circles"),
        refused("Base", "a gap would close"),
    ];
    assert_eq!(
        outcome.message(),
        "Shrank 1 part by 0.25 mm. Not changed: Lid: has arcs or circles; Base: a gap would close"
    );
    outcome.changed = 0;
    assert_eq!(
        outcome.message(),
        "Nothing changed: Lid: has arcs or circles; Base: a gap would close"
    );
}

// ---- Editor::offset_instances ----

#[test]
fn growing_a_solid_box_is_one_undo_step() {
    let mut model = Model::new();
    let id = add_box(&mut model, BodyId::Root, S, &[], "Peg", DMat4::IDENTITY);
    let mut editor = Editor::new();
    select(&mut editor, &[id]);
    let outcome = editor
        .offset_instances(&mut model, Direction::Grow, D)
        .unwrap();
    assert_eq!(outcome.changed, 1);
    assert!(outcome.refused.is_empty());
    assert_eq!(outcome.direction, Direction::Grow);
    assert_eq!(outcome.message(), "Grew 1 part by 0.2 mm");
    assert!(near(volume(&model, BodyId::Root, id), cube(S + 2.0 * D)));
    assert_eq!(editor.message(), None);
    assert_eq!(editor.undo_stack().undo_count(), 1);
    assert_eq!(editor.undo_stack().undo_label(), Some("Grow by Clearance"));
    assert!(model.validate().is_empty());
    assert_eq!(editor.selection(), &[Entity::Instance(id)]);

    editor.key(EditKey::Undo, &mut model);
    assert!(near(volume(&model, BodyId::Root, id), cube(S)));
    editor.key(EditKey::Redo, &mut model);
    assert!(near(volume(&model, BodyId::Root, id), cube(S + 2.0 * D)));
}

#[test]
fn shrinking_a_solid_box() {
    let mut model = Model::new();
    let id = add_box(&mut model, BodyId::Root, S, &[], "Peg", DMat4::IDENTITY);
    let mut editor = Editor::new();
    select(&mut editor, &[id]);
    let outcome = editor
        .offset_instances(&mut model, Direction::Shrink, D)
        .unwrap();
    assert_eq!(outcome.message(), "Shrank 1 part by 0.2 mm");
    assert!(near(volume(&model, BodyId::Root, id), cube(S - 2.0 * D)));
    assert_eq!(
        editor.undo_stack().undo_label(),
        Some("Shrink by Clearance")
    );
    editor.key(EditKey::Undo, &mut model);
    assert!(near(volume(&model, BodyId::Root, id), cube(S)));
}

#[test]
fn a_solid_and_an_open_box_change_one_and_refuse_one() {
    let mut model = Model::new();
    let solid = add_box(&mut model, BodyId::Root, S, &[], "Peg", DMat4::IDENTITY);
    let open = add_box(
        &mut model,
        BodyId::Root,
        S,
        &[1],
        "Lid",
        DMat4::from_translation(v(0.1, 0.0, 0.0)),
    );
    let open_definition = definition_of(&model, BodyId::Root, open);
    let open_before = model.body(BodyId::Definition(open_definition)).cloned();
    let mut editor = Editor::new();
    select(&mut editor, &[open, solid]);
    let outcome = editor
        .offset_instances(&mut model, Direction::Grow, D)
        .unwrap();
    assert_eq!(outcome.changed, 1);
    assert_eq!(
        outcome.refused,
        vec![refused("Lid", "not a solid (try Repair)")]
    );
    assert_eq!(
        outcome.message(),
        "Grew 1 part by 0.2 mm. Not changed: Lid: not a solid (try Repair)"
    );
    assert!(near(volume(&model, BodyId::Root, solid), cube(S + 2.0 * D)));
    assert_eq!(
        model.body(BodyId::Definition(open_definition)).cloned(),
        open_before,
        "the refused part is untouched"
    );
    assert_eq!(editor.undo_stack().undo_count(), 1);
    editor.key(EditKey::Undo, &mut model);
    assert!(near(volume(&model, BodyId::Root, solid), cube(S)));
}

#[test]
fn two_solids_are_one_undo_step() {
    let mut model = Model::new();
    let a = add_box(&mut model, BodyId::Root, S, &[], "A", DMat4::IDENTITY);
    let b = add_box(
        &mut model,
        BodyId::Root,
        0.03,
        &[],
        "B",
        DMat4::from_translation(v(0.1, 0.0, 0.0)),
    );
    let mut editor = Editor::new();
    select(&mut editor, &[a, b]);
    let outcome = editor
        .offset_instances(&mut model, Direction::Grow, D)
        .unwrap();
    assert_eq!(outcome.message(), "Grew 2 parts by 0.2 mm");
    assert_eq!(editor.undo_stack().undo_count(), 1);
    editor.key(EditKey::Undo, &mut model);
    assert!(near(volume(&model, BodyId::Root, a), cube(S)));
    assert!(near(volume(&model, BodyId::Root, b), cube(0.03)));
}

#[test]
fn parts_of_different_scale_are_still_one_undo_step() {
    let mut model = Model::new();
    let a = add_box(&mut model, BodyId::Root, S, &[], "A", DMat4::IDENTITY);
    let b = add_box(
        &mut model,
        BodyId::Root,
        S,
        &[],
        "B",
        DMat4::from_translation(v(1.0, 0.0, 0.0)) * DMat4::from_scale(DVec3::splat(2.0)),
    );
    let mut editor = Editor::new();
    select(&mut editor, &[a, b]);
    let outcome = editor
        .offset_instances(&mut model, Direction::Grow, D)
        .unwrap();
    assert_eq!(outcome.changed, 2);
    assert!(near(volume(&model, BodyId::Root, a), cube(S + 2.0 * D)));
    assert!(near(volume(&model, BodyId::Root, b), cube(S + D)));
    assert_eq!(editor.undo_stack().undo_count(), 1);
    editor.key(EditKey::Undo, &mut model);
    assert!(near(volume(&model, BodyId::Root, a), cube(S)));
    assert!(near(volume(&model, BodyId::Root, b), cube(S)));
    editor.key(EditKey::Redo, &mut model);
    assert!(near(volume(&model, BodyId::Root, b), cube(S + D)));
}

#[test]
fn when_every_part_is_refused_nothing_is_recorded() {
    let mut model = Model::new();
    let open = add_box(&mut model, BodyId::Root, S, &[1], "Lid", DMat4::IDENTITY);
    let before = model.clone();
    let mut editor = Editor::new();
    select(&mut editor, &[open]);
    let outcome = editor
        .offset_instances(&mut model, Direction::Shrink, D)
        .unwrap();
    assert_eq!(outcome.changed, 0);
    assert_eq!(
        outcome.message(),
        "Nothing changed: Lid: not a solid (try Repair)"
    );
    assert!(model == before);
    assert_eq!(editor.undo_stack().undo_count(), 0);
}

#[test]
fn a_part_too_thin_for_the_clearance_is_refused() {
    let mut model = Model::new();
    let id = add_box(&mut model, BodyId::Root, S, &[], "Peg", DMat4::IDENTITY);
    let before = model.clone();
    let mut editor = Editor::new();
    select(&mut editor, &[id]);
    let outcome = editor
        .offset_instances(&mut model, Direction::Shrink, S) // twice the half-width
        .unwrap();
    assert_eq!(
        outcome.refused,
        vec![refused("Peg", "too thin for this clearance")]
    );
    assert!(model == before);
    assert_eq!(editor.undo_stack().undo_count(), 0);
}

#[test]
fn an_unnamed_instance_is_called_by_its_definition() {
    let mut model = Model::new();
    let id = add_box(&mut model, BodyId::Root, S, &[1], "", DMat4::IDENTITY);
    let mut editor = Editor::new();
    select(&mut editor, &[id]);
    let outcome = editor
        .offset_instances(&mut model, Direction::Grow, D)
        .unwrap();
    assert_eq!(outcome.refused[0].name, "Box");
}

#[test]
fn a_uniformly_scaled_instance_gets_the_distance_in_its_own_units() {
    let mut model = Model::new();
    let id = add_box(
        &mut model,
        BodyId::Root,
        S,
        &[],
        "Big",
        DMat4::from_translation(v(1.0, 0.0, 0.0)) * DMat4::from_scale(DVec3::splat(2.0)),
    );
    let mut editor = Editor::new();
    select(&mut editor, &[id]);
    editor
        .offset_instances(&mut model, Direction::Grow, D)
        .unwrap();
    // 0.2 mm in the world is 0.1 mm in the definition: each side gains 0.2 mm there.
    assert!(near(volume(&model, BodyId::Root, id), cube(S + D)));
}

#[test]
fn a_rotated_instance_is_as_good_as_a_plain_one() {
    let mut model = Model::new();
    let id = add_box(
        &mut model,
        BodyId::Root,
        S,
        &[],
        "Turned",
        DMat4::from_rotation_z(0.6),
    );
    let mut editor = Editor::new();
    select(&mut editor, &[id]);
    editor
        .offset_instances(&mut model, Direction::Grow, D)
        .unwrap();
    assert!(near(volume(&model, BodyId::Root, id), cube(S + 2.0 * D)));
}

#[test]
fn an_unevenly_scaled_instance_is_refused_without_the_kernel() {
    let mut model = Model::new();
    let uneven = add_box(
        &mut model,
        BodyId::Root,
        S,
        &[],
        "Squashed",
        DMat4::from_scale(v(1.0, 2.0, 1.0)),
    );
    let plain = add_box(
        &mut model,
        BodyId::Root,
        S,
        &[],
        "Plain",
        DMat4::from_translation(v(0.1, 0.0, 0.0)),
    );
    let mut editor = Editor::new();
    select(&mut editor, &[uneven, plain]);
    let outcome = editor
        .offset_instances(&mut model, Direction::Grow, D)
        .unwrap();
    assert_eq!(outcome.refused, vec![refused("Squashed", UNEVEN_SCALE)]);
    assert_eq!(outcome.changed, 1);
    assert!(near(volume(&model, BodyId::Root, uneven), cube(S)));
    assert!(near(volume(&model, BodyId::Root, plain), cube(S + 2.0 * D)));

    // Alone, nothing is recorded.
    let mut editor = Editor::new();
    select(&mut editor, &[uneven]);
    let outcome = editor
        .offset_instances(&mut model, Direction::Shrink, D)
        .unwrap();
    assert_eq!(
        outcome.message(),
        "Nothing changed: Squashed: scaled unevenly"
    );
    assert_eq!(editor.undo_stack().undo_count(), 0);
}

#[test]
fn copies_of_a_component_change_once_with_the_first_instances_scale() {
    let mut model = Model::new();
    let first = add_box(&mut model, BodyId::Root, S, &[], "One", DMat4::IDENTITY);
    let definition = definition_of(&model, BodyId::Root, first);
    let second = add_instance(
        &mut model,
        BodyId::Root,
        definition,
        "Two",
        DMat4::from_translation(v(0.1, 0.0, 0.0)) * DMat4::from_scale(DVec3::splat(2.0)),
    );
    let mut editor = Editor::new();
    select(&mut editor, &[first, second]);
    let outcome = editor
        .offset_instances(&mut model, Direction::Grow, D)
        .unwrap();
    assert_eq!(outcome.changed, 1);
    assert!(outcome.refused.is_empty());
    assert!(near(volume(&model, BodyId::Root, first), cube(S + 2.0 * D)));

    // Reversed, the 2x copy leads: half the distance.
    let mut model2 = Model::new();
    let first = add_box(&mut model2, BodyId::Root, S, &[], "One", DMat4::IDENTITY);
    let definition = definition_of(&model2, BodyId::Root, first);
    let second = add_instance(
        &mut model2,
        BodyId::Root,
        definition,
        "Two",
        DMat4::from_scale(DVec3::splat(2.0)),
    );
    let mut editor = Editor::new();
    select(&mut editor, &[second, first]);
    editor
        .offset_instances(&mut model2, Direction::Grow, D)
        .unwrap();
    assert!(near(volume(&model2, BodyId::Root, first), cube(S + D)));
}

#[test]
fn the_open_groups_parents_count_toward_the_scale() {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let outer = model.add_definition(
        DefinitionInfo {
            name: "Outer".into(),
            group: true,
        },
        &mut log,
    );
    let parent = add_instance(
        &mut model,
        BodyId::Root,
        outer,
        "Outer",
        DMat4::from_scale(DVec3::splat(2.0)),
    );
    let child = add_box(
        &mut model,
        BodyId::Definition(outer),
        S,
        &[],
        "Inner",
        DMat4::from_scale(DVec3::splat(2.0)), // 4x in the world
    );
    let mut editor = Editor::new();
    assert!(editor.open_instance(&model, parent));
    select(&mut editor, &[child]);
    let outcome = editor
        .offset_instances(&mut model, Direction::Grow, D)
        .unwrap();
    assert_eq!(outcome.changed, 1);
    // 0.2 mm / 4 = 0.05 mm each side.
    let definition = definition_of(&model, BodyId::Definition(outer), child);
    let volume = check_body(&model, BodyId::Definition(definition))
        .unwrap()
        .volume
        .unwrap();
    assert!(near(volume, cube(S + 2.0 * D / 4.0)));
}

#[test]
fn nothing_to_offset_is_an_error_and_changes_nothing() {
    let mut model = Model::new();
    let _id = add_box(&mut model, BodyId::Root, S, &[], "Peg", DMat4::IDENTITY);
    let before = model.clone();
    let mut editor = Editor::new();
    let message = editor
        .offset_instances(&mut model, Direction::Grow, D)
        .unwrap_err();
    assert_eq!(message, "Select a group or component first");
    assert_eq!(editor.message(), Some(message.as_str()));
    editor.select(vec![Entity::Face(FaceId(1))]);
    let message = editor
        .offset_instances(&mut model, Direction::Shrink, D)
        .unwrap_err();
    assert_eq!(message, "Select a group or component first");
    // An instance that is not in the open body does not count.
    editor.select(vec![Entity::Instance(InstanceId(999))]);
    assert!(
        editor
            .offset_instances(&mut model, Direction::Grow, D)
            .is_err()
    );
    assert!(model == before);
    assert_eq!(editor.undo_stack().undo_count(), 0);
}

#[test]
fn an_unusable_clearance_is_an_error() {
    let mut model = Model::new();
    let id = add_box(&mut model, BodyId::Root, S, &[], "Peg", DMat4::IDENTITY);
    let before = model.clone();
    let mut editor = Editor::new();
    select(&mut editor, &[id]);
    for bad in [0.0, -D, f64::NAN, f64::INFINITY] {
        let message = editor
            .offset_instances(&mut model, Direction::Grow, bad)
            .unwrap_err();
        assert_eq!(message, "The clearance is not usable", "{bad}");
    }
    assert!(model == before);
    assert_eq!(editor.undo_stack().undo_count(), 0);
}

#[test]
fn a_success_clears_an_earlier_message() {
    let mut model = Model::new();
    let id = add_box(&mut model, BodyId::Root, S, &[], "Peg", DMat4::IDENTITY);
    let mut editor = Editor::new();
    assert!(
        editor
            .offset_instances(&mut model, Direction::Grow, D)
            .is_err()
    );
    assert!(editor.message().is_some());
    select(&mut editor, &[id]);
    editor
        .offset_instances(&mut model, Direction::Grow, D)
        .unwrap();
    assert_eq!(editor.message(), None);
}

#[test]
fn a_change_makes_a_showing_fit_check_stale() {
    let mut model = Model::new();
    let a = add_box(&mut model, BodyId::Root, S, &[], "A", DMat4::IDENTITY);
    let b = add_box(
        &mut model,
        BodyId::Root,
        S,
        &[],
        "B",
        DMat4::from_translation(v(0.0205, 0.0, 0.0)), // 0.5 mm apart
    );
    let mut editor = Editor::new();
    select(&mut editor, &[a, b]);
    let state = editor.undo_stack().state();
    let fit = FitCheck::run(&model, editor.path(), editor.selection(), state, D).unwrap();
    assert!(fit.is_current(editor.path(), editor.selection(), state));
    editor
        .offset_instances(&mut model, Direction::Grow, D)
        .unwrap();
    let after = editor.undo_stack().state();
    assert_ne!(after, state);
    assert!(!fit.is_current(editor.path(), editor.selection(), after));
    // The path type is the same one the app keeps.
    let _: &EditPath = editor.path();
}

#[test]
fn a_fully_refused_offset_leaves_the_undo_state_alone() {
    let mut model = Model::new();
    let open = add_box(&mut model, BodyId::Root, S, &[1], "Lid", DMat4::IDENTITY);
    let mut editor = Editor::new();
    select(&mut editor, &[open]);
    let state = editor.undo_stack().state();
    editor
        .offset_instances(&mut model, Direction::Grow, D)
        .unwrap();
    assert_eq!(editor.undo_stack().state(), state);
}
