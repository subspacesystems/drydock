// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-zoom-cursor: wheel and Zoom-tool zoom about the model point under the cursor.
//! See docs/specs/m4-zoom-cursor.md.

use drydock::editor::Viewport;
use drydock::input::{self, Action};
use drydock::viewstate::{Effect, ViewState, zoom_pivot_at};
use drydock_dae::{Batch, FlatScene};
use drydock_view::{Camera, Projection, StandardView, ZOOM_STEP_FACTOR};
use glam::{DVec2, DVec3};

const VP: (f64, f64) = (800.0, 600.0);
const CENTER: DVec2 = DVec2::new(400.0, 300.0);

/// A square (-s..s) at height z, facing up.
fn quad(s: f32, z: f32) -> FlatScene {
    FlatScene {
        materials: vec![],
        batches: vec![Batch {
            material: None,
            positions: vec![[-s, -s, z], [s, -s, z], [s, s, z], [-s, s, z]],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            texcoords: vec![],
            indices: vec![0, 1, 2, 0, 2, 3],
        }],
        lines: vec![],
        bounds: None,
    }
}

/// Looking straight down at the origin from `distance` above it.
fn top(distance: f64, projection: Projection) -> Camera {
    let mut c = Camera {
        target: DVec3::ZERO,
        distance,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: 35f64.to_radians(),
        projection,
    };
    c.set_standard_view(StandardView::Top);
    c
}

fn view(camera: Camera) -> ViewState {
    let mut s = ViewState::new();
    s.camera = camera;
    s
}

fn near(a: DVec3, b: DVec3, tol: f64) -> bool {
    (a - b).length() <= tol
}

fn ndc(px: DVec2) -> DVec2 {
    DVec2::new(2.0 * px.x / VP.0 - 1.0, 1.0 - 2.0 * px.y / VP.1)
}

#[test]
fn a_new_view_has_no_zoom_pivot() {
    assert_eq!(ViewState::new().zoom_pivot, None);
}

#[test]
fn the_pick_is_the_model_point_under_the_cursor() {
    let scene = quad(50.0, 2.0);
    for projection in [Projection::Perspective, Projection::Parallel] {
        let camera = top(10.0, projection);
        let hit = zoom_pivot_at(&camera, Some(&scene), VP, CENTER).expect("hit at the center");
        assert!(
            near(hit, DVec3::new(0.0, 0.0, 2.0), 1e-5),
            "{projection:?} {hit:?}"
        );
        let cursor = DVec2::new(650.0, 120.0);
        let hit = zoom_pivot_at(&camera, Some(&scene), VP, cursor).expect("hit off center");
        assert!((hit.z - 2.0).abs() < 1e-5, "{projection:?} {hit:?}");
        let shown = Viewport {
            camera,
            size_px: VP,
        }
        .project(hit)
        .unwrap();
        assert!(shown.distance(cursor) < 1e-3, "{projection:?} {shown:?}");
    }
}

#[test]
fn the_pick_is_none_over_empty_space_or_without_a_model() {
    let camera = top(10.0, Projection::Perspective);
    let scene = quad(0.5, 0.0);
    assert!(zoom_pivot_at(&camera, Some(&scene), VP, CENTER).is_some());
    assert_eq!(
        zoom_pivot_at(&camera, Some(&scene), VP, DVec2::new(10.0, 10.0)),
        None
    );
    assert_eq!(zoom_pivot_at(&camera, None, VP, CENTER), None);
}

#[test]
fn a_zoom_action_with_a_zoom_pivot_scales_about_it() {
    let pivot = DVec3::new(3.0, -2.0, 1.0);
    let mut s = ViewState::new();
    s.zoom_pivot = Some(pivot);
    let mut want = s.camera;
    want.zoom_about(2.0, pivot);
    let action = Action::Zoom {
        steps: 2.0,
        cursor_ndc: DVec2::new(0.3, -0.2),
    };
    assert_eq!(s.apply(action, VP), Effect::None);
    assert_eq!(s.camera, want);
    // The pivot stays for the rest of the drag.
    assert_eq!(s.zoom_pivot, Some(pivot));
    want.zoom_about(0.5, pivot);
    s.apply(
        Action::Zoom {
            steps: 0.5,
            cursor_ndc: DVec2::new(-0.9, 0.9),
        },
        VP,
    );
    assert_eq!(s.camera, want);
}

