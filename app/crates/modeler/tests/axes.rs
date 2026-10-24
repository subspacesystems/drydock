// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-ui-axes: axes overlay, near-plane clipping, the View > Axes toggle.
//! See docs/specs/m4-ui-axes.md.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glam::{DMat4, DQuat, DVec3};
use modeler::chrome::{MenuState, menu_bar};
use modeler::input::Action;
use modeler::overlay::{Primitive, Style, axes, clip_in_front};
use modeler::viewstate::{Effect, ViewState};
use modeler_view::{Camera, Projection};
use std::cell::RefCell;
use std::rc::Rc;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn near(a: DVec3, b: DVec3) -> bool {
    a.distance(b) < 1e-9
}

fn line(p: &Primitive) -> (DVec3, DVec3, Style) {
    match p {
        Primitive::Line { a, b, style } => (*a, *b, *style),
        other => panic!("expected a line, got {other:?}"),
    }
}

fn styles(primitives: &[Primitive]) -> Vec<Style> {
    primitives.iter().map(|p| line(p).2).collect()
}

fn all_styles() -> Vec<Style> {
    let mut out = Vec::new();
    for axis in 0..3 {
        for positive in [true, false] {
            out.push(Style::Axis { axis, positive });
        }
    }
    out
}

fn check_frame(primitives: &[Primitive], origin: DVec3, dirs: [DVec3; 3], half: f64) {
    assert_eq!(primitives.len(), 6);
    assert_eq!(styles(primitives), all_styles());
    for (i, p) in primitives.iter().enumerate() {
        let (a, b, _) = line(p);
        let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
        let want = origin + dirs[i / 2] * (half * sign);
        assert!(near(a, origin), "line {i} starts at {a}, want {origin}");
        assert!(
            a.distance(b) - half < 1e-9 && near(b, want),
            "line {i} ends at {b}, want {want}"
        );
    }
}

#[test]
fn identity_gives_six_half_axes_from_the_origin() {
    let p = axes(DMat4::IDENTITY, 10.0);
    check_frame(&p, DVec3::ZERO, [DVec3::X, DVec3::Y, DVec3::Z], 10.0);
}

#[test]
fn a_placed_frame_moves_and_turns_the_axes() {
    let rot = DQuat::from_rotation_z(0.7) * DQuat::from_rotation_x(0.3);
    let t = DMat4::from_rotation_translation(rot, v(3.0, -2.0, 5.0));
    let p = axes(t, 4.0);
    check_frame(
        &p,
        v(3.0, -2.0, 5.0),
        [rot * DVec3::X, rot * DVec3::Y, rot * DVec3::Z],
        4.0,
    );
}

#[test]
fn scale_does_not_change_the_length() {
    let t = DMat4::from_scale_rotation_translation(
        v(2.0, 3.0, 0.5),
        DQuat::from_rotation_z(1.1),
        v(1.0, 1.0, 0.0),
    );
    let p = axes(t, 7.5);
    let rot = DQuat::from_rotation_z(1.1);
    check_frame(
        &p,
        v(1.0, 1.0, 0.0),
        [rot * DVec3::X, rot * DVec3::Y, DVec3::Z],
        7.5,
    );
}

#[test]
fn a_mirrored_frame_follows_its_axes() {
    let t = DMat4::from_scale(v(-1.0, 1.0, 1.0));
    let p = axes(t, 2.0);
    check_frame(&p, DVec3::ZERO, [-DVec3::X, DVec3::Y, DVec3::Z], 2.0);
}

#[test]
fn a_flattened_axis_is_left_out() {
    let t = DMat4::from_scale(v(1.0, 1.0, 0.0));
    let p = axes(t, 5.0);
    assert_eq!(
        styles(&p),
        vec![
            Style::Axis {
                axis: 0,
                positive: true
            },
            Style::Axis {
                axis: 0,
                positive: false
            },
            Style::Axis {
                axis: 1,
                positive: true
            },
            Style::Axis {
                axis: 1,
                positive: false
            },
        ]
    );
    assert!(near(line(&p[0]).1, v(5.0, 0.0, 0.0)));
    assert!(near(line(&p[3]).1, v(0.0, -5.0, 0.0)));
}

#[test]
fn bad_lengths_give_nothing() {
    for half in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(axes(DMat4::IDENTITY, half).is_empty(), "{half}");
    }
}

#[test]
fn a_non_finite_origin_gives_nothing() {
    let t = DMat4::from_translation(v(f64::NAN, 0.0, 0.0));
    assert!(axes(t, 1.0).is_empty());
}

fn depth(camera: &Camera, p: DVec3) -> f64 {
    (p - camera.eye()).dot(camera.basis().0)
}

fn perspective() -> Camera {
    let camera = Camera::default();
    assert_eq!(camera.projection, Projection::Perspective);
    camera
}

/// A point at `d` along the view direction from the eye, offset sideways.
fn at_depth(camera: &Camera, d: f64, side: f64, up: f64) -> DVec3 {
    let (forward, right, upv) = camera.basis();
    camera.eye() + forward * d + right * side + upv * up
}

#[test]
fn a_segment_in_front_is_unchanged() {
    let c = perspective();
    let a = at_depth(&c, 1.0, -2.0, 0.5);
    let b = at_depth(&c, 30.0, 4.0, -1.0);
    let (ca, cb) = clip_in_front(&c, a, b).expect("in front");
    assert!(near(ca, a) && near(cb, b));
}

