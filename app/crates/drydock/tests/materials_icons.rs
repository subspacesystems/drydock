// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-materials-icons: the Materials panel's buttons are icon buttons with tooltips,
//! driven with egui_kittest. See docs/specs/m6-materials-icons.md.

use drydock::icons::BUTTON_SIZE;
use drydock::materials_panel::{MaterialAction, MaterialsPanel, materials_panel};
use drydock_kernel::model::{Material, MaterialId, Model, ModelEdit};
use eframe::egui;
use eframe::egui::accesskit::{Role, Toggled};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

const WIDTH: f32 = 160.0;
const HEIGHT: f32 = 400.0;

const BUTTONS: [(&str, &str); 5] = [
    ("New...", "New material"),
    ("Edit...", "Edit the current material"),
    ("Purge Unused", "Remove the materials nothing uses"),
    ("List", "Show as a list"),
    ("Tiles", "Show as tiles"),
];

struct State {
    panel: MaterialsPanel,
    model: Model,
    current: Option<MaterialId>,
    actions: Vec<MaterialAction>,
}

fn brick() -> (Model, MaterialId) {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let id = model.add_material(Material::new("Brick", [0.7, 0.3, 0.2, 1.0]), &mut log);
    (model, id)
}

/// The panel in a column `WIDTH` wide, as the app's right panel gives it.
fn harness(model: Model, current: Option<MaterialId>) -> Harness<'static, State> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(600.0, 800.0))
        .build_ui_state(
            |ui, state: &mut State| {
                ui.allocate_ui(egui::vec2(WIDTH, HEIGHT), |ui| {
                    ui.set_max_width(WIDTH);
                    if let Some(action) =
                        materials_panel(ui, &mut state.panel, &state.model, state.current)
                    {
                        state.actions.push(action);
                    }
                });
            },
            State {
                panel: MaterialsPanel::default(),
                model,
                current,
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
fn every_button_is_a_square_icon_button() {
    let (model, id) = brick();
    let h = harness(model, Some(id));
    for (label, _) in BUTTONS {
        let node = h.get_by_label(label);
        assert_eq!(node.accesskit_node().role(), Role::Button, "{label}");
        let rect = node.rect();
        assert!(
            (rect.width() - BUTTON_SIZE).abs() < 0.5 && (rect.height() - BUTTON_SIZE).abs() < 0.5,
            "{label}: {rect:?}"
        );
    }
}

#[test]
fn buttons_keep_their_rows() {
    let (model, id) = brick();
    let h = harness(model, Some(id));
    let rect = |label: &str| h.get_by_label(label).rect();
    let (new, edit, purge) = (rect("New..."), rect("Edit..."), rect("Purge Unused"));
    let (list, tiles) = (rect("List"), rect("Tiles"));
    for button in [edit, purge] {
        assert!((button.center().y - new.center().y).abs() < 0.5, "one row");
    }
    assert!(
        new.right() <= edit.left() && edit.right() <= purge.left(),
        "in order"
    );
    assert!((list.center().y - tiles.center().y).abs() < 0.5, "one row");
    assert!(list.right() <= tiles.left());
    let search = h.get_by_role(Role::TextInput).rect();
    assert!(new.bottom() <= search.top() + 0.5);
    assert!(search.bottom() <= list.top() + 0.5);
    assert!(tiles.bottom() <= h.get_by_label("Default").rect().top() + 0.5);
}

#[test]
fn hovering_shows_each_tooltip() {
    for (label, tooltip) in BUTTONS {
        let (model, id) = brick();
        let mut h = harness(model, Some(id));
        h.get_by_label(label).hover();
        for _ in 0..90 {
            h.step();
        }
        h.run();
        assert!(
            h.query_by_label(tooltip).is_some(),
            "hovering {label} shows {tooltip:?}"
        );
    }
}

#[test]
fn list_and_tiles_show_which_is_on() {
    let (model, id) = brick();
    let mut h = harness(model, Some(id));
    assert!(selected(&h, "List") && !selected(&h, "Tiles"));
    h.get_by_label("Tiles").click();
    h.run();
    assert!(h.state().panel.tiles);
    assert!(!selected(&h, "List") && selected(&h, "Tiles"));
    h.get_by_label("List").click();
    h.run();
    assert!(!h.state().panel.tiles);
    assert!(selected(&h, "List") && !selected(&h, "Tiles"));
}

#[test]
fn new_edit_and_purge_still_work() {
    let (model, id) = brick();
    let mut h = harness(model, Some(id));
    h.get_by_label("New...").click();
    h.run();
    assert_eq!(h.state().panel.form.as_ref().unwrap().editing, None);
    h.state_mut().panel.form = None;
    h.run();
    h.get_by_label("Edit...").click();
    h.run();
    assert_eq!(h.state().panel.form.as_ref().unwrap().editing, Some(id));
    h.state_mut().panel.form = None;
    h.run();
    h.get_by_label("Purge Unused").click();
    h.run();
    assert!(h.state().panel.confirming());
    assert!(h.state().actions.is_empty());
}

#[test]
fn edit_is_disabled_without_a_current_material() {
    let (model, _) = brick();
    let mut h = harness(model, None);
    assert!(h.get_by_label("Edit...").accesskit_node().is_disabled());
    h.get_by_label("Edit...").click();
    h.run();
    assert_eq!(h.state().panel.form, None);
    assert!(!h.get_by_label("New...").accesskit_node().is_disabled());
}
