// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-solid-ui: Solid rows and the Repair button in Entity Info (driven with egui_kittest),
//! the repair through the editor, the repair message and the problem marks
//! (`docs/specs/m7-solid-ui.md`).

use std::collections::BTreeMap;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glam::{DMat4, DVec3};
use modeler::editor::Editor;
use modeler::entity_info::{EntityInfoPanel, InfoAction, entity_info_panel};
use modeler::input::EditKey;
use modeler::solid_ui::*;
use modeler_dae::{Batch, FlatScene, Material};
use modeler_kernel::editpath::EditPath;
use modeler_kernel::model::*;
use modeler_kernel::repair::{RepairReport, Unfixed};
use modeler_kernel::select::Entity;
use modeler_kernel::solid::SolidReport;
use modeler_kernel::topology::*;
use modeler_kernel::units::{LengthFormat, Unit};

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

/// A group in `body` holding a box of side `s` without the faces in `skip` and with the faces
/// in `flip` turned round, placed by `transform`.
fn add_box(
    model: &mut Model,
    body: BodyId,
    s: f64,
    skip: &[usize],
    flip: &[usize],
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
                let mut quad = quad.to_vec();
                if flip.contains(&n) {
                    quad.reverse();
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
    model
        .add_instance(
            body,
            Instance {
                definition,
                transform,
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap()
}

fn root_box(s: f64, skip: &[usize], flip: &[usize]) -> (Model, InstanceId) {
    let mut model = Model::new();
    let id = add_box(&mut model, BodyId::Root, s, skip, flip, DMat4::IDENTITY);
    (model, id)
}

fn mm(decimals: u8) -> LengthFormat {
    LengthFormat::Decimal {
        unit: Unit::Millimeter,
        decimals,
    }
}

fn status_of(model: &Model, id: InstanceId) -> SolidStatus {
    solid_status(model, BodyId::Root, &[Entity::Instance(id)]).unwrap()
}

fn row<'a>(rows: &'a [(String, String)], label: &str) -> Option<&'a str> {
    rows.iter()
        .find(|(l, _)| l == label)
        .map(|(_, value)| value.as_str())
}

// ---- solid_status ----

#[test]
fn the_status_needs_exactly_one_instance() {
    let (mut model, a) = root_box(1.0, &[], &[]);
    let b = add_box(&mut model, BodyId::Root, 1.0, &[], &[], DMat4::IDENTITY);
    assert!(solid_status(&model, BodyId::Root, &[]).is_none());
    let both = [Entity::Instance(a), Entity::Instance(b)];
    assert!(solid_status(&model, BodyId::Root, &both).is_none());
    let status = solid_status(&model, BodyId::Root, &[Entity::Instance(a)]).unwrap();
    assert_eq!(status.instance, a);
    assert_eq!(
        status.definition,
        model.root().instance(a).unwrap().definition
    );
    assert!(status.report.is_solid());
    let missing = [Entity::Instance(InstanceId(999))];
    assert!(solid_status(&model, BodyId::Root, &missing).is_none());
    let other_body = BodyId::Definition(status.definition);
    assert!(solid_status(&model, other_body, &[Entity::Instance(a)]).is_none());
}

#[test]
fn repair_is_for_anything_not_clean() {
    let (model, id) = root_box(1.0, &[], &[]);
    assert!(!can_repair(&status_of(&model, id)));
    let (model, id) = root_box(1.0, &[1], &[]);
    assert!(can_repair(&status_of(&model, id)));
    let (model, id) = root_box(1.0, &[], &[4]);
    let status = status_of(&model, id);
    assert!(status.report.is_solid(), "a turned face still closes it");
    assert!(can_repair(&status));
}

// ---- format_volume ----

#[test]
fn volumes_are_in_the_cube_of_the_unit() {
    assert_eq!(format_volume(1e-6, mm(2)), "1000mm³");
    assert_eq!(
        format_volume(
            1e-6,
            LengthFormat::Decimal {
                unit: Unit::Centimeter,
                decimals: 2
            }
        ),
        "1cm³"
    );
    assert_eq!(
        format_volume(
            1.5,
            LengthFormat::Decimal {
                unit: Unit::Meter,
                decimals: 3
            }
        ),
        "1.5m³"
    );
    assert_eq!(
        format_volume(
            1.234_567e-6,
            LengthFormat::Decimal {
                unit: Unit::Centimeter,
                decimals: 2
            }
        ),
        "1.23cm³"
    );
    let inch = 0.0254f64;
    assert_eq!(
        format_volume(
            inch * inch * inch * 8.0,
            LengthFormat::Decimal {
                unit: Unit::Inch,
                decimals: 2
            }
        ),
        "8in³"
    );
    let foot = 0.3048f64;
    assert_eq!(
        format_volume(
            foot * foot * foot * 2.5,
            LengthFormat::Decimal {
                unit: Unit::Foot,
                decimals: 2
            }
        ),
        "2.5ft³"
    );
    assert_eq!(
        format_volume(
            foot * foot * foot * 2.0,
            LengthFormat::Architectural { denominator: 16 }
        ),
        "2ft³"
    );
    assert_eq!(format_volume(0.0, mm(2)), "0mm³");
    assert_eq!(format_volume(f64::NAN, mm(2)), "0mm³");
}

// ---- status_rows ----

#[test]
fn a_solid_shows_yes_and_its_volume() {
    let (model, id) = root_box(0.01, &[], &[]);
    let rows = status_rows(&status_of(&model, id), mm(2));
    assert_eq!(
        rows,
        vec![
            ("Solid".to_string(), "Yes".to_string()),
            ("Volume".to_string(), "1000mm³".to_string()),
        ]
    );
}

#[test]
fn an_open_box_shows_its_counts() {
    let (model, id) = root_box(0.01, &[1], &[]);
    let rows = status_rows(&status_of(&model, id), mm(2));
    assert_eq!(
        rows,
        vec![
            ("Solid".to_string(), "No".to_string()),
            ("Open edges".to_string(), "4".to_string()),
        ]
    );
}

#[test]
fn a_solid_with_a_turned_face_says_so() {
    let (model, id) = root_box(0.01, &[], &[4]);
    let rows = status_rows(&status_of(&model, id), mm(2));
    assert_eq!(row(&rows, "Solid"), Some("Yes"));
    assert_eq!(row(&rows, "Volume"), Some("1000mm³"));
    assert_eq!(row(&rows, "Reversed faces"), Some("1"));
    assert_eq!(row(&rows, "Shells"), None);
}

#[test]
fn rows_list_every_problem_in_order() {
    let status = SolidStatus {
        instance: InstanceId(1),
        definition: DefinitionId(1),
        report: SolidReport {
            shells: 2,
            open_edges: vec![EdgeId(1), EdgeId(2)],
            loose_edges: vec![EdgeId(3)],
            shared_edges: vec![EdgeId(4), EdgeId(5), EdgeId(6)],
            internal_faces: vec![FaceId(1)],
            reversed_faces: vec![FaceId(2), FaceId(3)],
            nested_instances: 4,
            volume: None,
        },
    };
    let rows = status_rows(&status, mm(2));
    let labels: Vec<&str> = rows.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            "Solid",
            "Open edges",
            "Loose edges",
            "Shared edges",
            "Reversed faces",
            "Internal faces",
            "Nested",
            "Shells"
        ]
    );
    let values: Vec<&str> = rows.iter().map(|(_, v)| v.as_str()).collect();
    assert_eq!(values, vec!["No", "2", "1", "3", "2", "1", "4", "2"]);
}

