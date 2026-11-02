// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-fix-dialogs: Enter presses OK in the Export, Mesh units and 3D Text dialogs, and the
//! Print Setup window opens below the menus. See docs/specs/m7-fix-dialogs.md.

use drydock::mesh_export::*;
use drydock::mesh_import::{MeshUnit, UnitsOutcome, units_choice};
use drydock::print_setup::{PrintSetupTexts, PrintSetupView, WINDOW_MARGIN, window_pos};
use drydock::text_tool::*;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

// ---- Export ----

struct Export {
    dialog: ExportDialog,
    outcomes: Vec<ExportOutcome>,
}

fn export_harness(tolerance_text: &str) -> Harness<'static, Export> {
    let mut dialog = ExportDialog::new(ExportOptions::new(ExportFormat::ThreeMf));
    dialog.tolerance_text = tolerance_text.into();
    let mut h = Harness::new_ui_state(
        |ui, state: &mut Export| {
            if let Some(outcome) = export_dialog(ui, &mut state.dialog, true) {
                state.outcomes.push(outcome);
            }
        },
        Export {
            dialog,
            outcomes: Vec::new(),
        },
    );
    h.run();
    h
}

#[test]
fn export_enter_presses_ok() {
    let mut h = export_harness("0.01");
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(h.state().outcomes, vec![ExportOutcome::Ok]);
}

#[test]
fn export_enter_does_nothing_while_ok_is_disabled() {
    let mut h = export_harness("wide");
    assert!(h.get_by_label("OK").accesskit_node().is_disabled());
    h.key_press(egui::Key::Enter);
    h.run();
    assert!(h.state().outcomes.is_empty());
}

#[test]
fn export_cancel_still_cancels_and_nothing_fires_unasked() {
    let mut h = export_harness("0.01");
    assert!(h.state().outcomes.is_empty());
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().outcomes, vec![ExportOutcome::Cancel]);
}

#[test]
fn export_ok_button_still_works() {
    let mut h = export_harness("0.01");
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(h.state().outcomes, vec![ExportOutcome::Ok]);
}

// ---- Mesh units ----

struct Choice {
    unit: MeshUnit,
    outcomes: Vec<UnitsOutcome>,
}

fn choice_harness() -> Harness<'static, Choice> {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut Choice| {
            if let Some(outcome) = units_choice(ui, "cube.stl", &mut state.unit) {
                state.outcomes.push(outcome);
            }
        },
        Choice {
            unit: MeshUnit::default(),
            outcomes: Vec::new(),
        },
    );
    h.run();
    h
}

#[test]
fn units_enter_presses_ok_and_keeps_the_picked_unit() {
    let mut h = choice_harness();
    h.get_by_label("cm").click();
    h.run();
    assert!(h.state().outcomes.is_empty());
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(h.state().outcomes, vec![UnitsOutcome::Ok]);
    assert_eq!(h.state().unit, MeshUnit::Centimeters);
}

#[test]
fn units_cancel_still_cancels() {
    let mut h = choice_harness();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().outcomes, vec![UnitsOutcome::Cancel]);
}

// ---- 3D Text ----

struct Text {
    dialog: TextDialog,
    outcomes: Vec<TextOutcome>,
}

fn text_harness(text: &str) -> Harness<'static, Text> {
    let mut dialog = TextDialog::new();
    dialog.text = text.into();
    let mut h = Harness::new_ui_state(
        |ui, state: &mut Text| {
            if let Some(outcome) = text_dialog(ui, &mut state.dialog) {
                state.outcomes.push(outcome);
            }
        },
        Text {
            dialog,
            outcomes: Vec::new(),
        },
    );
    h.run();
    h
}

#[test]
fn text_enter_presses_ok() {
    let mut h = text_harness("Label");
    h.key_press(egui::Key::Enter);
    h.run();
    assert_eq!(h.state().outcomes, vec![TextOutcome::Ok]);
}

#[test]
fn text_enter_does_nothing_while_the_text_is_empty() {
    let mut h = text_harness("");
    assert!(h.get_by_label("OK").accesskit_node().is_disabled());
    h.key_press(egui::Key::Enter);
    h.run();
    assert!(h.state().outcomes.is_empty());
}

#[test]
fn text_cancel_still_cancels() {
    let mut h = text_harness("Label");
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().outcomes, vec![TextOutcome::Cancel]);
}

// ---- Print Setup position ----

#[test]
fn the_window_position_is_the_free_area_corner_plus_the_margin() {
    let available = egui::Rect::from_min_max(egui::pos2(0.0, 70.0), egui::pos2(800.0, 600.0));
    let pos = window_pos(available);
    assert_eq!(pos, egui::pos2(WINDOW_MARGIN, 70.0 + WINDOW_MARGIN));
}

#[test]
fn print_setup_opens_below_the_menu_bar() {
    let mut texts = PrintSetupTexts::default();
    let mut h = Harness::new_ui(move |ui| {
        egui::Panel::top("menus").exact_size(60.0).show(ui, |ui| {
            ui.label("menu bar");
        });
        let ctx = ui.ctx().clone();
        let free_area = ui.available_rect_before_wrap();
        egui::Window::new("Print Setup")
            .default_pos(window_pos(free_area))
            .show(&ctx, |ui| {
                let view = PrintSetupView {
                    bed: Default::default(),
                    show_bed: false,
                    overhang_degrees: 45.0,
                    can_drop: false,
                    report: None,
                };
                drydock::print_setup::print_setup_ui(ui, &mut texts, &view);
            });
    });
    h.run();
    let menu = h.get_by_label("menu bar").rect();
    let window = h.get_by_label("Printer bed").rect();
    assert!(
        window.top() >= menu.bottom() + WINDOW_MARGIN - 1.0,
        "window {window:?} under menu {menu:?}"
    );
}
