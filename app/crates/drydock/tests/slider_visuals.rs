// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-slider-visuals: sliders show a rail in both themes and the handle reacts to hover.
//! See docs/specs/m10-slider-visuals.md.

use drydock::look::{self, Theme, palette, style};
use drydock::shadows_panel::shadows_panel;
use drydock_kernel::sun::ShadowSettings;
use eframe::egui::{self, accesskit::Role};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

fn lin(v: u8) -> f64 {
    let v = f64::from(v) / 255.0;
    if v <= 0.03928 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: egui::Color32) -> f64 {
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

fn contrast(a: egui::Color32, b: egui::Color32) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn slider_visuals(theme: Theme) -> egui::Visuals {
    let mut v = style(theme).visuals;
    look::slider_widgets(theme, &mut v);
    v
}

#[test]
fn the_rail_has_3_to_1_contrast_with_the_panel_in_both_themes() {
    for theme in [Theme::Light, Theme::Dark] {
        let p = palette(theme);
        let v = slider_visuals(theme);
        let rail = v.widgets.inactive.bg_fill;
        assert_eq!(rail, p.border_control, "{theme:?}");
        let ratio = contrast(rail, p.surface);
        assert!(ratio >= 3.0, "{theme:?} rail on the panel: {ratio:.2}");
    }
}

#[test]
fn the_handle_changes_on_hover_and_press() {
    for theme in [Theme::Light, Theme::Dark] {
        let p = palette(theme);
        let w = slider_visuals(theme).widgets;
        assert_eq!(w.inactive.fg_stroke, egui::Stroke::new(1.0, p.ink));
        for (name, state) in [("hovered", w.hovered), ("active", w.active)] {
            assert_eq!(state.bg_fill, p.accent, "{theme:?} {name}");
            assert_eq!(
                state.fg_stroke,
                egui::Stroke::new(2.0, p.ink),
                "{theme:?} {name}"
            );
            assert_ne!(state.bg_fill, w.inactive.bg_fill, "{theme:?} {name}");
        }
    }
}

#[test]
fn the_shared_widget_colours_are_untouched() {
    for theme in [Theme::Light, Theme::Dark] {
        let p = palette(theme);
        let v = style(theme).visuals;
        assert_eq!(v.widgets.inactive.bg_fill, p.surface_raised, "{theme:?}");
        assert_eq!(v.widgets.hovered.bg_fill, p.accent_soft, "{theme:?}");
        let mut scoped = v.clone();
        look::slider_widgets(theme, &mut scoped);
        assert_eq!(scoped.panel_fill, v.panel_fill, "{theme:?}");
        assert_eq!(
            scoped.widgets.noninteractive, v.widgets.noninteractive,
            "{theme:?}"
        );
        assert_eq!(
            scoped.widgets.inactive.weak_bg_fill, v.widgets.inactive.weak_bg_fill,
            "{theme:?}"
        );
        assert_eq!(
            scoped.widgets.hovered.weak_bg_fill,
            v.widgets.hovered.weak_bg_fill
        );
    }
}

fn flatten<'a>(shape: &'a egui::Shape, out: &mut Vec<&'a egui::Shape>) {
    match shape {
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| flatten(s, out)),
        other => out.push(other),
    }
}

/// Fill and stroke of every rectangle painted in the last frame, in order (egui 0.36 draws
/// slider handles as rectangles by default).
fn circles(h: &Harness<'_, ShadowSettings>) -> Vec<(egui::Color32, egui::Stroke)> {
    let mut shapes = Vec::new();
    for clipped in &h.output().shapes {
        flatten(&clipped.shape, &mut shapes);
    }
    shapes
        .iter()
        .filter_map(|s| match s {
            egui::Shape::Rect(r) => Some((r.fill, r.stroke)),
            _ => None,
        })
        .collect()
}

fn harness(theme: Theme) -> Harness<'static, ShadowSettings> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(360.0, 400.0))
        .build_ui_state(
            move |ui, settings: &mut ShadowSettings| {
                ui.set_style(style(theme));
                shadows_panel(ui, settings);
            },
            ShadowSettings::default(),
        );
    h.run();
    h
}

#[test]
fn hovering_a_shadows_slider_repaints_its_handle_in_the_accent() {
    for theme in [Theme::Light, Theme::Dark] {
        let p = palette(theme);
        let mut h = harness(theme);
        let rest = circles(&h);
        let handles = rest
            .iter()
            .filter(|(fill, stroke)| {
                *fill == p.border_control && stroke.width == 1.0 && stroke.color == p.ink
            })
            .count();
        assert_eq!(handles, 4, "{theme:?}: four resting handles in {rest:?}");
        h.query_all_by_role(Role::Slider)
            .next()
            .expect("a slider")
            .hover();
        h.run();
        let hovered = circles(&h);
        assert_eq!(hovered.len(), rest.len(), "{theme:?}");
        let changed: Vec<_> = hovered.iter().zip(&rest).filter(|(a, b)| a != b).collect();
        assert_eq!(
            changed.len(),
            1,
            "{theme:?}: only the hovered handle changes"
        );
        let (fill, stroke) = changed[0].0;
        assert_eq!(*fill, p.accent, "{theme:?}");
        assert_eq!(stroke.width, 2.0, "{theme:?}");
    }
}