#[test]
fn a_segment_behind_is_dropped() {
    let c = perspective();
    let a = at_depth(&c, -1.0, -2.0, 0.5);
    let b = at_depth(&c, -30.0, 4.0, -1.0);
    assert_eq!(clip_in_front(&c, a, b), None);
}

#[test]
fn a_segment_inside_the_near_distance_is_dropped() {
    let c = perspective();
    let near_d = c.distance / 1000.0;
    let a = at_depth(&c, near_d * 0.5, 1.0, 0.0);
    let b = at_depth(&c, near_d * 0.9, -1.0, 0.0);
    assert_eq!(clip_in_front(&c, a, b), None);
}

fn check_crossing(c: &Camera, kept: DVec3, moved_from: DVec3, moved: DVec3) {
    let near_d = c.distance / 1000.0;
    assert!(
        (depth(c, moved) - near_d).abs() < 1e-9,
        "clipped end depth {} want {near_d}",
        depth(c, moved)
    );
    // On the original segment, between its ends.
    let seg = kept - moved_from;
    let t = (moved - moved_from).dot(seg) / seg.length_squared();
    assert!((0.0..=1.0).contains(&t), "t = {t}");
    assert!(near(moved, moved_from + seg * t), "off the segment");
}

#[test]
fn a_crossing_segment_is_cut_at_the_near_distance() {
    let c = perspective();
    let a = at_depth(&c, -5.0, 1.0, 0.0);
    let b = at_depth(&c, 20.0, 0.0, 2.0);
    let (ca, cb) = clip_in_front(&c, a, b).expect("part in front");
    assert!(near(cb, b), "the end in front stays");
    check_crossing(&c, b, a, ca);
}

#[test]
fn the_ends_keep_their_order() {
    let c = perspective();
    let a = at_depth(&c, 12.0, -3.0, 1.0);
    let b = at_depth(&c, -8.0, 2.0, 0.0);
    let (ca, cb) = clip_in_front(&c, a, b).expect("part in front");
    assert!(near(ca, a), "the end in front stays first");
    check_crossing(&c, a, b, cb);
}

#[test]
fn an_end_exactly_at_the_near_distance_is_kept() {
    let c = perspective();
    let near_d = c.distance / 1000.0;
    let a = at_depth(&c, near_d, 0.0, 0.0);
    let b = at_depth(&c, 5.0, 1.0, 1.0);
    let (ca, cb) = clip_in_front(&c, a, b).expect("in front");
    assert!(near(ca, a) && near(cb, b));
}

#[test]
fn a_parallel_camera_keeps_everything() {
    let c = Camera {
        projection: Projection::Parallel,
        ..Camera::default()
    };
    let a = at_depth(&c, -50.0, 1.0, 0.0);
    let b = at_depth(&c, -60.0, 0.0, 2.0);
    let (ca, cb) = clip_in_front(&c, a, b).expect("parallel keeps it");
    assert!(near(ca, a) && near(cb, b));
}

#[test]
fn non_finite_ends_give_nothing() {
    let c = perspective();
    let good = at_depth(&c, 5.0, 0.0, 0.0);
    for bad in [v(f64::NAN, 0.0, 0.0), v(0.0, f64::INFINITY, 0.0)] {
        assert_eq!(clip_in_front(&c, bad, good), None);
        assert_eq!(clip_in_front(&c, good, bad), None);
    }
    let p = Camera {
        projection: Projection::Parallel,
        ..Camera::default()
    };
    assert_eq!(clip_in_front(&p, good, v(0.0, 0.0, f64::NAN)), None);
}

#[test]
fn axes_start_shown() {
    assert!(ViewState::new().axes);
}

#[test]
fn toggle_flips_and_leaves_the_camera() {
    let mut view = ViewState::new();
    let camera = view.camera;
    assert_eq!(view.apply(Action::ToggleAxes, (800.0, 600.0)), Effect::None);
    assert!(!view.axes);
    assert_eq!(view.camera, camera);
    assert_eq!(view.apply(Action::ToggleAxes, (800.0, 600.0)), Effect::None);
    assert!(view.axes);
}

fn choose_axes(axes: bool) -> Vec<Action> {
    let seen: Rc<RefCell<Vec<Action>>> = Rc::default();
    let sink = seen.clone();
    let state = MenuState {
        axes,
        ..MenuState::default()
    };
    let mut h = Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            sink.borrow_mut().push(action);
        }
    });
    h.run();
    h.get_by_label("View").click();
    h.run();
    h.get_by_label("Axes").click();
    h.run();
    seen.borrow().clone()
}

#[test]
fn the_view_menu_toggles_the_axes() {
    assert_eq!(choose_axes(true), vec![Action::ToggleAxes]);
    assert_eq!(choose_axes(false), vec![Action::ToggleAxes]);
}

#[test]
fn the_menu_closes_after_choosing_axes() {
    let state = MenuState {
        axes: true,
        ..MenuState::default()
    };
    let mut h = Harness::new_ui(move |ui| {
        menu_bar(ui, &state);
    });
    h.run();
    h.get_by_label("View").click();
    h.run();
    h.get_by_label("Axes").click();
    h.run();
    h.run();
    assert!(h.query_by_label("Axes").is_none());
}
