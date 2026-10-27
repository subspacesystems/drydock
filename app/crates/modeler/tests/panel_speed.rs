// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-panel-speed: Entity Info and Components don't redo their work every frame.
//! See docs/specs/m6-panel-speed.md.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glam::DVec3;
use modeler::components_panel::*;
use modeler::entity_info::*;
use modeler_kernel::component::make_component;
use modeler_kernel::model::*;
use modeler_kernel::select::Entity;
use modeler_kernel::units::Unit;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// Draw the closed polygon into `body`.
fn polygon(model: &mut Model, body: BodyId, pts: &[DVec3]) {
    let mut log = ModelEdit::new();
    model
        .edit_geometry(body, &mut log, |ctx, edit| {
            for i in 0..pts.len() {
                ctx.add_edge(pts[i], pts[(i + 1) % pts.len()], edit)
                    .unwrap();
            }
        })
        .unwrap();
}

fn rect(x0: f64, y0: f64, w: f64, h: f64) -> Vec<DVec3> {
    vec![
        v(x0, y0, 0.0),
        v(x0 + w, y0, 0.0),
        v(x0 + w, y0 + h, 0.0),
        v(x0, y0 + h, 0.0),
    ]
}

fn circle(cx: f64, cy: f64, r: f64, n: usize) -> Vec<DVec3> {
    (0..n)
        .map(|i| {
            let a = i as f64 / n as f64 * std::f64::consts::TAU;
            v(cx + r * a.cos(), cy + r * a.sin(), 0.0)
        })
        .collect()
}

fn entities(model: &Model, body: BodyId) -> Vec<Entity> {
    let geometry = model.body(body).unwrap().geometry();
    let mut out: Vec<Entity> = geometry.edges().map(|(id, _)| Entity::Edge(id)).collect();
    out.extend(geometry.faces().map(|(id, _)| Entity::Face(id)));
    out.sort();
    out
}

/// A root rectangle at x 0..2 plus one component instance of a rectangle at x 5..8.
fn sample() -> (Model, InstanceId) {
    let mut model = Model::new();
    polygon(&mut model, BodyId::Root, &rect(5.0, 0.0, 3.0, 1.0));
    let selection = entities(&model, BodyId::Root);
    let mut undo = UndoStack::new();
    let instance = make_component(
        &mut model,
        BodyId::Root,
        &selection,
        DefinitionInfo {
            name: "Box".to_string(),
            group: false,
        },
        &mut undo,
        "Make Component",
    )
    .unwrap();
    polygon(&mut model, BodyId::Root, &rect(0.0, 0.0, 2.0, 1.0));
    (model, instance)
}

fn define(model: &mut Model, name: &str, pts: &[&[DVec3]]) -> DefinitionId {
    let mut log = ModelEdit::new();
    let id = model.add_definition(
        DefinitionInfo {
            name: name.to_string(),
            group: false,
        },
        &mut log,
    );
    for poly in pts {
        polygon(model, BodyId::Definition(id), poly);
    }
    id
}

// ---------------------------------------------------------------------------------------
// DescribeCache

#[test]
fn describe_cache_gives_what_describe_gives() {
    let (model, instance) = sample();
    let mut cache = DescribeCache::default();
    let all = entities(&model, BodyId::Root);
    let selections = vec![
        Vec::new(),
        vec![Entity::Instance(instance)],
        all.clone(),
        all.iter()
            .copied()
            .filter(|e| matches!(e, Entity::Edge(_)))
            .collect(),
    ];
    for selection in &selections {
        let expected = describe(&model, BodyId::Root, selection);
        assert_eq!(cache.get(&model, BodyId::Root, selection), &expected);
    }
}

#[test]
fn describe_cache_reuses_the_description_for_the_same_inputs() {
    let (model, instance) = sample();
    let mut cache = DescribeCache::default();
    assert_eq!(cache.computed(), 0);
    let selection = vec![Entity::Instance(instance)];
    for _ in 0..50 {
        cache.get(&model, BodyId::Root, &selection);
    }
    assert_eq!(cache.computed(), 1);
}

