// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-axes-depth: `overlay::axis_lines`. See docs/specs/m4-axes-depth.md.

use glam::{DMat4, DQuat, DVec3};
use modeler::overlay::{Primitive, axes, axis_lines};
use modeler_render::ExtraLine;

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
    let lines = axis_lines(t, 100.0, 1.0);
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
    let lines = axis_lines(DMat4::IDENTITY, 10.0, 1.0);
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
fn the_negative_half_is_dashes_with_gaps() {
    let lines = axis_lines(DMat4::IDENTITY, 10.0, 1.0);
    let red = of_axis(&lines, 0);
    // positive line + dashes at [0,1], [2,3], [4,5], [6,7], [8,9]
    assert_eq!(red.len(), 1 + 5);
    for (k, d) in red[1..].iter().enumerate() {
        assert!(near(d.a, DVec3::new(-2.0 * k as f64, 0.0, 0.0)), "dash {k}");
        assert!(
            near(d.b, DVec3::new(-2.0 * k as f64 - 1.0, 0.0, 0.0)),
            "dash {k}"
        );
    }
}

#[test]
fn the_last_dash_is_cut_at_the_half_length() {
    let lines = axis_lines(DMat4::IDENTITY, 4.5, 1.0);
    let blue = of_axis(&lines, 2);
    // dashes at [0,1], [2,3], [4,4.5]
    assert_eq!(blue.len(), 1 + 3);
    assert!(near(blue[3].a, DVec3::new(0.0, 0.0, -4.0)));
    assert!(near(blue[3].b, DVec3::new(0.0, 0.0, -4.5)));
}

#[test]
fn dashes_follow_a_turned_frame() {
    let t = placed();
    let lines = axis_lines(t, 10.0, 1.0);
    let origin = t.transform_point3(DVec3::ZERO);
    let dir = t.transform_vector3(DVec3::Y).normalize();
    let green = of_axis(&lines, 1);
    assert!(near(green[2].a, origin - dir * 2.0));
    assert!(near(green[2].b, origin - dir * 3.0));
}

#[test]
fn at_most_three_hundred_dashes_per_axis() {
    let lines = axis_lines(DMat4::IDENTITY, 1.0e6, 1.0);
    for i in 0..3 {
        assert_eq!(of_axis(&lines, i).len(), 1 + 300, "axis {i}");
    }
}

#[test]
fn a_bad_dash_gives_one_solid_negative_line() {
    for dash in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let lines = axis_lines(DMat4::IDENTITY, 10.0, dash);
        let red = of_axis(&lines, 0);
        assert_eq!(red.len(), 2, "dash {dash}");
        assert!(near(red[1].a, DVec3::ZERO) && near(red[1].b, DVec3::new(-10.0, 0.0, 0.0)));
    }
}

#[test]
fn a_flattened_axis_and_bad_input_are_left_out_like_the_overlay_axes() {
    let flat = DMat4::from_scale(DVec3::new(1.0, 1.0, 0.0));
    let lines = axis_lines(flat, 10.0, 1.0);
    assert!(of_axis(&lines, 2).is_empty());
    assert!(!of_axis(&lines, 0).is_empty() && !of_axis(&lines, 1).is_empty());
    for half in [0.0, -1.0, f64::NAN] {
        assert!(axis_lines(DMat4::IDENTITY, half, 1.0).is_empty());
    }
    let away = DMat4::from_translation(DVec3::new(f64::NAN, 0.0, 0.0));
    assert!(axis_lines(away, 10.0, 1.0).is_empty());
}
