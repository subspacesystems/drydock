// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-brand-dialog-buttons: the confirming button of a dialog is filled with the theme's
//! `action` colour and labelled `on_action`; Cancel and Close stay ordinary. The tests read
//! the painted shapes of the last frame. See docs/specs/m10-brand-dialog-buttons.md.

use drydock::dialog::{DialogButton, button_row, confirm_dialog};
use drydock::look::{self, Theme};
use drydock::mesh_export::{ExportDialog, ExportFormat, ExportOptions, export_dialog};
use drydock::mesh_import::{MeshUnit, units_choice};
use drydock::text_tool::{TextDialog, text_dialog};
use eframe::egui::{self, Color32, Shape};
use egui_kittest::Harness;

const THEMES: [Theme; 2] = [Theme::Light, Theme::Dark];

fn preference(theme: Theme) -> egui::ThemePreference {
    match theme {
        Theme::Light => egui::ThemePreference::Light,
        Theme::Dark => egui::ThemePreference::Dark,
    }
}

/// Installs the brand look for `theme` and runs two frames so it is in force.
fn themed<S>(h: &mut Harness<'_, S>, theme: Theme) {
    look::apply(&h.ctx);
    h.ctx.set_theme(preference(theme));
    h.run();
    h.run();
}

fn linear(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: Color32) -> f64 {
    0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
}

fn contrast(a: Color32, b: Color32) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn flatten<'a>(shape: &'a Shape, out: &mut Vec<&'a Shape>) {
    match shape {
        Shape::Vec(shapes) => shapes.iter().for_each(|s| flatten(s, out)),
        other => out.push(other),
    }
}

/// The fill behind the label `label` and the colour the label is drawn in, from the last
/// frame: the smallest filled rectangle that contains the label's text (the one painted
/// last when several are equally small).
fn painted<S>(h: &Harness<'_, S>, label: &str) -> (Color32, Color32) {
    let mut shapes = Vec::new();
    for clipped in &h.output().shapes {
        flatten(&clipped.shape, &mut shapes);
    }
    let text = shapes
        .iter()
        .find_map(|s| match s {
            Shape::Text(t) if t.galley.text() == label => Some(t),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no text {label:?} was painted"));
    let color = text
        .override_text_color
        .or_else(|| {
            text.galley
                .job
                .sections
                .first()
                .map(|s| s.format.color)
                .filter(|c| *c != Color32::PLACEHOLDER)
        })
        .unwrap_or(text.fallback_color);
    let centre = text.visual_bounding_rect().center();
    let fill = shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Rect(r) if r.fill.a() > 0 && r.rect.contains(centre) => Some(r),
            _ => None,
        })
        .reduce(|best, r| {
            if r.rect.area() <= best.rect.area() {
                r
            } else {
                best
            }
        })
        .unwrap_or_else(|| panic!("no filled rectangle behind {label:?}"))
        .fill;
    (fill, color)
}

fn assert_primary<S>(h: &Harness<'_, S>, label: &str, theme: Theme) {
    let p = look::palette(theme);
    let (fill, text) = painted(h, label);
    assert_eq!(fill, p.action, "{label} fill in {theme:?}");
    assert_eq!(text, p.on_action, "{label} label in {theme:?}");
    assert!(contrast(fill, text) >= 4.5, "{label} contrast in {theme:?}");
}

fn assert_ordinary<S>(h: &Harness<'_, S>, label: &str, theme: Theme) {
    let p = look::palette(theme);
    let (fill, text) = painted(h, label);
    assert_ne!(fill, p.action, "{label} fill in {theme:?}");
    assert_ne!(text, p.on_action, "{label} label in {theme:?}");
    assert!(contrast(fill, text) >= 4.5, "{label} contrast in {theme:?}");
}

#[test]
fn palette_action_pair_has_contrast_and_hover_fill_is_lighter() {
    for theme in THEMES {
        let p = look::palette(theme);
        assert!(contrast(p.action, p.on_action) >= 4.5, "{theme:?}");
        let hover = look::primary_hover_fill(theme);
        assert_ne!(hover, p.action, "{theme:?}");
        assert!(luminance(hover) > luminance(p.action), "{theme:?}");
        assert!(contrast(hover, p.on_action) >= 4.5, "{theme:?}");
    }
}

#[test]
fn primary_hover_fill_blends_ten_percent_toward_white() {
    let a = look::palette(Theme::Dark).action;
    let want = |c: u8| (f32::from(c) + (255.0 - f32::from(c)) * 0.1).round() as u8;
    assert_eq!(
        look::primary_hover_fill(Theme::Dark),
        Color32::from_rgb(want(a.r()), want(a.g()), want(a.b()))
    );
}

