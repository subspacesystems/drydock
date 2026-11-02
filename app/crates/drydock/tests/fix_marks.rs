// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-fix-marks: the problem edges as overlay lines (`docs/specs/m7-fix-marks.md`).

use drydock::overlay::{Primitive, Style, problem_stroke};
use drydock::solid_ui::mark_primitives;
use drydock_dae::{PROBLEM_COLOR, SolidMarks};
use glam::DVec3;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn marks(segments: Vec<[DVec3; 2]>) -> SolidMarks {
    SolidMarks {
        triangles: Vec::new(),
        segments,
    }
}

#[test]
fn empty_marks_give_no_primitives() {
    assert!(mark_primitives(&SolidMarks::default()).is_empty());
}

#[test]
fn one_problem_line_per_segment_in_order() {
    let segs = vec![
        [v(0.0, 0.0, 1.0), v(1.0, 0.0, 1.0)],
        [v(1.0, 0.0, 1.0), v(1.0, 2.0, 1.0)],
        [v(1.0, 2.0, 1.0), v(0.0, 0.0, 1.0)],
    ];
    let out = mark_primitives(&marks(segs.clone()));
    assert_eq!(out.len(), 3);
    for (p, s) in out.iter().zip(&segs) {
        assert_eq!(
            p,
            &Primitive::Line {
                a: s[0],
                b: s[1],
                style: Style::Problem
            }
        );
    }
}

#[test]
fn triangles_alone_draw_no_lines() {
    let mut m = SolidMarks::default();
    m.triangles.push(Default::default());
    assert!(mark_primitives(&m).is_empty());
}

#[test]
fn problem_stroke_is_the_problem_color_and_wider_than_selected() {
    let (rgba, width) = problem_stroke();
    for (byte, f) in rgba.iter().zip(PROBLEM_COLOR) {
        let want = f64::from(f) * 255.0;
        assert!((f64::from(*byte) - want).abs() <= 1.0, "{rgba:?}");
    }
    // Style::Selected is 3.0 points wide.
    assert!(width >= 4.0, "{width}");
}
