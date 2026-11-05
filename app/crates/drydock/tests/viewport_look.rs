// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Viewport colours per theme (`docs/specs/m10-brand-viewport.md`).

use drydock::look::{Theme, apply_viewport_theme, axis_color, palette, viewport_style};
use drydock::overlay::axis_lines;
use drydock_render::{Style, SunLight};
use glam::DMat4;

fn unit(c: eframe::egui::Color32) -> [f32; 4] {
    [
        f32::from(c.r()) / 255.0,
        f32::from(c.g()) / 255.0,
        f32::from(c.b()) / 255.0,
        1.0,
    ]
}

fn close(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
}

#[test]
fn the_viewport_style_takes_its_colours_from_the_palette() {
    for theme in [Theme::Dark, Theme::Light] {
        let p = palette(theme);
        let s = viewport_style(theme);
        assert!(
            close(s.background, unit(p.viewport_bg)),
            "{theme:?} background"
        );
        assert!(close(s.default_line, unit(p.edge)), "{theme:?} edge");
        assert!(close(s.default_face, unit(p.face_front)), "{theme:?} face");
        assert!(
            close(s.default_back.expect("a back colour"), unit(p.face_back)),
            "{theme:?} back"
        );
        assert_eq!(s.sun, None);
    }
}

#[test]
fn the_brand_values_for_dark_and_light() {
    let d = viewport_style(Theme::Dark);
    assert!(close(
        d.background,
        [5.0 / 255.0, 10.0 / 255.0, 25.0 / 255.0, 1.0]
    ));
    assert!(close(
        d.default_back.unwrap(),
        [176.0 / 255.0, 74.0 / 255.0, 128.0 / 255.0, 1.0]
    ));
    let l = viewport_style(Theme::Light);
    assert!(close(
        l.background,
        [244.0 / 255.0, 248.0 / 255.0, 252.0 / 255.0, 1.0]
    ));
    assert!(close(
        l.default_line,
        [9.0 / 255.0, 17.0 / 255.0, 38.0 / 255.0, 1.0]
    ));
    assert!(close(
        l.default_back.unwrap(),
        [194.0 / 255.0, 90.0 / 255.0, 140.0 / 255.0, 1.0]
    ));
}

#[test]
fn switching_themes_changes_the_colours_and_keeps_the_sun() {
    let sun = SunLight {
        direction: glam::DVec3::Z,
        light: 0.75,
        dark: 0.4,
        shadows: false,
        on_ground: false,
    };
    let mut style = Style {
        sun: Some(sun),
        ..viewport_style(Theme::Dark)
    };
    assert!(
        !apply_viewport_theme(&mut style, Theme::Dark),
        "already dark"
    );
    assert!(apply_viewport_theme(&mut style, Theme::Light));
    assert_eq!(style.sun, Some(sun));
    let mut expected = viewport_style(Theme::Light);
    expected.sun = Some(sun);
    assert_eq!(style, expected);
    assert!(apply_viewport_theme(&mut style, Theme::Dark));
    assert!(!apply_viewport_theme(&mut style, Theme::Dark));
}

#[test]
fn the_axis_lines_use_the_themes_axis_colours() {
    for theme in [Theme::Dark, Theme::Light] {
        let lines = axis_lines(DMat4::IDENTITY, 10.0, 1.0, theme);
        assert_eq!(lines.len(), 6);
        for (i, pair) in lines.chunks(2).enumerate() {
            let [r, g, b] = axis_color(theme, i);
            let want = [
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
                1.0,
            ];
            for line in pair {
                assert!(close(line.color, want), "{theme:?} axis {i}");
            }
        }
    }
    assert_ne!(axis_color(Theme::Dark, 0), axis_color(Theme::Light, 0));
}
