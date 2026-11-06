// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-open-dialog-primary: the Open / Import dialog's confirming button is filled with the
//! theme's `action` colour and labelled `on_action`; Cancel stays ordinary. See
//! docs/specs/m10-open-dialog-primary.md.

use drydock::look::{self, Theme};
use drydock::open_buttons::{OpenButtons, OpenMode, choose_label, open_buttons};
use eframe::egui::{self, Color32, Shape};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

const THEMES: [Theme; 2] = [Theme::Light, Theme::Dark];
const MODES: [OpenMode; 3] = [OpenMode::Open, OpenMode::Component, OpenMode::Models];

fn flatten<'a>(shape: &'a Shape, out: &mut Vec<&'a Shape>) {
    match shape {
        Shape::Vec(shapes) => shapes.iter().for_each(|s| flatten(s, out)),
        other => out.push(other),
    }
}

/// (fill behind the label, colour of the label) from the last frame.
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

fn harness(mode: OpenMode, theme: Theme) -> Harness<'static, OpenButtons> {
    let mut h = Harness::new_ui_state(
        move |ui, seen: &mut OpenButtons| {
            let r = open_buttons(ui, mode);
            seen.chosen |= r.chosen;
            seen.cancelled |= r.cancelled;
        },
        OpenButtons::default(),
    );
    look::apply(&h.ctx);
    h.ctx.set_theme(match theme {
        Theme::Light => egui::ThemePreference::Light,
        Theme::Dark => egui::ThemePreference::Dark,
    });
    h.run();
    h.run();
    h
}

#[test]
fn labels_follow_the_mode() {
    assert_eq!(choose_label(OpenMode::Open), "Open");
    assert_eq!(choose_label(OpenMode::Component), "Import");
    assert_eq!(choose_label(OpenMode::Models), "Import");
}

#[test]
fn confirming_button_is_primary_and_cancel_is_not() {
    for theme in THEMES {
        let p = look::palette(theme);
        for mode in MODES {
            let h = harness(mode, theme);
            let label = choose_label(mode);
            let (fill, text) = painted(&h, label);
            assert_eq!(fill, p.action, "{label} fill, {mode:?}, {theme:?}");
            assert_eq!(text, p.on_action, "{label} label, {mode:?}, {theme:?}");
            let (fill, text) = painted(&h, "Cancel");
            assert_ne!(fill, p.action, "Cancel fill, {mode:?}, {theme:?}");
            assert_ne!(text, p.on_action, "Cancel label, {mode:?}, {theme:?}");
        }
    }
}

#[test]
fn clicks_are_reported_separately() {
    for mode in MODES {
        let mut h = harness(mode, Theme::Dark);
        h.get_by_label(choose_label(mode)).click();
        h.run();
        assert_eq!(
            *h.state(),
            OpenButtons {
                chosen: true,
                cancelled: false
            },
            "{mode:?}"
        );
        let mut h = harness(mode, Theme::Dark);
        h.get_by_label("Cancel").click();
        h.run();
        assert_eq!(
            *h.state(),
            OpenButtons {
                chosen: false,
                cancelled: true
            },
            "{mode:?}"
        );
    }
}