#[test]
fn theme_of_follows_dark_mode() {
    assert_eq!(
        look::theme_of(&look::style(Theme::Dark).visuals),
        Theme::Dark
    );
    assert_eq!(
        look::theme_of(&look::style(Theme::Light).visuals),
        Theme::Light
    );
}

#[test]
fn primary_button_is_action_with_on_action_label_and_clicks() {
    for theme in THEMES {
        let mut h = Harness::new_ui_state(
            |ui, clicks: &mut u32| {
                if look::primary_button(ui, "Apply").clicked() {
                    *clicks += 1;
                }
            },
            0u32,
        );
        themed(&mut h, theme);
        assert_primary(&h, "Apply", theme);
        use egui_kittest::kittest::Queryable;
        h.get_by_label("Apply").click();
        h.run();
        assert_eq!(*h.state(), 1);
    }
}

#[test]
fn primary_button_is_lighter_under_the_pointer() {
    for theme in THEMES {
        let mut h = Harness::new_ui(|ui| {
            look::primary_button(ui, "Apply");
        });
        themed(&mut h, theme);
        use egui_kittest::kittest::Queryable;
        let centre = h.get_by_label("Apply").rect().center();
        h.hover_at(centre);
        h.run();
        let (fill, text) = painted(&h, "Apply");
        assert_eq!(fill, look::primary_hover_fill(theme), "{theme:?}");
        assert_eq!(text, look::palette(theme).on_action, "{theme:?}");
    }
}

#[test]
fn export_dialog_ok_is_primary_and_cancel_is_not() {
    for theme in THEMES {
        let mut h = Harness::new_ui_state(
            |ui, d: &mut ExportDialog| {
                export_dialog(ui, d, true);
            },
            ExportDialog::new(ExportOptions::new(ExportFormat::ThreeMf)),
        );
        themed(&mut h, theme);
        assert_primary(&h, "OK", theme);
        assert_ordinary(&h, "Cancel", theme);
    }
}

#[test]
fn units_dialog_ok_is_primary_and_cancel_is_not() {
    for theme in THEMES {
        let mut h = Harness::new_ui_state(
            |ui, unit: &mut MeshUnit| {
                units_choice(ui, "cube.stl", unit);
            },
            MeshUnit::default(),
        );
        themed(&mut h, theme);
        assert_primary(&h, "OK", theme);
        assert_ordinary(&h, "Cancel", theme);
    }
}

#[test]
fn text_dialog_ok_is_primary_and_cancel_is_not() {
    for theme in THEMES {
        let mut dialog = TextDialog::new();
        dialog.text = "Label".into();
        let mut h = Harness::new_ui_state(
            |ui, d: &mut TextDialog| {
                text_dialog(ui, d);
            },
            dialog,
        );
        themed(&mut h, theme);
        assert_primary(&h, "OK", theme);
        assert_ordinary(&h, "Cancel", theme);
    }
}

fn row_harness(buttons: &'static [(&'static str, bool)], theme: Theme) -> Harness<'static, ()> {
    let mut h = Harness::new_ui(move |ui| {
        let buttons: Vec<DialogButton> = buttons
            .iter()
            .map(|&(label, default)| DialogButton {
                label,
                default,
                enabled: true,
            })
            .collect();
        button_row(ui, &buttons);
    });
    themed(&mut h, theme);
    h
}

#[test]
fn a_default_apply_is_primary_and_a_plain_close_is_not() {
    for theme in THEMES {
        let h = row_harness(&[("Apply", true), ("Close", false)], theme);
        assert_primary(&h, "Apply", theme);
        assert_ordinary(&h, "Close", theme);
    }
}

#[test]
fn a_default_cancel_or_close_is_never_primary() {
    for theme in THEMES {
        let h = row_harness(&[("Delete", false), ("Cancel", true)], theme);
        assert_ordinary(&h, "Delete", theme);
        assert_ordinary(&h, "Cancel", theme);
        let h = row_harness(&[("Close", true)], theme);
        assert_ordinary(&h, "Close", theme);
    }
}

#[test]
fn a_row_without_a_default_has_no_violet_button() {
    for theme in THEMES {
        let h = row_harness(&[("Purge", false), ("Keep", false)], theme);
        assert_ordinary(&h, "Purge", theme);
        assert_ordinary(&h, "Keep", theme);
    }
}

#[test]
fn a_confirm_dialog_keeps_both_buttons_ordinary() {
    for theme in THEMES {
        let mut h = Harness::new_ui(|ui| {
            confirm_dialog(
                &ui.ctx().clone(),
                egui::Id::new("c"),
                "Delete",
                "Delete it?",
                "Delete",
            );
        });
        themed(&mut h, theme);
        assert_ordinary(&h, "Delete", theme);
        assert_ordinary(&h, "Cancel", theme);
    }
}
