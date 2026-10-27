// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-components-panel: the Components panel, driven with egui_kittest.
//! See docs/specs/m6-components-panel.md.

use eframe::egui;
use eframe::egui::accesskit::{Role, Toggled};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use glam::DVec3;
use modeler::components_panel::*;
use modeler::icons::BUTTON_SIZE;
use modeler_kernel::model::*;

const WIDTH: f32 = 200.0;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// Add a definition drawn as the closed polygon `pts` (none = no geometry).
fn define(model: &mut Model, name: &str, group: bool, pts: &[DVec3]) -> DefinitionId {
    let mut log = ModelEdit::new();
    let id = model.add_definition(
        DefinitionInfo {
            name: name.to_string(),
            group,
        },
        &mut log,
    );
    model
        .edit_geometry(BodyId::Definition(id), &mut log, |ctx, edit| {
            for i in 0..pts.len() {
                ctx.add_edge(pts[i], pts[(i + 1) % pts.len()], edit)
                    .unwrap();
            }
        })
        .unwrap();
    id
}

fn square() -> Vec<DVec3> {
    vec![
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(0.0, 1.0, 0.0),
    ]
}

fn sample() -> (Model, [DefinitionId; 4]) {
    let mut model = Model::new();
    let door = define(&mut model, "door", false, &square());
    let chair = define(&mut model, "Chair", false, &square());
    let table = define(&mut model, "Table", false, &square());
    let group = define(&mut model, "Fence group", true, &square());
    (model, [door, chair, table, group])
}

#[test]
fn listed_has_components_only_in_name_order_ignoring_case() {
    let (model, [door, chair, table, _]) = sample();
    assert_eq!(listed(&model, ""), vec![chair, door, table]);
}

#[test]
fn listed_filters_by_substring_ignoring_case() {
    let (model, [door, chair, table, _]) = sample();
    assert_eq!(listed(&model, "TA"), vec![table]);
    assert_eq!(listed(&model, "o"), vec![door]);
    assert_eq!(listed(&model, "a"), vec![chair, table]);
    assert!(listed(&model, "fence").is_empty(), "groups never listed");
    assert!(listed(&model, "zzz").is_empty());
}

#[test]
fn listed_breaks_name_ties_by_id() {
    let mut model = Model::new();
    let first = define(&mut model, "Same", false, &square());
    let second = define(&mut model, "same", false, &square());
    assert_eq!(listed(&model, ""), vec![first, second]);
}

