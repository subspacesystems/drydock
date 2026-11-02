// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m7-text-outline.md`.

use drydock_dae::{TextError, TextLoop, TextOutline, text_outline};
use glam::DVec2;
use proptest::prelude::*;

const ROBOTO: &[u8] = include_bytes!("../../drydock/assets/fonts/Roboto-Regular.ttf");

fn outline(text: &str, size: f64, tol: f64) -> TextOutline {
    text_outline(ROBOTO, text, size, tol).expect("outline")
}

fn area(points: &[DVec2]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| points[i].perp_dot(points[(i + 1) % n]))
        .sum::<f64>()
        / 2.0
}

fn inside(point: DVec2, poly: &[DVec2]) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.y > point.y) != (b.y > point.y)
            && point.x < a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x)
        {
            c = !c;
        }
    }
    c
}

fn bbox(o: &TextOutline) -> (DVec2, DVec2) {
    let mut lo = DVec2::splat(f64::MAX);
    let mut hi = DVec2::splat(f64::MIN);
    for l in &o.loops {
        for p in &l.points {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
    }
    (lo, hi)
}

fn points(o: &TextOutline) -> usize {
    o.loops.iter().map(|l| l.points.len()).sum()
}

fn dist_to_loop(p: DVec2, l: &TextLoop) -> f64 {
    let n = l.points.len();
    (0..n)
        .map(|i| {
            let (a, b) = (l.points[i], l.points[(i + 1) % n]);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
            (p - (a + ab * t)).length()
        })
        .fold(f64::MAX, f64::min)
}

fn check_structure(o: &TextOutline) {
    for (i, l) in o.loops.iter().enumerate() {
        let n = l.points.len();
        assert!(n >= 3, "loop {i} has {n} points");
        for k in 0..n {
            assert!(l.points[k].is_finite());
            assert_ne!(
                l.points[k],
                l.points[(k + 1) % n],
                "loop {i} repeats a point"
            );
        }
        let a = area(&l.points);
        if l.hole {
            assert!(a < 0.0, "hole {i} must run clockwise");
            let p = l.parent.expect("hole has a parent");
            assert!(
                p < o.loops.len() && !o.loops[p].hole,
                "parent of {i} is an outer loop"
            );
            assert!(
                inside(l.points[0], &o.loops[p].points),
                "hole {i} inside its parent"
            );
        } else {
            assert!(a > 0.0, "outer {i} must run counter-clockwise");
            assert_eq!(l.parent, None);
        }
    }
}

#[test]
fn o_has_an_outer_loop_and_a_hole() {
    let o = outline("O", 1.0, 0.001);
    assert_eq!(o.loops.len(), 2);
    let outer = o.loops.iter().position(|l| !l.hole).unwrap();
    let hole = o.loops.iter().position(|l| l.hole).unwrap();
    assert_eq!(o.loops[hole].parent, Some(outer));
    assert!(area(&o.loops[outer].points) > -area(&o.loops[hole].points));
    check_structure(&o);
}

#[test]
fn i_has_two_outer_loops() {
    let o = outline("i", 1.0, 0.001);
    assert_eq!(o.loops.len(), 2);
    assert!(o.loops.iter().all(|l| !l.hole && l.parent.is_none()));
    check_structure(&o);
}

#[test]
fn eight_has_one_outer_and_two_holes() {
    let o = outline("8", 1.0, 0.001);
    assert_eq!(o.loops.iter().filter(|l| !l.hole).count(), 1);
    assert_eq!(o.loops.iter().filter(|l| l.hole).count(), 2);
    let outer = o.loops.iter().position(|l| !l.hole).unwrap();
    assert!(
        o.loops
            .iter()
            .filter(|l| l.hole)
            .all(|l| l.parent == Some(outer))
    );
    check_structure(&o);
}

#[test]
fn space_has_no_loops_but_an_advance() {
    let o = outline(" ", 1.0, 0.001);
    assert!(o.loops.is_empty());
    assert!(o.width > 0.1 && o.width < 0.6, "width {}", o.width);
    assert_eq!(o.lines, 1);
}

#[test]
fn empty_text_is_empty() {
    let o = outline("", 1.0, 0.001);
    assert!(o.loops.is_empty());
    assert_eq!(o.width, 0.0);
    assert_eq!(o.lines, 1);
    assert!(
        o.line_height > 0.9 && o.line_height < 1.6,
        "line height {}",
        o.line_height
    );
}

#[test]
fn capital_o_sits_on_the_baseline_at_the_em_scale() {
    let (lo, hi) = bbox(&outline("O", 1.0, 0.001));
    assert!(lo.x > 0.0 && lo.x < 0.15, "left bearing {}", lo.x);
    assert!(lo.y > -0.05 && lo.y <= 0.0, "bottom {}", lo.y);
    assert!(hi.y > 0.65 && hi.y < 0.80, "top {}", hi.y);
}

#[test]
fn bounding_box_and_metrics_scale_with_size() {
    let one = outline("Rust 8", 1.0, 0.001);
    let big = outline("Rust 8", 0.02, 0.00002);
    let (lo1, hi1) = bbox(&one);
    let (lo2, hi2) = bbox(&big);
    let slack = 0.00005;
    assert!((lo1 * 0.02 - lo2).abs().max_element() < slack);
    assert!((hi1 * 0.02 - hi2).abs().max_element() < slack);
    assert!((one.width * 0.02 - big.width).abs() < 1e-9);
    assert!((one.line_height * 0.02 - big.line_height).abs() < 1e-9);
}

#[test]
fn smaller_tolerance_never_gives_fewer_points() {
    for text in ["O", "S&g", "Rust 8"] {
        let counts: Vec<usize> = [0.05, 0.01, 0.002, 0.0005]
            .iter()
            .map(|t| points(&outline(text, 1.0, *t)))
            .collect();
        assert!(
            counts.windows(2).all(|w| w[0] <= w[1]),
            "{text}: {counts:?}"
        );
        assert!(counts[0] < counts[3], "{text}: {counts:?}");
    }
}

#[test]
fn flattened_edges_stay_within_the_tolerance() {
    let tol = 0.01;
    let coarse = outline("S&g@", 1.0, tol);
    let fine = outline("S&g@", 1.0, 0.0001);
    assert_eq!(coarse.loops.len(), fine.loops.len());
    for (c, f) in coarse.loops.iter().zip(&fine.loops) {
        assert_eq!(c.hole, f.hole);
        let n = c.points.len();
        for i in 0..n {
            let mid = (c.points[i] + c.points[(i + 1) % n]) / 2.0;
            let d = dist_to_loop(mid, f);
            assert!(d <= tol + 0.0002, "chord midpoint {d} from the outline");
        }
    }
}

#[test]
fn glyphs_repeat_along_the_line_without_kerning() {
    let a = outline("A", 1.0, 0.001);
    let aa = outline("AA", 1.0, 0.001);
    let n = a.loops.len();
    assert!(n >= 2);
    assert_eq!(aa.loops.len(), 2 * n);
    assert!((aa.width - 2.0 * a.width).abs() < 1e-9);
    for k in 0..n {
        let (first, second) = (&aa.loops[k], &aa.loops[k + n]);
        assert_eq!(first, &a.loops[k]);
        assert_eq!(second.hole, first.hole);
        assert_eq!(second.parent, first.parent.map(|p| p + n));
        for (p, q) in first.points.iter().zip(&second.points) {
            assert!((*p + DVec2::new(a.width, 0.0) - *q).length() < 1e-9);
        }
    }
}

#[test]
fn newline_starts_a_line_one_line_height_down() {
    let a = outline("A", 1.0, 0.001);
    let two = outline("A\nA", 1.0, 0.001);
    let n = a.loops.len();
    assert_eq!(two.lines, 2);
    assert_eq!(two.loops.len(), 2 * n);
    assert!((two.line_height - a.line_height).abs() < 1e-12);
    for k in 0..n {
        for (p, q) in two.loops[k].points.iter().zip(&two.loops[k + n].points) {
            assert!((*p - DVec2::new(0.0, a.line_height) - *q).length() < 1e-9);
        }
    }
    let wide = outline("A\nAAA", 1.0, 0.001);
    assert!((wide.width - outline("AAA", 1.0, 0.001).width).abs() < 1e-9);
    assert_eq!(outline("A\n\nA", 1.0, 0.001).lines, 3);
}

#[test]
fn carriage_return_is_ignored_and_tab_is_four_spaces() {
    assert_eq!(outline("A\r\nA", 1.0, 0.001), outline("A\nA", 1.0, 0.001));
    let tab = outline("\t", 1.0, 0.001);
    let space = outline(" ", 1.0, 0.001);
    assert!((tab.width - 4.0 * space.width).abs() < 1e-9);
    assert!(tab.loops.is_empty());
}

#[test]
fn characters_the_font_lacks_draw_the_missing_glyph() {
    let a = outline("\u{4E2D}", 1.0, 0.001);
    let b = outline("\u{4E2E}", 1.0, 0.001);
    assert_eq!(a, b);
    assert!(a.width > 0.0);
}

#[test]
fn refuses_bad_arguments() {
    let err = |font: &[u8], size: f64, tol: f64| text_outline(font, "A", size, tol).unwrap_err();
    assert_eq!(err(ROBOTO, 0.0, 0.01), TextError::BadSize);
    assert_eq!(err(ROBOTO, -1.0, 0.01), TextError::BadSize);
    assert_eq!(err(ROBOTO, f64::NAN, 0.01), TextError::BadSize);
    assert_eq!(err(ROBOTO, f64::INFINITY, 0.01), TextError::BadSize);
    assert_eq!(err(ROBOTO, 1.0, 0.0), TextError::BadTolerance);
    assert_eq!(err(ROBOTO, 1.0, f64::NAN), TextError::BadTolerance);
    assert_eq!(err(b"not a font", 1.0, 0.01), TextError::BadFont);
    assert_eq!(err(&[], 1.0, 0.01), TextError::BadFont);
    assert_eq!(err(b"not a font", 0.0, 0.01), TextError::BadSize);
}

#[test]
fn output_is_deterministic() {
    assert_eq!(
        outline("Hello, 3D!", 0.01, 0.0001),
        outline("Hello, 3D!", 0.01, 0.0001)
    );
}

#[test]
fn every_printable_ascii_character_is_well_formed() {
    for c in ' '..='~' {
        let o = outline(&c.to_string(), 1.0, 0.002);
        check_structure(&o);
        assert!(o.width > 0.0, "{c:?}");
    }
}

proptest! {
    #[test]
    fn any_text_gives_well_formed_loops(
        text in "[ -~\\n]{0,24}",
        size in 0.001f64..100.0,
        ratio in 0.0005f64..0.05,
    ) {
        let o = text_outline(ROBOTO, &text, size, size * ratio).unwrap();
        check_structure(&o);
        prop_assert_eq!(o.lines, text.split('\n').count());
    }
}