#[test]
fn describe_cache_redoes_it_when_the_selection_changes() {
    let (model, instance) = sample();
    let mut cache = DescribeCache::default();
    cache.get(&model, BodyId::Root, &[Entity::Instance(instance)]);
    let all = entities(&model, BodyId::Root);
    let info = cache.get(&model, BodyId::Root, &all).clone();
    assert_eq!(cache.computed(), 2);
    assert_eq!(info, describe(&model, BodyId::Root, &all));
    cache.get(&model, BodyId::Root, &[Entity::Instance(instance)]);
    assert_eq!(cache.computed(), 3);
}

#[test]
fn describe_cache_redoes_it_when_the_body_changes() {
    let (model, instance) = sample();
    let definition = model.root().instance(instance).unwrap().definition;
    let body = BodyId::Definition(definition);
    let mut cache = DescribeCache::default();
    let inside = entities(&model, body);
    cache.get(&model, BodyId::Root, &inside);
    let info = cache.get(&model, body, &inside).clone();
    assert_eq!(cache.computed(), 2);
    assert_eq!(info, describe(&model, body, &inside));
}

#[test]
fn describe_cache_redoes_it_when_the_model_changes() {
    let (mut model, instance) = sample();
    let mut cache = DescribeCache::default();
    let selection = vec![Entity::Instance(instance)];
    cache.get(&model, BodyId::Root, &selection);

    let definition = model.root().instance(instance).unwrap().definition;
    let mut editor = modeler::editor::Editor::new();
    assert!(editor.rename_definition(&mut model, definition, "Crate"));
    let info = cache.get(&model, BodyId::Root, &selection).clone();
    assert_eq!(cache.computed(), 2);
    assert_eq!(info.instance.unwrap().definition_name, "Crate");

    polygon(
        &mut model,
        BodyId::Definition(definition),
        &rect(5.0, 2.0, 1.0, 1.0),
    );
    let info = cache.get(&model, BodyId::Root, &selection).clone();
    assert_eq!(cache.computed(), 3);
    assert_eq!(info, describe(&model, BodyId::Root, &selection));
}

#[test]
fn describe_cache_tells_two_models_apart() {
    let (a, instance) = sample();
    let (mut b, _) = sample();
    polygon(&mut b, BodyId::Root, &rect(0.0, 5.0, 9.0, 1.0));
    let mut cache = DescribeCache::default();
    let all_b = entities(&b, BodyId::Root);
    cache.get(&a, BodyId::Root, &[Entity::Instance(instance)]);
    let info = cache.get(&b, BodyId::Root, &all_b).clone();
    assert_eq!(info, describe(&b, BodyId::Root, &all_b));
    assert_eq!(cache.computed(), 2);
}

// ---------------------------------------------------------------------------------------
// Entity Info panel

struct InfoState {
    panel: EntityInfoPanel,
    model: Model,
    selection: Vec<Entity>,
}

fn info_harness(model: Model, selection: Vec<Entity>) -> Harness<'static, InfoState> {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut InfoState| {
            entity_info_panel(
                ui,
                &mut state.panel,
                &state.model,
                BodyId::Root,
                &state.selection,
                Unit::Meter,
            );
        },
        InfoState {
            panel: EntityInfoPanel::default(),
            model,
            selection,
        },
    );
    h.run();
    h
}

#[test]
fn entity_info_panel_describes_once_while_nothing_changes() {
    let (model, instance) = sample();
    let mut h = info_harness(model, vec![Entity::Instance(instance)]);
    for _ in 0..20 {
        h.step();
    }
    assert_eq!(h.state().panel.cache.computed(), 1);
    h.get_by_label("Component");
}

#[test]
fn entity_info_panel_follows_a_new_selection() {
    let (model, instance) = sample();
    let mut h = info_harness(model, vec![Entity::Instance(instance)]);
    let edges: Vec<Entity> = entities(&h.state().model, BodyId::Root)
        .into_iter()
        .filter(|e| matches!(e, Entity::Edge(_)))
        .collect();
    h.state_mut().selection = edges;
    h.run();
    h.get_by_label("4 Edges");
    assert_eq!(h.state().panel.cache.computed(), 2);
}

// ---------------------------------------------------------------------------------------
// Thumbnails

#[test]
fn thumbnail_cache_gives_what_thumbnail_segments_gives() {
    let mut model = Model::new();
    let a = define(&mut model, "A", &[&rect(0.0, 0.0, 1.0, 2.0)]);
    let b = define(&mut model, "B", &[&circle(0.0, 0.0, 1.0, 12)]);
    let mut cache = ThumbnailCache::default();
    assert_eq!(
        cache.get(&model, a),
        thumbnail_segments(&model, a).as_slice()
    );
    assert_eq!(
        cache.get(&model, b),
        thumbnail_segments(&model, b).as_slice()
    );
}