#[test]
fn orbit_and_pan_ignore_the_zoom_pivot() {
    let mut with = ViewState::new();
    with.zoom_pivot = Some(DVec3::new(9.0, 9.0, 9.0));
    let mut without = ViewState::new();
    for action in [
        Action::Orbit { dx: 20.0, dy: -8.0 },
        Action::Pan { dx: 12.0, dy: 4.0 },
    ] {
        with.apply(action, VP);
        without.apply(action, VP);
        assert_eq!(with.camera, without.camera, "{action:?}");
    }
}

#[test]
fn a_wheel_notch_close_to_a_surface_does_not_pass_it() {
    // Tyrel's report: the eye is 0.5 m above a floor, the target 50 m below the eye.
    let scene = quad(5.0, 49.5);
    let mut s = view(top(50.0, Projection::Perspective));
    assert!((s.camera.eye().z - 50.0).abs() < 1e-9);
    s.scroll_zoom(1.0, CENTER, Some(&scene), VP);
    let z = s.camera.eye().z;
    assert!(z > 49.5, "passed the floor: eye at z = {z}");
    assert!(
        (z - (49.5 + 0.5 * ZOOM_STEP_FACTOR)).abs() < 1e-5,
        "eye at z = {z}"
    );
    // And back out.
    s.scroll_zoom(-1.0, CENTER, Some(&scene), VP);
    assert!(
        (s.camera.eye().z - 50.0).abs() < 1e-5,
        "{}",
        s.camera.eye().z
    );
}

#[test]
fn a_wheel_zoom_over_the_model_scales_about_the_hit() {
    let scene = quad(50.0, 2.0);
    let cursor = DVec2::new(600.0, 200.0);
    for projection in [Projection::Perspective, Projection::Parallel] {
        let mut s = view(top(10.0, projection));
        let hit = zoom_pivot_at(&s.camera, Some(&scene), VP, cursor).unwrap();
        let mut want = s.camera;
        want.zoom_about(1.5, hit);
        s.scroll_zoom(1.5, cursor, Some(&scene), VP);
        assert!(near(s.camera.target, want.target, 1e-9), "{projection:?}");
        assert!((s.camera.distance - want.distance).abs() < 1e-12);
    }
}

#[test]
fn a_wheel_zoom_over_empty_space_works_as_before() {
    let scene = quad(0.5, 0.0);
    let cursor = DVec2::new(30.0, 550.0);
    for scene in [Some(&scene), None] {
        let mut s = view(top(10.0, Projection::Perspective));
        let mut want = view(top(10.0, Projection::Perspective));
        want.apply(input::scroll(2.0, ndc(cursor)), VP);
        s.scroll_zoom(2.0, cursor, scene, VP);
        assert_eq!(s.camera, want.camera, "{}", scene.is_some());
    }
}

#[test]
fn a_wheel_zoom_leaves_the_pivots_alone_and_zero_lines_do_nothing() {
    let scene = quad(50.0, 2.0);
    let orbit_pivot = Some(DVec3::new(1.0, 1.0, 1.0));
    for zoom_pivot in [None, Some(DVec3::new(-4.0, 0.0, 3.0))] {
        let mut s = view(top(10.0, Projection::Perspective));
        s.pivot = orbit_pivot;
        s.zoom_pivot = zoom_pivot;
        s.scroll_zoom(1.0, CENTER, Some(&scene), VP);
        assert_eq!(s.pivot, orbit_pivot);
        assert_eq!(s.zoom_pivot, zoom_pivot);
    }
    let mut s = view(top(10.0, Projection::Perspective));
    let before = s.camera;
    s.scroll_zoom(0.0, CENTER, Some(&scene), VP);
    assert_eq!(s.camera, before);
}