#[test]
fn an_empty_group_is_not_a_solid_with_no_shells() {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let definition = model.add_definition(
        DefinitionInfo {
            name: "Empty".into(),
            group: true,
        },
        &mut log,
    );
    let id = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition,
                transform: DMat4::IDENTITY,
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    let rows = status_rows(&status_of(&model, id), mm(2));
    assert_eq!(
        rows,
        vec![
            ("Solid".to_string(), "No".to_string()),
            ("Shells".to_string(), "0".to_string()),
        ]
    );
}

#[test]
fn two_shells_that_are_each_closed_show_the_count() {
    let mut model = Model::new();
    let first = add_box(&mut model, BodyId::Root, 1.0, &[], &[], DMat4::IDENTITY);
    // A second box in the same definition.
    let definition = model.root().instance(first).unwrap().definition;
    let mut log = ModelEdit::new();
    model
        .edit_geometry(BodyId::Definition(definition), &mut log, |ctx, edit| {
            let corners = [
                v(5.0, 0.0, 0.0),
                v(6.0, 0.0, 0.0),
                v(6.0, 1.0, 0.0),
                v(5.0, 1.0, 0.0),
                v(5.0, 0.0, 1.0),
                v(6.0, 0.0, 1.0),
                v(6.0, 1.0, 1.0),
                v(5.0, 1.0, 1.0),
            ];
            let vs: Vec<VertexId> = corners
                .iter()
                .map(|&p| ctx.insert_vertex(p, edit))
                .collect();
            let mut edges = BTreeMap::new();
            for quad in FACES {
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
    let rows = status_rows(&status_of(&model, first), mm(2));
    assert_eq!(row(&rows, "Solid"), Some("Yes"));
    assert_eq!(row(&rows, "Shells"), Some("2"));
    assert_eq!(row(&rows, "Volume"), Some("2000000000mm³"));
}

// ---- repair_message ----

fn report(f: impl FnOnce(&mut RepairReport)) -> RepairReport {
    let mut r = RepairReport::default();
    f(&mut r);
    r
}

#[test]
fn the_message_says_what_was_done() {
    let r = report(|r| r.reversed = vec![FaceId(1)]);
    assert_eq!(repair_message(&r), "Repaired: turned 1 face");
    let r = report(|r| {
        r.removed_internal = vec![FaceId(1), FaceId(2)];
        r.removed_stray = vec![FaceId(3)];
        r.removed_edges = vec![EdgeId(1)];
        r.filled = vec![FaceId(4), FaceId(5)];
        r.reversed = vec![FaceId(6), FaceId(7), FaceId(8)];
    });
    assert_eq!(
        repair_message(&r),
        "Repaired: removed 3 faces, removed 1 edge, filled 2 holes, turned 3 faces"
    );
    let r = report(|r| r.filled = vec![FaceId(1)]);
    assert_eq!(repair_message(&r), "Repaired: filled 1 hole");
}

#[test]
fn the_message_for_nothing_to_do() {
    assert_eq!(
        repair_message(&RepairReport::default()),
        "Nothing to repair"
    );
}

#[test]
fn the_message_lists_what_it_could_not_fix() {
    let r = report(|r| {
        r.filled = vec![FaceId(1)];
        r.unfixed = vec![
            Unfixed::NotOneLoop(vec![EdgeId(1)]),
            Unfixed::NotOneLoop(vec![EdgeId(2)]),
            Unfixed::NotFlat(vec![EdgeId(3)]),
            Unfixed::SharedEdge(EdgeId(4)),
            Unfixed::NestedInstances(3),
        ];
    });
    assert_eq!(
        repair_message(&r),
        "Repaired: filled 1 hole; could not fix: open borders not forming one loop: 2, \
         holes that aren't flat: 1, edges used by three or more faces: 1, \
         groups or components inside: 3"
    );
    let r = report(|r| {
        r.unfixed = vec![
            Unfixed::Nested(vec![EdgeId(1)]),
            Unfixed::BadFill(vec![EdgeId(2)]),
            Unfixed::Covered(vec![EdgeId(3)]),
        ];
    });
    assert_eq!(
        repair_message(&r),
        "Nothing could be repaired; could not fix: ring-shaped holes: 1, \
         fills that aren't valid faces: 1, fills that would overlap a face: 1"
    );
}

// ---- Editor::repair_instance ----

#[test]
fn repairing_a_turned_face_is_one_undo_step() {
    let (mut model, id) = root_box(1.0, &[], &[4]);
    let mut editor = Editor::new();
    let report = editor.repair_instance(&mut model, id).unwrap();
    assert_eq!(report.reversed.len(), 1);
    assert!(report.after.is_clean());
    assert_eq!(repair_message(&report), "Repaired: turned 1 face");
    assert_eq!(
        editor.message(),
        None,
        "the app shows the note, not the editor"
    );
    assert_eq!(editor.undo_stack().undo_count(), 1);
    assert_eq!(editor.undo_stack().undo_label(), Some("Repair Solid"));
    assert!(status_of(&model, id).report.is_clean());
    editor.key(EditKey::Undo, &mut model);
    assert_eq!(status_of(&model, id).report.reversed_faces.len(), 1);
    assert!(model.validate().is_empty());
}

#[test]
fn repairing_fills_a_flat_hole() {
    let (mut model, id) = root_box(1.0, &[1], &[]);
    let mut editor = Editor::new();
    let report = editor.repair_instance(&mut model, id).unwrap();
    assert_eq!(report.filled.len(), 1);
    assert_eq!(repair_message(&report), "Repaired: filled 1 hole");
    let status = status_of(&model, id);
    assert!(status.report.is_clean());
    assert!((status.report.volume.unwrap() - 1.0).abs() < 1e-9);
}

#[test]
fn repairing_a_clean_box_changes_nothing() {
    let (mut model, id) = root_box(1.0, &[], &[]);
    let mut editor = Editor::new();
    let report = editor.repair_instance(&mut model, id).unwrap();
    assert!(!report.changed());
    assert_eq!(repair_message(&report), "Nothing to repair");
    assert_eq!(editor.undo_stack().undo_count(), 0);
}

#[test]
fn what_cannot_be_fixed_is_reported() {
    // Two neighbouring faces missing: the hole's border is not flat.
    let (mut model, id) = root_box(1.0, &[1, 4], &[]);
    let mut editor = Editor::new();
    let report = editor.repair_instance(&mut model, id).unwrap();
    assert!(!report.changed());
    assert_eq!(
        repair_message(&report),
        "Nothing could be repaired; could not fix: holes that aren't flat: 1"
    );
    assert_eq!(editor.undo_stack().undo_count(), 0);
}

#[test]
fn an_instance_that_is_not_in_the_open_body_is_refused() {
    let (mut model, id) = root_box(1.0, &[1], &[]);
    let mut editor = Editor::new();
    let err = editor
        .repair_instance(&mut model, InstanceId(id.0 + 50))
        .unwrap_err();
    assert_eq!(err, "Select a group or component to repair");
    assert_eq!(editor.message(), Some(err.as_str()));
    assert_eq!(editor.undo_stack().undo_count(), 0);
}

// ---- Entity Info panel ----

struct State {
    panel: EntityInfoPanel,
    model: Model,
    selection: Vec<Entity>,
    actions: Vec<InfoAction>,
}

fn harness(model: Model, selection: Vec<Entity>) -> Harness<'static, State> {
    let mut model = model;
    model
        .set_length_format(mm(2), &mut ModelEdit::new())
        .unwrap();
    let mut h = Harness::new_ui_state(
        |ui, state: &mut State| {
            if let Some(action) = entity_info_panel(
                ui,
                &mut state.panel,
                &state.model,
                BodyId::Root,
                &state.selection,
            ) {
                state.actions.push(action);
            }
        },
        State {
            panel: EntityInfoPanel::default(),
            model,
            selection,
            actions: Vec::new(),
        },
    );
    h.run();
    h
}

#[test]
fn the_panel_shows_a_solid_with_its_volume_and_a_disabled_repair() {
    let (model, id) = root_box(0.01, &[], &[]);
    let mut h = harness(model, vec![Entity::Instance(id)]);
    h.get_by_label("Solid");
    h.get_by_label("Yes");
    h.get_by_label("Volume");
    h.get_by_label("1000mm³");
    h.get_by_label("Repair").click();
    h.run();
    assert!(h.state().actions.is_empty(), "nothing to repair");
}

#[test]
fn the_panel_shows_an_open_box_and_repair_asks_for_a_repair() {
    let (model, id) = root_box(0.01, &[1], &[]);
    let mut h = harness(model, vec![Entity::Instance(id)]);
    h.get_by_label("Solid");
    h.get_by_label("No");
    h.get_by_label("Open edges");
    h.get_by_label("4");
    assert!(
        h.query_by_label("Shells").is_none(),
        "one shell is not news"
    );
    assert!(h.query_by_label("Volume").is_none());
    h.get_by_label("Repair").click();
    h.run();
    assert_eq!(h.state().actions, vec![InfoAction::Repair(id)]);
}

#[test]
fn no_solid_rows_for_other_selections() {
    let (mut model, a) = root_box(0.01, &[1], &[]);
    let b = add_box(&mut model, BodyId::Root, 0.01, &[], &[], DMat4::IDENTITY);
    let h = harness(
        model.clone(),
        vec![Entity::Instance(a), Entity::Instance(b)],
    );
    assert!(h.query_by_label("Solid").is_none());
    assert!(h.query_by_label("Repair").is_none());
    let h = harness(model, vec![]);
    assert!(h.query_by_label("Solid").is_none());
    assert!(h.query_by_label("Repair").is_none());
}

#[test]
fn the_check_is_not_redone_every_frame() {
    let (model, a) = root_box(0.01, &[1], &[]);
    let mut h = harness(model, vec![Entity::Instance(a)]);
    for _ in 0..5 {
        h.run();
    }
    assert_eq!(h.state().panel.solid.computed(), 1);
}

#[test]
fn the_cache_follows_the_selection() {
    let (mut model, a) = root_box(1.0, &[1], &[]);
    let b = add_box(&mut model, BodyId::Root, 1.0, &[], &[], DMat4::IDENTITY);
    let mut cache = SolidCache::default();
    assert_eq!(
        cache
            .get(&model, BodyId::Root, &[Entity::Instance(a)])
            .map(|s| s.instance),
        Some(a)
    );
    assert_eq!(
        cache
            .get(&model, BodyId::Root, &[Entity::Instance(a)])
            .map(|s| s.instance),
        Some(a)
    );
    assert_eq!(cache.computed(), 1);
    assert_eq!(
        cache
            .get(&model, BodyId::Root, &[Entity::Instance(b)])
            .map(|s| s.instance),
        Some(b)
    );
    assert_eq!(cache.computed(), 2);
    assert!(cache.get(&model, BodyId::Root, &[]).is_none());
    assert_eq!(cache.computed(), 3);
}

// ---- MarkTracker and marked_scene ----

#[test]
fn marks_follow_the_selection_and_the_edit() {
    let (model, id) = root_box(1.0, &[1], &[]);
    let root = EditPath::root();
    let mut tracker = MarkTracker::default();
    assert!(tracker.marks().is_none());

    let selected = [Entity::Instance(id)];
    assert!(tracker.update(&model, &root, &selected, 0), "marks appear");
    let marks = tracker.marks().unwrap().clone();
    assert_eq!(marks.segments.len(), 4);
    assert!(!marks.is_empty());

    assert!(
        !tracker.update(&model, &root, &selected, 0),
        "nothing changed"
    );
    assert!(
        !tracker.update(&model, &root, &selected, 1),
        "the model was edited but the marks are the same"
    );
    assert_eq!(tracker.marks(), Some(&marks));

    assert!(
        tracker.update(&model, &root, &[], 1),
        "deselected: marks go"
    );
    assert!(tracker.marks().is_none());
    assert!(!tracker.update(&model, &root, &[], 2));

    assert!(tracker.update(&model, &root, &selected, 2));
    assert!(tracker.clear());
    assert!(tracker.marks().is_none());
    assert!(!tracker.clear());
    assert!(
        tracker.update(&model, &root, &selected, 2),
        "cleared: recomputed"
    );
}

#[test]
fn a_clean_part_has_no_marks() {
    let (model, id) = root_box(1.0, &[], &[]);
    let mut tracker = MarkTracker::default();
    let selected = [Entity::Instance(id)];
    assert!(!tracker.update(&model, &EditPath::root(), &selected, 0));
    assert!(tracker.marks().is_none());
}

#[test]
fn marks_after_a_repair_are_gone() {
    let (mut model, id) = root_box(1.0, &[1], &[]);
    let mut editor = Editor::new();
    let mut tracker = MarkTracker::default();
    let selected = [Entity::Instance(id)];
    assert!(tracker.update(
        &model,
        &EditPath::root(),
        &selected,
        editor.undo_stack().state()
    ));
    editor.repair_instance(&mut model, id).unwrap();
    assert!(tracker.update(
        &model,
        &EditPath::root(),
        &selected,
        editor.undo_stack().state()
    ));
    assert!(tracker.marks().is_none());
    editor.key(EditKey::Undo, &mut model);
    assert!(tracker.update(
        &model,
        &EditPath::root(),
        &selected,
        editor.undo_stack().state()
    ));
    assert_eq!(tracker.marks().unwrap().segments.len(), 4);
}

#[test]
fn marks_inside_an_open_group_are_placed_by_the_path() {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let outer_def = model.add_definition(
        DefinitionInfo {
            name: "Outer".into(),
            group: true,
        },
        &mut log,
    );
    let inner = add_box(
        &mut model,
        BodyId::Definition(outer_def),
        1.0,
        &[1],
        &[],
        DMat4::IDENTITY,
    );
    let outer = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: outer_def,
                transform: DMat4::from_translation(v(0.0, 0.0, 10.0)),
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    let path = EditPath::root().open(&model, outer).unwrap();
    let mut tracker = MarkTracker::default();
    assert!(tracker.update(&model, &path, &[Entity::Instance(inner)], 0));
    let marks = tracker.marks().unwrap();
    assert!(
        marks
            .segments
            .iter()
            .flatten()
            .all(|p| (p.z - 11.0).abs() < 1e-9)
    );
    // The inner instance is not in the root body.
    assert!(tracker.update(&model, &EditPath::root(), &[Entity::Instance(inner)], 0));
    assert!(tracker.marks().is_none());
}

#[test]
fn marked_scene_borrows_without_marks() {
    let scene = FlatScene {
        materials: vec![Material {
            id: "grey".into(),
            name: "grey".into(),
            color: [0.8; 4],
            texture: None,
            opacity: 1.0,
            unlit: false,
        }],
        batches: vec![Batch {
            material: Some(0),
            positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            texcoords: vec![],
            indices: vec![0, 1, 2],
        }],
        lines: vec![],
        bounds: None,
    };
    assert!(matches!(
        marked_scene(&scene, None),
        std::borrow::Cow::Borrowed(_)
    ));
    let marks = modeler_dae::SolidMarks {
        triangles: vec![[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0)]],
        segments: vec![],
    };
    let shown = marked_scene(&scene, Some(&marks));
    assert!(matches!(shown, std::borrow::Cow::Owned(_)));
    assert_eq!(shown.materials.len(), 2);
    assert_eq!(shown.materials[1].name, "Problem");
}
