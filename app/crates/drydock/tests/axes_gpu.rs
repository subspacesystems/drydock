// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! `overlay::axis_lines`. See docs/specs/m4-axes-depth.md and docs/specs/m4-axes-screen.md
//! (the negative half became one dashed line, sized in screen pixels).

use drydock::look::Theme;
use drydock::overlay::{Primitive, axes, axis_lines};
use drydock_render::ExtraLine;
use glam::{DMat4, DQuat, DVec3};

const COLORS: [[f32; 4]; 3] = [
    [200.0 / 255.0, 40.0 / 255.0, 40.0 / 255.0, 1.0],
    [40.0 / 255.0, 150.0 / 255.0, 40.0 / 255.0, 1.0],
    [40.0 / 255.0, 70.0 / 255.0, 210.0 / 255.0, 1.0],
];

fn near(a: DVec3, b: DVec3) -> bool {
    (a - b).length() < 1e-9
}

fn colour_matches(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
}

fn placed() -> DMat4 {
    DMat4::from_rotation_translation(DQuat::from_rotation_z(0.7), DVec3::new(1.0, -2.0, 0.5))
}

/// Lines of axis `i`: positive line first, then the dashes.
fn of_axis(lines: &[ExtraLine], i: usize) -> Vec<ExtraLine> {
    lines
        .iter()
        .copied()
        .filter(|l| colour_matches(l.color, COLORS[i]))
        .collect()
}

#[test]
fn the_positive_halves_are_single_lines_like_the_overlay_axes() {
    let t = placed();
    let lines = axis_lines(t, 100.0, 1.0, Theme::Dark);
    let reference = axes(t, 100.0);
    for i in 0..3 {
        let Primitive::Line { a, b, .. } = reference[2 * i] else {
            panic!("line")
        };
        let mine = of_axis(&lines, i);
        assert!(near(mine[0].a, a) && near(mine[0].b, b), "axis {i}");
    }
}

#[test]
fn each_axis_has_its_colour_and_order() {
    let lines = axis_lines(DMat4::IDENTITY, 10.0, 1.0, Theme::Dark);
    let mut last = 0;
    for l in &lines {
        let i = COLORS
            .iter()
            .position(|c| colour_matches(l.color, *c))
            .expect("one of the three colours");
        assert!(i >= last, "axes come in order red, green, blue");
        last = i;
    }
    assert_eq!(last, 2);
}

#[test]
fn each_axis_is_a_solid_positive_line_then_a_dashed_negative_line() {
    let t = placed();
    let lines = axis_lines(t, 10.0, 1.0, Theme::Dark);
    assert_eq!(lines.len(), 6);
    let origin = t.transform_point3(DVec3::ZERO);
    for (i, unit) in [DVec3::X, DVec3::Y, DVec3::Z].into_iter().enumerate() {
        let dir = t.transform_vector3(unit).normalize();
        let mine = of_axis(&lines, i);
        assert_eq!(mine.len(), 2, "axis {i}");
        assert!(near(mine[0].a, origin) && near(mine[0].b, origin + dir * 10.0));
        assert_eq!(mine[0].dash_px, None, "axis {i}");
        assert!(near(mine[1].a, origin) && near(mine[1].b, origin - dir * 10.0));
        assert_eq!(mine[1].dash_px, Some([6.0, 4.0]), "axis {i}");
    }
}

#[test]
fn width_and_dashes_are_points_times_pixels_per_point() {
    for (ppp, width, dash) in [(1.0, 1.5, [6.0, 4.0]), (2.0, 3.0, [12.0, 8.0])] {
        let lines = axis_lines(DMat4::IDENTITY, 10.0, ppp, Theme::Dark);
        for l in &lines {
            assert!((l.width_px - width).abs() < 1e-6, "ppp {ppp}");
        }
        for i in 0..3 {
            assert_eq!(of_axis(&lines, i)[1].dash_px, Some(dash), "ppp {ppp}");
        }
    }
}

#[test]
fn a_bad_pixels_per_point_counts_as_one() {
    for ppp in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let lines = axis_lines(DMat4::IDENTITY, 10.0, ppp, Theme::Dark);
        assert_eq!(lines.len(), 6, "ppp {ppp}");
        assert!((lines[0].width_px - 1.5).abs() < 1e-6, "ppp {ppp}");
        assert_eq!(lines[1].dash_px, Some([6.0, 4.0]), "ppp {ppp}");
    }
}

#[test]
fn a_flattened_axis_and_bad_input_are_left_out_like_the_overlay_axes() {
    let flat = DMat4::from_scale(DVec3::new(1.0, 1.0, 0.0));
    let lines = axis_lines(flat, 10.0, 1.0, Theme::Dark);
    assert!(of_axis(&lines, 2).is_empty());
    assert!(!of_axis(&lines, 0).is_empty() && !of_axis(&lines, 1).is_empty());
    for half in [0.0, -1.0, f64::NAN] {
        assert!(axis_lines(DMat4::IDENTITY, half, 1.0, Theme::Dark).is_empty());
    }
    let away = DMat4::from_translation(DVec3::new(f64::NAN, 0.0, 0.0));
    assert!(axis_lines(away, 10.0, 1.0, Theme::Dark).is_empty());
}