#[test]
fn thumbnail_cache_works_each_definition_out_once() {
    let mut model = Model::new();
    let a = define(&mut model, "A", &[&rect(0.0, 0.0, 1.0, 2.0)]);
    let b = define(&mut model, "B", &[&circle(0.0, 0.0, 1.0, 12)]);
    let mut cache = ThumbnailCache::default();
    assert_eq!(cache.computed(), 0);
    for _ in 0..30 {
        cache.get(&model, a);
        cache.get(&model, b);
    }
    assert_eq!(cache.computed(), 2);
}

#[test]
fn thumbnail_cache_redoes_them_when_the_model_changes() {
    let mut model = Model::new();
    let a = define(&mut model, "A", &[&rect(0.0, 0.0, 1.0, 2.0)]);
    let mut cache = ThumbnailCache::default();
    cache.get(&model, a);
    polygon(&mut model, BodyId::Definition(a), &rect(3.0, 0.0, 1.0, 1.0));
    let got = cache.get(&model, a).to_vec();
    assert_eq!(cache.computed(), 2);
    assert_eq!(got, thumbnail_segments(&model, a));
    assert_eq!(got.len(), 8);
}

fn length(s: &[egui::Pos2; 2]) -> f32 {
    (s[1] - s[0]).length()
}

#[test]
fn thumbnails_keep_at_most_the_cap_and_the_longest_segments() {
    let mut model = Model::new();
    let square = rect(0.0, 0.0, 10.0, 10.0);
    let ring = circle(5.0, 5.0, 1.0, 900);
    let busy = define(&mut model, "Busy", &[&square, &ring]);
    let plain = define(&mut model, "Plain", &[&square]);

    let segments = thumbnail_segments(&model, busy);
    assert_eq!(segments.len(), MAX_THUMBNAIL_SEGMENTS);
    // The square's four sides are far longer than any piece of the ring; they stay, in
    // the same place as in a thumbnail of the square alone (framing from every edge).
    let plain_segments = thumbnail_segments(&model, plain);
    assert_eq!(plain_segments.len(), 4);
    for side in &plain_segments {
        assert!(
            segments.iter().any(|s| {
                let same = (s[0] - side[0]).length() < 1e-4 && (s[1] - side[1]).length() < 1e-4;
                let flipped = (s[0] - side[1]).length() < 1e-4 && (s[1] - side[0]).length() < 1e-4;
                same || flipped
            }),
            "side {side:?} missing"
        );
    }
    let mut lengths: Vec<f32> = segments.iter().map(length).collect();
    lengths.sort_by(|a, b| b.partial_cmp(a).unwrap());
    assert!(lengths[3] > 10.0 * lengths[4]);
}

#[test]
fn thumbnails_under_the_cap_keep_every_edge() {
    let mut model = Model::new();
    let ring = circle(0.0, 0.0, 1.0, MAX_THUMBNAIL_SEGMENTS);
    let d = define(&mut model, "Ring", &[&ring]);
    assert_eq!(thumbnail_segments(&model, d).len(), MAX_THUMBNAIL_SEGMENTS);
}

// ---------------------------------------------------------------------------------------
// Components panel

struct ComponentsState {
    panel: ComponentsPanel,
    model: Model,
}

#[test]
fn components_panel_works_each_thumbnail_out_once() {
    for tiles in [false, true] {
        let mut model = Model::new();
        for name in ["A", "B", "C"] {
            define(&mut model, name, &[&circle(0.0, 0.0, 1.0, 24)]);
        }
        let mut h = Harness::builder()
            .with_size(egui::vec2(400.0, 600.0))
            .build_ui_state(
                |ui, state: &mut ComponentsState| {
                    components_panel(ui, &mut state.panel, &state.model);
                },
                ComponentsState {
                    panel: ComponentsPanel {
                        tiles,
                        ..ComponentsPanel::default()
                    },
                    model,
                },
            );
        h.run();
        for _ in 0..20 {
            h.step();
        }
        assert_eq!(h.state().panel.thumbnails.computed(), 3, "tiles {tiles}");
        h.get_by_label("B");
    }
}
