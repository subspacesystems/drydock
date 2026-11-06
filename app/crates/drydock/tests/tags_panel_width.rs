// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-tags-rename-width: renaming a tag must not change the width of the dock the Tags
//! panel sits in. Driven with egui_kittest inside a real resizable right side panel (the
//! way the app docks it), where an over-wide row makes the panel grow.
//! See docs/specs/m10-tags-rename-width.md.

use drydock::tags_panel::*;
use drydock_kernel::model::*;
use drydock_kernel::tags::TagId;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

const WIDTH: f32 = 240.0;

struct State {
    panel: TagsPanel,
    model: Model,
    actions: Vec<TagAction>,
    /// The side panel's width in the last frame.
    width: f32,
}

fn harness() -> (Harness<'static, State>, TagId) {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let id = model.add_tag("Walls", &mut log).unwrap();
    let mut h = Harness::builder()
        .with_size(egui::vec2(900.0, 600.0))
        .build_ui_state(
            |ui, state: &mut State| {
                // The way the app docks a panel: resizable right panel, scroll areas inside.
                let out = egui::Panel::right("dock")
                    .resizable(true)
                    .default_size(WIDTH)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if let Some(a) = tags_panel(ui, &mut state.panel, &state.model) {
                                    state.actions.push(a);
                                }
                            });
                    });
                state.width = out.response.rect.width();
            },
            State {
                panel: TagsPanel::default(),
                model,
                actions: Vec::new(),
                width: 0.0,
            },
        );
    h.run_steps(4);
    (h, id)
}

fn settle(h: &mut Harness<'_, State>) -> f32 {
    h.run_steps(6);
    h.state().width
}

#[test]
fn a_long_rename_text_does_not_widen_the_panel() {
    let (mut h, id) = harness();
    let before = settle(&mut h);
    assert!(
        (before - WIDTH).abs() < 2.0,
        "panel starts at {WIDTH}, got {before}"
    );
    h.state_mut().panel.editing = Some((id, "x".repeat(40)));
    let during = settle(&mut h);
    assert!(
        during <= before + 0.5,
        "width grew from {before} to {during} while renaming"
    );
}

#[test]
fn the_width_is_unchanged_after_enter() {
    let (mut h, id) = harness();
    let before = settle(&mut h);
    h.state_mut().panel.editing = Some((id, "y".repeat(40)));
    settle(&mut h);
    h.key_press(egui::Key::Enter);
    let after = settle(&mut h);
    assert!(h.state().panel.editing.is_none(), "Enter ends the rename");
    assert!(
        h.state()
            .actions
            .iter()
            .any(|a| matches!(a, TagAction::Rename(i, t) if *i == id && t.len() == 40)),
        "Enter confirms the rename"
    );
    assert!(
        (after - before).abs() < 0.5,
        "width {before} before, {after} after Enter"
    );
}

#[test]
fn the_delete_button_stays_inside_the_panel() {
    let (mut h, id) = harness();
    h.state_mut().panel.editing = Some((id, "z".repeat(40)));
    settle(&mut h);
    let delete = h.get_by_label("Delete Walls").rect();
    let right_edge = 900.0;
    assert!(
        delete.max.x <= right_edge + 0.5,
        "delete button ends at {}",
        delete.max.x
    );
    assert!(
        delete.min.x >= right_edge - WIDTH - 0.5,
        "delete button starts at {}, outside the {WIDTH}-wide panel",
        delete.min.x
    );
}
