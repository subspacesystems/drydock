// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! A docked panel's title bar stands out from its body with the same fill as a dialog's
//! title bar (round 4).

use eframe::egui::{self, vec2};
use egui_kittest::Harness;
use modeler::dock::{HEADER_HEIGHT, PanelId, header_fill, panel_header};
use modeler::look::{Theme, style};

fn flatten<'a>(shape: &'a egui::Shape, out: &mut Vec<&'a egui::Shape>) {
    match shape {
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| flatten(s, out)),
        other => out.push(other),
    }
}

#[test]
fn the_title_fill_is_a_dialog_title_bar_and_differs_from_the_panel() {
    for theme in [Theme::Light, Theme::Dark] {
        let visuals = style(theme).visuals;
        let fill = header_fill(&visuals);
        assert_eq!(fill, visuals.widgets.open.weak_bg_fill, "{theme:?}");
        assert_ne!(fill, visuals.panel_fill, "{theme:?}");
        assert_ne!(fill, visuals.window_fill, "{theme:?}");
    }
}

#[test]
fn the_header_paints_a_band_across_the_whole_row() {
    for docked in [true, false] {
        let mut h = Harness::builder()
            .with_size(vec2(300.0, 200.0))
            .build_ui(move |ui| {
                ui.set_style(style(Theme::Dark));
                panel_header(ui, PanelId::Materials, false, docked);
            });
        h.run();
        let fill = header_fill(&style(Theme::Dark).visuals);
        let mut shapes = Vec::new();
        for clipped in &h.output().shapes {
            flatten(&clipped.shape, &mut shapes);
        }
        let band = shapes.iter().find_map(|shape| match shape {
            egui::Shape::Rect(rect) if rect.fill == fill => Some(rect.rect),
            _ => None,
        });
        let band = band.expect("a rectangle in the title fill");
        assert!(
            (band.height() - HEADER_HEIGHT).abs() < 0.5,
            "one header tall: {band:?}"
        );
        assert!(band.width() >= 280.0, "spans the row: {band:?}");
    }
}
