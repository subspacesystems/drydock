// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-entity-info-tag-width: a long tag name in Entity Info's Tag box must not widen the
//! dock the panel sits in, and the "Tag" label sits left of the box like the other rows.
//! Driven with egui_kittest inside a resizable right side panel.
//! See docs/specs/m10-entity-info-tag-width.md.

use drydock::entity_info::*;
use drydock_kernel::component::make_component;
use drydock_kernel::model::*;
use drydock_kernel::select::Entity;
use eframe::egui;
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use glam::DVec3;

const WIDTH: f32 = 240.0;
const SCREEN: f32 = 900.0;

struct State {
    panel: EntityInfoPanel,
    model: Model,
    selection: Vec<Entity>,
    instance: InstanceId,
    width: f32,
}

fn harness(tag_name: Option<&str>) -> Harness<'static, State> {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let pts = [
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(1.0, 0.0, 0.0),
        DVec3::new(1.0, 1.0, 0.0),
        DVec3::new(0.0, 1.0, 0.0),
    ];
    model
        .edit_geometry(BodyId::Root, &mut log, |ctx, edit| {
            for i in 0..4 {
                ctx.add_edge(pts[i], pts[(i + 1) % 4], edit).unwrap();
            }
        })
        .unwrap();
    let geometry = model.body(BodyId::Root).unwrap().geometry();
    let mut parts: Vec<Entity> = geometry.edges().map(|(id, _)| Entity::Edge(id)).collect();
    parts.extend(geometry.faces().map(|(id, _)| Entity::Face(id)));
    parts.sort();
    let instance = make_component(
        &mut model,
        BodyId::Root,
        &parts,
        DefinitionInfo {
            name: "G".to_string(),
            group: true,
        },
        &mut UndoStack::new(),
        "Make Component",
    )
    .unwrap();
    if let Some(name) = tag_name {
        let tag = model.add_tag(name, &mut ModelEdit::new()).unwrap();
        model
            .assign_tag(instance, Some(tag), &mut ModelEdit::new())
            .unwrap();
    }
    let mut h = Harness::builder()
        .with_size(egui::vec2(SCREEN, 600.0))
        .build_ui_state(
            |ui, state: &mut State| {
                let out = egui::Panel::right("dock")
                    .resizable(true)
                    .default_size(WIDTH)
                    .show(ui, |ui| {
                        entity_info_panel(
                            ui,
                            &mut state.panel,
                            &state.model,
                            BodyId::Root,
                            &state.selection,
                        );
                    });
                state.width = out.response.rect.width();
            },
            State {
                panel: EntityInfoPanel::default(),
                model,
                selection: vec![Entity::Instance(instance)],
                instance,
                width: 0.0,
            },
        );
    h.run_steps(8);
    h
}

#[test]
fn a_long_tag_name_does_not_widen_the_panel() {
    let plain = harness(None).state().width;
    assert!(
        (plain - WIDTH).abs() < 2.0,
        "panel starts at {WIDTH}, got {plain}"
    );
    let long = harness(Some(&"w".repeat(40))).state().width;
    assert!(
        long <= plain + 0.5,
        "width {plain} with no tag, {long} with a long tag"
    );
}

#[test]
fn the_width_stays_after_the_tag_is_removed() {
    let mut h = harness(Some(&"w".repeat(40)));
    let before = h.state().width;
    let instance = h.state().instance;
    h.state_mut()
        .model
        .assign_tag(instance, None, &mut ModelEdit::new())
        .unwrap();
    h.run_steps(8);
    assert!((h.state().width - before).abs() < 0.5);
}

#[test]
fn the_box_ends_inside_the_panel_and_the_label_is_on_its_left() {
    let h = harness(Some(&"w".repeat(40)));
    let combo = h.get_by_role_and_label(Role::ComboBox, "Tag").rect();
    assert!(combo.max.x <= SCREEN + 0.5, "box ends at {}", combo.max.x);
    assert!(
        combo.min.x >= SCREEN - WIDTH - 0.5,
        "box starts at {}",
        combo.min.x
    );
    assert!(combo.width() >= 55.0, "box is {} wide", combo.width());
    let label = h
        .query_all_by_label("Tag")
        .map(|n| n.rect())
        .find(|r| r.max.x <= combo.min.x + 0.5);
    assert!(
        label.is_some(),
        "no 'Tag' label left of the box at {combo:?}"
    );
}

#[test]
fn the_box_still_reads_the_whole_name() {
    let name = "w".repeat(40);
    let h = harness(Some(&name));
    let node = h.get_by_role_and_label(Role::ComboBox, "Tag");
    assert_eq!(
        node.accesskit_node().value().as_deref(),
        Some(name.as_str())
    );
}