fn extent(segments: &[[egui::Pos2; 2]]) -> (egui::Pos2, egui::Pos2) {
    let mut lo = egui::pos2(f32::MAX, f32::MAX);
    let mut hi = egui::pos2(f32::MIN, f32::MIN);
    for p in segments.iter().flatten() {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    (lo, hi)
}

#[test]
fn thumbnail_has_one_segment_per_edge_filling_the_unit_square() {
    let mut model = Model::new();
    let id = define(&mut model, "box", false, &square());
    let segments = thumbnail_segments(&model, id);
    assert_eq!(segments.len(), 4);
    let (lo, hi) = extent(&segments);
    let (w, h) = (hi.x - lo.x, hi.y - lo.y);
    assert!((w.max(h) - 1.0).abs() < 1e-5, "larger extent is 1: {w} {h}");
    assert!(lo.x >= -1e-5 && lo.y >= -1e-5 && hi.x <= 1.0 + 1e-5 && hi.y <= 1.0 + 1e-5);
    // The larger axis starts at 0, the other is centred.
    if w >= h {
        assert!(lo.x.abs() < 1e-5 && (lo.y + hi.y - 1.0).abs() < 1e-5);
    } else {
        assert!(lo.y.abs() < 1e-5 && (lo.x + hi.x - 1.0).abs() < 1e-5);
    }
}

#[test]
fn thumbnail_projects_isometrically_with_z_vertical() {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let id = model.add_definition(
        DefinitionInfo {
            name: "post".into(),
            group: false,
        },
        &mut log,
    );
    model
        .edit_geometry(BodyId::Definition(id), &mut log, |ctx, edit| {
            ctx.add_edge(v(0.0, 0.0, 0.0), v(0.0, 0.0, 1.0), edit)
                .unwrap();
        })
        .unwrap();
    let segments = thumbnail_segments(&model, id);
    assert_eq!(segments.len(), 1);
    let [a, b] = segments[0];
    assert!((a.x - b.x).abs() < 1e-5, "z edge is vertical");
    assert!(
        (a.y - b.y).abs() > 0.99,
        "and spans the whole drawing: {a:?} {b:?}"
    );
    // Higher z is higher on screen (smaller y).
    let ends = model
        .body(BodyId::Definition(id))
        .unwrap()
        .geometry()
        .edges()
        .next()
        .unwrap()
        .1
        .ends;
    let z = |vertex: modeler_kernel::topology::VertexId| -> f64 {
        model
            .body(BodyId::Definition(id))
            .unwrap()
            .geometry()
            .vertex(vertex)
            .unwrap()
            .position
            .z
    };
    let (top, bottom) = if z(ends[0]) > z(ends[1]) {
        (a, b)
    } else {
        (b, a)
    };
    assert!(top.y < bottom.y);
}

#[test]
fn equal_x_and_y_edges_project_to_equal_lengths() {
    let mut model = Model::new();
    let id = define(&mut model, "box", false, &square());
    let segments = thumbnail_segments(&model, id);
    let lengths: Vec<f32> = segments.iter().map(|s| (s[1] - s[0]).length()).collect();
    for l in &lengths {
        assert!((l - lengths[0]).abs() < 1e-4, "{lengths:?}");
    }
}

#[test]
fn thumbnail_of_nothing_is_empty() {
    let mut model = Model::new();
    let empty = define(&mut model, "empty", false, &[]);
    assert!(thumbnail_segments(&model, empty).is_empty());
    assert!(thumbnail_segments(&model, DefinitionId(999)).is_empty());
}

struct State {
    panel: ComponentsPanel,
    model: Model,
    actions: Vec<ComponentAction>,
}

fn harness(model: Model, tiles: bool) -> Harness<'static, State> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(600.0, 800.0))
        .build_ui_state(
            |ui, state: &mut State| {
                ui.allocate_ui(egui::vec2(WIDTH, 600.0), |ui| {
                    ui.set_max_width(WIDTH);
                    if let Some(action) = components_panel(ui, &mut state.panel, &state.model) {
                        state.actions.push(action);
                    }
                });
            },
            State {
                panel: ComponentsPanel {
                    tiles,
                    ..ComponentsPanel::default()
                },
                model,
                actions: Vec::new(),
            },
        );
    h.run();
    h
}

fn selected(h: &Harness<'_, State>, label: &str) -> bool {
    h.get_by_label(label).accesskit_node().toggled() == Some(Toggled::True)
}

#[test]
fn list_has_a_row_per_component_in_order() {
    let (model, _) = sample();
    let h = harness(model, false);
    assert!(h.query_by_label("Components").is_some());
    let chair = h.get_by_label("Chair").rect();
    let door = h.get_by_label("door").rect();
    let table = h.get_by_label("Table").rect();
    assert!(chair.bottom() <= door.top() + 0.5 && door.bottom() <= table.top() + 0.5);
    assert!(h.query_by_label("Fence group").is_none());
    for r in [chair, door, table] {
        assert!(
            r.height() >= LIST_THUMB - 0.5,
            "row holds the picture: {r:?}"
        );
        assert!(r.width() <= WIDTH + 0.5);
    }
}

#[test]
fn tiles_are_bigger_and_wrap() {
    let (model, _) = sample();
    let h = harness(model, true);
    let chair = h.get_by_label("Chair").rect();
    let door = h.get_by_label("door").rect();
    let table = h.get_by_label("Table").rect();
    for r in [chair, door, table] {
        assert!(r.width() >= TILE_THUMB - 0.5 && r.height() >= TILE_THUMB - 0.5);
        assert!(r.right() <= WIDTH + 0.5, "inside the panel: {r:?}");
    }
    // 200 points hold two or three 64-point tiles, not four: the third or later wraps.
    assert!(
        (chair.center().y - door.center().y).abs() < 1.0,
        "first two share a row"
    );
    assert!(door.left() > chair.left(), "left to right");
    assert!(
        table.top() >= chair.top() - 0.5 && table.left() >= 0.0,
        "{table:?}"
    );
}

