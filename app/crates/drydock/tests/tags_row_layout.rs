// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-tags-row-layout: in a Tags panel row the name (or the rename box) sits right after
//! the eye, and Assign and the delete button sit at the panel's right edge.
//! See docs/specs/m10-tags-row-layout.md.

use drydock::tags_panel::*;
use drydock_kernel::model::*;
use drydock_kernel::tags::TagId;
use eframe::egui;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

const WIDTH: f32 = 240.0;
const SCREEN: f32 = 900.0;

struct State {
    panel: TagsPanel,
    model: Model,
}

fn harness() -> (Harness<'static, State>, TagId) {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let id = model.add_tag("Walls", &mut log).unwrap();
    let mut h = Harness::builder()
        .with_size(egui::vec2(SCREEN, 600.0))
        .build_ui_state(
            |ui, state: &mut State| {
                egui::Panel::right("dock")
                    .resizable(true)
                    .default_size(WIDTH)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                tags_panel(ui, &mut state.panel, &state.model);
                            });
                    });
            },
            State {
                panel: TagsPanel {
                    selected: 1,
                    ..TagsPanel::default()
                },
                model,
            },
        );
    h.run_steps(6);
    (h, id)
}

#[test]
fn the_name_sits_next_to_its_eye() {
    let (h, _) = harness();
    let eye = h.get_by_label("Visible: Walls").rect();
    let name = h.get_by_label("Walls").rect();
    assert!(
        name.min.x >= eye.max.x - 0.5 && name.min.x - eye.max.x <= 12.0,
        "eye ends at {}, name starts at {}",
        eye.max.x,
        name.min.x
    );
}

#[test]
fn the_buttons_stay_at_the_right_edge_after_the_name() {
    let (h, _) = harness();
    let name = h.get_by_label("Walls").rect();
    let assign = h.get_by_label("Assign selection to Walls").rect();
    let delete = h.get_by_label("Delete Walls").rect();
    assert!(
        delete.max.x <= SCREEN + 0.5,
        "delete ends at {}",
        delete.max.x
    );
    assert!(
        SCREEN - delete.max.x <= 16.0,
        "delete is {} px from the right edge",
        SCREEN - delete.max.x
    );
    assert!(
        assign.min.x > name.max.x,
        "Assign must be right of the name"
    );
    assert!(
        assign.max.x <= delete.min.x + 0.5,
        "Assign is left of delete"
    );
}

#[test]
fn the_rename_box_sits_next_to_the_eye_and_stays_inside() {
    let (mut h, id) = harness();
    h.state_mut().panel.editing = Some((id, "Walls".to_string()));
    h.run_steps(6);
    let eye = h.get_by_label("Visible: Walls").rect();
    let edit = h.get_by_role(Role::TextInput).rect();
    let assign = h.get_by_label("Assign selection to Walls").rect();
    assert!(
        edit.min.x >= eye.max.x - 0.5 && edit.min.x - eye.max.x <= 12.0,
        "eye ends at {}, box starts at {}",
        eye.max.x,
        edit.min.x
    );
    assert!(
        edit.max.x <= assign.min.x + 0.5,
        "box ends at {}, Assign starts at {}",
        edit.max.x,
        assign.min.x
    );
}
