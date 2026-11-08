// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-materials-form-width: the New / Edit Material form keeps the right dock's width, and
//! every form widget stays inside the dock. Driven with egui_kittest inside a resizable
//! right side panel with a scroll area, the way the app docks the Materials panel.
//! See docs/specs/m11-materials-form-width.md.

use drydock::materials_panel::{MaterialsPanel, materials_panel};
use drydock_kernel::model::{Material, Model, ModelEdit};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

const WINDOW: f32 = 900.0;
const WIDTH: f32 = 280.0;

/// The app's font (the workspace builds egui without its bundled fonts).
const ROBOTO: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::empty();
    fonts.font_data.insert(
        "Roboto".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(ROBOTO)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.insert(family, vec!["Roboto".to_owned()]);
    }
    ctx.set_fonts(fonts);
}

struct State {
    panel: MaterialsPanel,
    model: Model,
    /// The dock's width in the last frame.
    width: f32,
}

/// A dock with the form open, over a model with enough materials that the list scrolls.
fn harness(editing: bool) -> Harness<'static, State> {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let mut first = None;
    for i in 0..40 {
        let id = model.add_material(Material::new(&format!("Material {i}"), [0.5; 4]), &mut log);
        first.get_or_insert(id);
    }
    let mut panel = MaterialsPanel::default();
    if editing {
        let id = first.unwrap();
        panel.form = Some(drydock::materials_panel::MaterialForm::edit(
            id,
            model.material(id).unwrap(),
        ));
    }
    let mut h = Harness::builder()
        .with_size(egui::vec2(WINDOW, 700.0))
        .build_ui_state(
            |ui, state: &mut State| {
                let out = egui::Panel::right("dock")
                    .resizable(true)
                    .default_size(WIDTH)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                materials_panel(ui, &mut state.panel, &state.model, None);
                            });
                    });
                state.width = out.response.rect.width();
            },
            State {
                panel,
                model,
                width: 0.0,
            },
        );
    install_fonts(&h.ctx);
    if !editing {
        h.run_steps(3);
        h.get_by_label("New...").click();
    }
    h.run_steps(6);
    assert!(h.state().panel.form.is_some(), "the form is open");
    h
}

fn settle(h: &mut Harness<'_, State>) -> f32 {
    h.run_steps(6);
    h.state().width
}

/// Every field widget of the form must lie inside the dock.
fn assert_inside(h: &Harness<'_, State>, context: &str) {
    use eframe::egui::accesskit::Role;
    let left = WINDOW - h.state().width;
    for role in [
        Role::TextInput,
        Role::SpinButton,
        Role::Slider,
        Role::Button,
    ] {
        for node in h.query_all_by_role(role) {
            let r = node.rect();
            assert!(
                r.max.x <= WINDOW + 0.5 && r.min.x >= left - 0.5,
                "{context}: a {role:?} {:?} spans {}..{}, dock is {left}..{WINDOW}",
                node.accesskit_node().label(),
                r.min.x,
                r.max.x
            );
        }
    }
}

fn fields(h: &Harness<'_, State>) -> Vec<egui::Pos2> {
    use eframe::egui::accesskit::Role;
    [Role::TextInput, Role::SpinButton, Role::Slider]
        .into_iter()
        .flat_map(|role| h.query_all_by_role(role).map(|n| n.rect().center()))
        .collect()
}

fn click_every_field(h: &mut Harness<'_, State>, before: f32) {
    // Skip the search box (first text input) by clicking only widgets under the form's
    // heading: all of them, clicking twice is harmless.
    for pos in fields(h) {
        h.event(egui::Event::PointerMoved(pos));
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        let now = settle(h);
        assert!(
            (now - before).abs() <= 1.0,
            "dock width {before} -> {now} after clicking at {pos:?}"
        );
        h.key_press(egui::Key::Escape);
        h.run_steps(2);
    }
}

#[test]
fn opening_the_new_form_keeps_the_dock_width() {
    let mut h = harness(false);
    let now = settle(&mut h);
    assert!(
        (now - WIDTH).abs() <= 1.0,
        "dock was {WIDTH}, is {now} with the form open"
    );
    assert_inside(&h, "form open");
}

#[test]
fn clicking_into_each_field_keeps_the_dock_width() {
    let mut h = harness(false);
    let before = settle(&mut h);
    click_every_field(&mut h, before);
    assert_inside(&h, "after clicks");
}

#[test]
fn scrolling_keeps_the_dock_width() {
    let mut h = harness(false);
    let before = settle(&mut h);
    h.event(egui::Event::PointerMoved(egui::pos2(WINDOW - 100.0, 400.0)));
    for _ in 0..3 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -150.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        let now = settle(&mut h);
        assert!(
            (now - before).abs() <= 1.0,
            "dock width {before} -> {now} after scrolling"
        );
    }
    assert_inside(&h, "after scrolling");
}

#[test]
fn the_edit_form_keeps_the_dock_width_too() {
    let mut h = harness(true);
    let before = settle(&mut h);
    assert!((before - WIDTH).abs() <= 1.0, "dock {before}");
    click_every_field(&mut h, before);
    assert_inside(&h, "editing");
}

#[test]
fn a_long_texture_path_does_not_widen_the_dock() {
    let mut h = harness(false);
    let before = settle(&mut h);
    h.state_mut().panel.form.as_mut().unwrap().texture = "/very/long/path/".repeat(8) + "wood.png";
    let now = settle(&mut h);
    assert!((now - before).abs() <= 1.0, "dock {before} -> {now}");
    assert_inside(&h, "long texture path");
}