#[test]
fn heading_search_and_switch_are_stacked() {
    let (model, _) = sample();
    let h = harness(model, false);
    let heading = h.get_by_label("Components").rect();
    let search = h.get_by_role(Role::TextInput).rect();
    let list = h.get_by_label("List");
    let tiles = h.get_by_label("Tiles");
    for node in [&list, &tiles] {
        let r = node.rect();
        assert!((r.width() - BUTTON_SIZE).abs() < 0.5 && (r.height() - BUTTON_SIZE).abs() < 0.5);
    }
    assert!((list.rect().center().y - tiles.rect().center().y).abs() < 0.5);
    assert!(list.rect().right() <= tiles.rect().left());
    assert!(heading.bottom() <= search.top() + 0.5);
    assert!(search.bottom() <= list.rect().top() + 0.5);
    assert!(tiles.rect().bottom() <= h.get_by_label("Chair").rect().top() + 0.5);
}

#[test]
fn switch_shows_and_sets_the_mode() {
    let (model, _) = sample();
    let mut h = harness(model, false);
    assert!(selected(&h, "List") && !selected(&h, "Tiles"));
    h.get_by_label("Tiles").click();
    h.run();
    assert!(h.state().panel.tiles);
    assert!(!selected(&h, "List") && selected(&h, "Tiles"));
    h.get_by_label("List").click();
    h.run();
    assert!(!h.state().panel.tiles);
}

#[test]
fn switch_tooltips_show_on_hover() {
    for (label, tooltip) in [("List", "Show as a list"), ("Tiles", "Show as tiles")] {
        let (model, _) = sample();
        let mut h = harness(model, false);
        h.get_by_label(label).hover();
        for _ in 0..90 {
            h.step();
        }
        h.run();
        assert!(h.query_by_label(tooltip).is_some(), "{label}");
    }
}

#[test]
fn clicking_an_entry_asks_to_place_it() {
    for tiles in [false, true] {
        let (model, [door, chair, _, _]) = sample();
        let mut h = harness(model, tiles);
        h.get_by_label("door").click();
        h.run();
        h.get_by_label("Chair").click();
        h.run();
        assert_eq!(
            h.state().actions,
            vec![ComponentAction::Place(door), ComponentAction::Place(chair)],
            "tiles: {tiles}"
        );
    }
}

#[test]
fn typing_in_the_search_filters_the_entries() {
    let (model, _) = sample();
    let mut h = harness(model, false);
    h.get_by_role(Role::TextInput).click();
    h.get_by_role(Role::TextInput).type_text("ta");
    h.run();
    assert_eq!(h.state().panel.search, "ta");
    assert!(h.query_by_label("Table").is_some());
    assert!(h.query_by_label("Chair").is_none());
    assert!(h.query_by_label("door").is_none());
}

#[test]
fn empty_messages() {
    let h = harness(Model::new(), false);
    assert!(h.query_by_label("No components").is_some());
    assert!(h.query_by_label("No matches").is_none());

    let (model, _) = sample();
    let mut h = harness(model, false);
    h.state_mut().panel.search = "zzz".into();
    h.run();
    assert!(h.query_by_label("No matches").is_some());
    assert!(h.query_by_label("No components").is_none());

    // Only groups: no components.
    let mut model = Model::new();
    define(&mut model, "Only group", true, &square());
    let h = harness(model, false);
    assert!(h.query_by_label("No components").is_some());
}

#[test]
fn nothing_is_returned_without_a_click() {
    let (model, _) = sample();
    let h = harness(model, false);
    assert!(h.state().actions.is_empty());
}
