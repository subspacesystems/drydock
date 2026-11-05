// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-eyedropper-2: a pipette icon that is also the cursor, and the Materials panel showing
//! the current material plainly (a box behind the whole tile, scrolled into view when it
//! changes). See docs/specs/m6-eyedropper-2.md. The editor and button rules are in
//! tests/eyedropper.rs.

use drydock::materials_panel::{MaterialAction, MaterialsPanel, materials_panel};
use drydock_kernel::model::{Material, MaterialId, Model, ModelEdit};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

// ---- the panel ----

const WIDTH: f32 = 160.0;
const HEIGHT: f32 = 300.0;

struct State {
    panel: MaterialsPanel,
    model: Model,
    current: Option<MaterialId>,
    actions: Vec<MaterialAction>,
}

fn model_with(count: usize) -> (Model, Vec<MaterialId>) {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let ids = (0..count)
        .map(|i| {
            let shade = (i as f32 / count as f32).min(1.0);
            model.add_material(
                Material::new(&format!("Mat{i:02}"), [shade, 0.5, 0.5, 1.0]),
                &mut log,
            )
        })
        .collect();
    (model, ids)
}

fn harness(model: Model, current: Option<MaterialId>, tiles: bool) -> Harness<'static, State> {
    // The window is the panel's column, so whatever is on screen is in view.
    let mut h = Harness::builder()
        .with_size(egui::vec2(WIDTH, HEIGHT))
        .build_ui_state(
            |ui, state: &mut State| {
                if let Some(action) =
                    materials_panel(ui, &mut state.panel, &state.model, state.current)
                {
                    state.actions.push(action);
                }
            },
            State {
                panel: {
                    let mut panel = MaterialsPanel::default();
                    panel.tiles = tiles;
                    panel
                },
                model,
                current,
                actions: Vec::new(),
            },
        );
    h.run();
    h
}

/// The entry is on screen inside the panel (rows scrolled away may leave the tree).
fn visible(h: &Harness<'_, State>, label: &str) -> bool {
    h.query_by_label(label).is_some_and(|node| {
        let rect = node.rect();
        rect.top() >= -0.5 && rect.bottom() <= HEIGHT + 0.5
    })
}

#[test]
fn the_current_tile_is_one_selected_box_around_swatch_and_name() {
    use egui::accesskit::Toggled;
    let (model, ids) = model_with(2);
    let h = harness(model, Some(ids[1]), true);
    let tile = h.get_by_label("Mat01");
    assert_eq!(tile.accesskit_node().toggled(), Some(Toggled::True));
    let rect = tile.rect();
    assert!(
        rect.width() >= 64.0 - 0.5,
        "the selected box spans the tile: {rect:?}"
    );
    assert!(
        rect.height() >= 40.0 + 18.0,
        "the selected box covers the swatch and the name: {rect:?}"
    );
    assert_eq!(
        h.get_by_label("Mat00").accesskit_node().toggled(),
        Some(Toggled::False)
    );
}

#[test]
fn a_newly_current_material_is_scrolled_into_view() {
    for tiles in [false, true] {
        let (model, ids) = model_with(60);
        let mut h = harness(model, Some(ids[0]), tiles);
        assert!(visible(&h, "Mat00"));
        assert!(!visible(&h, "Mat59"), "the last one starts out of view");
        // The eyedropper (or anything else) makes the last material current.
        h.state_mut().current = Some(ids[59]);
        h.run();
        h.run();
        assert!(visible(&h, "Mat59"), "scrolled to it (tiles: {tiles})");
        // And back to the first.
        h.state_mut().current = Some(ids[0]);
        h.run();
        h.run();
        assert!(visible(&h, "Mat00"), "scrolled back (tiles: {tiles})");
    }
}

#[test]
fn the_panel_only_scrolls_when_the_current_material_changes() {
    let (model, ids) = model_with(60);
    let mut h = harness(model, Some(ids[0]), false);
    assert!(visible(&h, "Mat00"));
    // The user scrolls down; the current material stays put, so the list must not jump
    // back to it.
    h.get_by_label("Mat05").hover();
    h.step();
    for _ in 0..10 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -200.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.step();
    }
    // A tooltip may keep asking for frames, so step rather than run; the wheel scroll is
    // animated, so give it time.
    h.run_steps(30);
    assert!(!visible(&h, "Mat00"), "the user's scroll is kept");
}
