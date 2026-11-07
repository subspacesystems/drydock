// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-button-text-center: a button's label is centered vertically in its rect even when the
//! button is taller than its text. Reads the painted text shapes of the last frame.
//! See docs/specs/m10-button-text-center.md.

use drydock::dialog::{BUTTON_MIN_SIZE, DialogButton, button_row};
use drydock::look::{self, Theme};
use eframe::egui::{self, Shape};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

fn flatten<'a>(shape: &'a Shape, out: &mut Vec<&'a Shape>) {
    match shape {
        Shape::Vec(shapes) => shapes.iter().for_each(|s| flatten(s, out)),
        other => out.push(other),
    }
}

/// Vertical center of the painted galley for `label`.
fn text_center_y<S>(h: &Harness<'_, S>, label: &str) -> f32 {
    let mut shapes = Vec::new();
    for clipped in &h.output().shapes {
        flatten(&clipped.shape, &mut shapes);
    }
    shapes
        .iter()
        .find_map(|s| match s {
            Shape::Text(t) if t.galley.text() == label => Some(t.pos.y + t.galley.rect.center().y),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no text shape {label:?}"))
}

fn themed<S>(h: &mut Harness<'_, S>, theme: egui::ThemePreference) {
    look::apply(&h.ctx);
    h.ctx.set_theme(theme);
    h.run();
    h.run();
}

fn check<S>(h: &Harness<'_, S>, label: &str) {
    let r = h.get_by_label(label).rect();
    let dy = text_center_y(h, label) - r.center().y;
    assert!(
        dy.abs() <= 1.0,
        "{label}: text center off by {dy} px in {r:?}"
    );
}

fn dialog_row(
    buttons: Vec<(&'static str, bool)>,
    theme: egui::ThemePreference,
) -> Harness<'static, ()> {
    let mut h = Harness::builder()
        .with_size([420.0, 160.0])
        .build_ui(move |ui| {
            let row: Vec<DialogButton<'_>> = buttons
                .iter()
                .map(|&(label, default)| DialogButton {
                    label,
                    default,
                    enabled: true,
                })
                .collect();
            button_row(ui, &row);
        });
    themed(&mut h, theme);
    h
}

#[test]
fn dialog_buttons_are_centered_in_both_themes() {
    for theme in [egui::ThemePreference::Light, egui::ThemePreference::Dark] {
        let h = dialog_row(
            vec![("Apply", true), ("Close", false), ("Other", false)],
            theme,
        );
        for label in ["Apply", "Close", "Other"] {
            check(&h, label);
        }
        let h = dialog_row(vec![("Export", true), ("Cancel", false)], theme);
        check(&h, "Export");
        check(&h, "Cancel");
    }
}

#[test]
fn dialog_buttons_really_are_taller_than_their_text() {
    let h = dialog_row(vec![("Close", false)], egui::ThemePreference::Light);
    assert!(h.get_by_label("Close").rect().height() >= BUTTON_MIN_SIZE.y - 0.5);
}

#[test]
fn primary_button_sized_and_sized_button_are_centered() {
    let mut h = Harness::builder().with_size([420.0, 200.0]).build_ui(|ui| {
        let _ = look::primary_button_sized(ui, "Primary", BUTTON_MIN_SIZE);
        let _ = look::sized_button(ui, "Plain", BUTTON_MIN_SIZE);
        let _ = look::sized_button(ui, "Tall", egui::vec2(120.0, 60.0));
    });
    themed(&mut h, egui::ThemePreference::Dark);
    for label in ["Primary", "Plain", "Tall"] {
        check(&h, label);
    }
}

#[test]
fn centered_in_a_vertical_and_a_right_to_left_layout() {
    let mut h = Harness::builder().with_size([420.0, 300.0]).build_ui(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            let _ = look::sized_button(ui, "RightLeft", BUTTON_MIN_SIZE);
        });
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            let _ = look::sized_button(ui, "TopDown", BUTTON_MIN_SIZE);
        });
    });
    themed(&mut h, egui::ThemePreference::Light);
    check(&h, "RightLeft");
    check(&h, "TopDown");
}

#[test]
fn small_buttons_are_unchanged() {
    let mut h = Harness::builder().with_size([300.0, 100.0]).build_ui(|ui| {
        let _ = ui.button("Add");
    });
    themed(&mut h, egui::ThemePreference::Light);
    let r = h.get_by_label("Add").rect();
    assert!(r.height() < 28.0, "{r:?}");
    check(&h, "Add");
}

#[test]
fn theme_enum_is_still_exported() {
    let _ = (Theme::Light, Theme::Dark);
}
