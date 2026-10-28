// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-dialogs: colon labels, a right-aligned button row with a default button that Enter
//! presses, bordered buttons, and the Geolocation window laid out that way.
//! See docs/specs/m6-dialogs.md.

use eframe::egui::{self, Key, Rect};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use modeler::dialog::{
    BUTTON_MIN_SIZE, DialogButton, ROW_GAP, button_row, default_fill, field_label,
};
use modeler::editor::Editor;
use modeler::look::{self, Theme};
use modeler::sun_windows::SunWindows;
use modeler_kernel::model::Model;

#[test]
fn field_labels_end_with_one_colon() {
    assert_eq!(field_label("Name"), "Name:");
    assert_eq!(field_label("Name:"), "Name:");
    assert_eq!(field_label("Latitude (deg)  "), "Latitude (deg):");
}

#[test]
fn the_default_fill_is_not_an_ordinary_buttons() {
    for theme in [Theme::Light, Theme::Dark] {
        let visuals = look::style(theme).visuals;
        assert_ne!(
            default_fill(&visuals),
            visuals.widgets.inactive.weak_bg_fill
        );
        assert_eq!(default_fill(&visuals), visuals.selection.bg_fill);
    }
}

#[test]
fn buttons_have_a_visible_border_and_room() {
    for theme in [Theme::Light, Theme::Dark] {
        let style = look::style(theme);
        let stroke = style.visuals.widgets.inactive.bg_stroke;
        assert!(stroke.width >= 1.0, "{theme:?}");
        assert_ne!(stroke.color, egui::Color32::TRANSPARENT, "{theme:?}");
        assert!(style.spacing.button_padding.y >= 4.0, "{theme:?}");
        assert!(
            (20.0..=28.0).contains(&style.spacing.interact_size.y),
            "{theme:?}"
        );
    }
}

struct Row {
    buttons: Vec<(&'static str, bool, bool)>,
    got: Vec<usize>,
}

fn row(buttons: &[(&'static str, bool, bool)]) -> Harness<'static, Row> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(400.0, 200.0))
        .build_ui_state(
            |ui, row: &mut Row| {
                ui.label("Above");
                let buttons: Vec<DialogButton> = row
                    .buttons
                    .iter()
                    .map(|&(label, default, enabled)| DialogButton {
                        label,
                        default,
                        enabled,
                    })
                    .collect();
                if let Some(index) = button_row(ui, &buttons) {
                    row.got.push(index);
                }
            },
            Row {
                buttons: buttons.to_vec(),
                got: Vec::new(),
            },
        );
    h.ctx.set_fonts(look::font_definitions(None));
    h.run();
    h
}

fn rect(h: &Harness<'_, Row>, label: &str) -> Rect {
    h.get_by_label(label).rect()
}

#[test]
fn the_row_is_on_one_line_at_the_right_in_order_with_a_gap_above() {
    let h = row(&[("Apply", true, true), ("Close", false, true)]);
    let (apply, close, above) = (rect(&h, "Apply"), rect(&h, "Close"), rect(&h, "Above"));
    assert!((apply.center().y - close.center().y).abs() < 1.0);
    assert!(apply.right() <= close.left());
    assert!(
        close.right() <= 400.0 && close.right() >= 400.0 - 24.0,
        "{close:?}"
    );
    assert!(
        apply.top() - above.bottom() >= ROW_GAP - 1.0,
        "{apply:?} {above:?}"
    );
}

#[test]
fn buttons_are_at_least_the_minimum_size() {
    let h = row(&[("OK", true, true), ("Cancel", false, true)]);
    for label in ["OK", "Cancel"] {
        let r = rect(&h, label);
        assert!(r.width() >= BUTTON_MIN_SIZE.x - 0.5, "{label}: {r:?}");
        assert!(r.height() >= BUTTON_MIN_SIZE.y - 0.5, "{label}: {r:?}");
    }
}

#[test]
fn clicking_reports_the_buttons_index() {
    let mut h = row(&[("Apply", true, true), ("Close", false, true)]);
    h.get_by_label("Close").click();
    h.run();
    assert_eq!(h.state().got, vec![1]);
}

#[test]
fn enter_presses_the_default_button() {
    let mut h = row(&[("Apply", false, true), ("Close", true, true)]);
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(h.state().got, vec![1]);
}

#[test]
fn enter_does_nothing_without_an_enabled_default() {
    let mut none = row(&[("Apply", false, true), ("Close", false, true)]);
    none.key_press(Key::Enter);
    none.run();
    assert!(none.state().got.is_empty());

    let mut disabled = row(&[("Apply", true, false), ("Close", false, true)]);
    disabled.key_press(Key::Enter);
    disabled.run();
    assert!(disabled.state().got.is_empty());
}

#[test]
fn a_disabled_button_does_not_report_clicks() {
    let mut h = row(&[("Apply", true, false), ("Close", false, true)]);
    h.get_by_label("Apply").click();
    h.run();
    assert!(h.state().got.is_empty());
}

struct Windows {
    windows: SunWindows,
    model: Model,
    editor: Editor,
    changed: usize,
}

fn geo_window() -> Harness<'static, Windows> {
    let mut windows = SunWindows::new();
    windows.geo_open = true;
    let mut h = Harness::new_ui_state(
        |ui, s: &mut Windows| {
            let ctx = ui.ctx().clone();
            if s.windows.show(&ctx, &mut s.model, &mut s.editor) {
                s.changed += 1;
            }
        },
        Windows {
            windows,
            model: Model::new(),
            editor: Editor::new(),
            changed: 0,
        },
    );
    h.ctx.set_fonts(look::font_definitions(None));
    h.run();
    h
}

#[test]
fn geolocation_labels_end_with_colons() {
    let h = geo_window();
    for label in [
        "Latitude (deg):",
        "Longitude (deg):",
        "North angle (deg):",
        "UTC offset (h):",
    ] {
        h.get_by_label_contains(label);
    }
}

#[test]
fn geolocation_buttons_share_one_right_aligned_row() {
    let h = geo_window();
    let apply = h.get_by_label("Apply").rect();
    let close = h.get_by_label("Close").rect();
    let last_field = h.get_by_label_contains("UTC offset (h):").rect();
    let hint = h.get_by_label_contains("Drag to turn").rect();
    assert!(
        (apply.center().y - close.center().y).abs() < 1.0,
        "{apply:?} {close:?}"
    );
    assert!(apply.right() <= close.left());
    assert!(close.right() >= hint.right() - 16.0, "{close:?} {hint:?}");
    assert!(apply.top() - last_field.bottom() >= ROW_GAP - 2.0);
    assert!(apply.width() >= BUTTON_MIN_SIZE.x - 0.5);
}

#[test]
fn enter_applies_the_geolocation_form() {
    let mut h = geo_window();
    h.key_press(Key::Enter);
    h.run();
    h.run();
    assert!(!h.state().windows.geo_open);
}

#[test]
fn close_closes_the_geolocation_window_without_applying() {
    let mut h = geo_window();
    h.get_by_label("Close").click();
    h.run();
    h.run();
    assert!(!h.state().windows.geo_open);
    assert_eq!(h.state().changed, 0);
}
