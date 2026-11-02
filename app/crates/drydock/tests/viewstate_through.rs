// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-zoom-through: Ctrl+wheel steps the eye through the surface under the cursor.
//! See docs/specs/m4-zoom-through.md.

use drydock::editor::Viewport;
use drydock::viewstate::{ViewState, through_distance};
use drydock_dae::{Batch, FlatScene};
use drydock_view::{Camera, Projection, StandardView};
use glam::{DVec2, DVec3};

const VP: (f64, f64) = (800.0, 600.0);
const CENTER: DVec2 = DVec2::new(400.0, 300.0);

fn quad(s: f32, z: f32) -> Batch {
    Batch {
        material: None,
        positions: vec![[-s, -s, z], [s, -s, z], [s, s, z], [-s, s, z]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        texcoords: vec![],
        indices: vec![0, 1, 2, 0, 2, 3],
    }
}

/// Large squares at the given heights.
fn floors(zs: &[f32]) -> FlatScene {
    FlatScene {
        materials: vec![],
        batches: zs.iter().map(|&z| quad(50.0, z)).collect(),
        lines: vec![],
        bounds: None,
    }
}

/// Looking straight down at the origin from `distance` above it.
fn top(distance: f64, projection: Projection) -> ViewState {
    let mut camera = Camera {
        target: DVec3::ZERO,
        distance,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: 35f64.to_radians(),
        projection,
    };
    camera.set_standard_view(StandardView::Top);
    let mut s = ViewState::new();
    s.camera = camera;
    s
}

fn near(a: DVec3, b: DVec3) -> bool {
    (a - b).length() < 1e-6
}

fn same_orientation(a: &Camera, b: &Camera) {
    assert_eq!(a.yaw, b.yaw);
    assert_eq!(a.pitch, b.pitch);
    assert_eq!(a.fov_y, b.fov_y);
    assert_eq!(a.projection, b.projection);
    assert!((a.distance - b.distance).abs() < 1e-12);
}

#[test]
fn a_new_view_has_no_carry() {
    assert_eq!(ViewState::new().through_carry, 0.0);
}

#[test]
fn through_distance_stops_halfway_to_the_next_surface() {
    assert_eq!(through_distance(&[5.0, 9.0]), Some(7.0));
    assert_eq!(through_distance(&[9.0, 5.0, 20.0]), Some(7.0));
}

#[test]
fn through_distance_mirrors_when_nothing_is_beyond() {
    assert_eq!(through_distance(&[5.0]), Some(10.0));
}

#[test]
fn through_distance_treats_nearly_equal_hits_as_one_surface() {
    assert_eq!(through_distance(&[5.0, 5.0, 9.0]), Some(7.0));
    assert_eq!(through_distance(&[5.0, 5.0 * (1.0 + 1e-9), 9.0]), Some(7.0));
    assert_eq!(through_distance(&[5.0, 5.0]), Some(10.0));
    // A millimetre apart at 5 m is two surfaces.
    let d = through_distance(&[5.0, 5.001]).unwrap();
    assert!((d - 5.0005).abs() < 1e-12, "{d}");
}

#[test]
fn through_distance_ignores_what_is_not_ahead() {
    assert_eq!(through_distance(&[]), None);
    assert_eq!(through_distance(&[-1.0, 0.0]), None);
    assert_eq!(through_distance(&[f64::NAN, f64::INFINITY]), None);
    assert_eq!(through_distance(&[-3.0, f64::NAN, 4.0]), Some(8.0));
}

#[test]
fn one_notch_forward_lands_between_the_first_two_surfaces() {
    let scene = floors(&[1.0, 3.0]);
    let mut s = top(10.0, Projection::Perspective);
    let before = s.camera;
    s.scroll_through(1.0, CENTER, Some(&scene), VP);
    // Hits at 7 (z = 3) and 9 (z = 1): the eye travels 8, to z = 2.
    assert!(
        near(s.camera.eye(), before.eye() - DVec3::Z * 8.0),
        "{:?}",
        s.camera.eye()
    );
    assert!(near(s.camera.target, before.target - DVec3::Z * 8.0));
    same_orientation(&s.camera, &before);
    assert_eq!(s.through_carry, 0.0);
}

#[test]
fn the_last_surface_is_passed_by_the_distance_to_it() {
    let scene = floors(&[1.0, 3.0]);
    let mut s = top(10.0, Projection::Perspective);
    s.scroll_through(2.0, CENTER, Some(&scene), VP);
    // 10 -> 2 (between the floors), then 1 to the lower floor and 1 beyond it.
    assert!(
        (s.camera.eye().z - 0.0).abs() < 1e-6,
        "{:?}",
        s.camera.eye()
    );
}

#[test]
fn a_notch_back_steps_out_through_the_surface_behind() {
    let scene = floors(&[1.0, 3.0]);
    let mut s = top(10.0, Projection::Perspective);
    s.scroll_through(1.0, CENTER, Some(&scene), VP);
    let inside = s.camera;
    s.scroll_through(-1.0, CENTER, Some(&scene), VP);
    // From z = 2: the upper floor is 1 behind, nothing beyond it, so the eye goes to z = 4.
    assert!(
        near(s.camera.eye(), inside.eye() + DVec3::Z * 2.0),
        "{:?}",
        s.camera.eye()
    );
    same_orientation(&s.camera, &inside);
}

#[test]
fn fractions_of_a_notch_add_up() {
    let scene = floors(&[1.0, 3.0]);
    let mut s = top(10.0, Projection::Perspective);
    let before = s.camera;
    s.scroll_through(0.5, CENTER, Some(&scene), VP);
    assert_eq!(s.camera, before);
    assert_eq!(s.through_carry, 0.5);
    s.scroll_through(0.75, CENTER, Some(&scene), VP);
    assert!(
        (s.camera.eye().z - 2.0).abs() < 1e-6,
        "{:?}",
        s.camera.eye()
    );
    assert!((s.through_carry - 0.25).abs() < 1e-12);
    s.scroll_through(-0.5, CENTER, Some(&scene), VP);
    assert!((s.camera.eye().z - 2.0).abs() < 1e-6);
    assert!((s.through_carry + 0.25).abs() < 1e-12);
    s.scroll_through(-0.75, CENTER, Some(&scene), VP);
    assert!(
        (s.camera.eye().z - 4.0).abs() < 1e-6,
        "{:?}",
        s.camera.eye()
    );
    assert!(s.through_carry.abs() < 1e-12);
}

#[test]
fn off_center_the_eye_moves_along_the_cursor_ray() {
    let scene = floors(&[1.0]);
    let cursor = DVec2::new(650.0, 140.0);
    let mut s = top(10.0, Projection::Perspective);
    let before = s.camera;
    let (origin, dir) = Viewport {
        camera: before,
        size_px: VP,
    }
    .ray(cursor);
    let dir = dir.normalize();
    let hit = scene.raycast(origin, dir).expect("hit");
    s.scroll_through(1.0, cursor, Some(&scene), VP);
    // One surface only: the eye ends as far beyond it as it was in front.
    let t = (hit - origin).length();
    assert!(
        near(s.camera.eye(), origin + dir * 2.0 * t),
        "{:?}",
        s.camera.eye()
    );
    same_orientation(&s.camera, &before);
    // Still on the same ray, so the surface point stays under the cursor (now behind).
    let (o2, d2) = Viewport {
        camera: s.camera,
        size_px: VP,
    }
    .ray(cursor);
    assert!(d2.normalize().distance(dir) < 1e-9);
    assert!((o2 - origin).cross(dir).length() < 1e-6);
}

#[test]
fn with_nothing_ahead_a_notch_is_a_plain_wheel_zoom() {
    let scene = floors(&[20.0]); // above the eye, behind a forward step
    let mut s = top(10.0, Projection::Perspective);
    let mut plain = top(10.0, Projection::Perspective);
    s.scroll_through(1.0, CENTER, Some(&scene), VP);
    plain.scroll_zoom(1.0, CENTER, Some(&scene), VP);
    assert_eq!(s.camera, plain.camera);
}

#[test]
fn with_nothing_behind_a_notch_back_is_a_plain_wheel_zoom() {
    let scene = floors(&[1.0]);
    let mut s = top(10.0, Projection::Perspective);
    let mut plain = top(10.0, Projection::Perspective);
    s.scroll_through(-1.0, CENTER, Some(&scene), VP);
    plain.scroll_zoom(-1.0, CENTER, Some(&scene), VP);
    assert_eq!(s.camera, plain.camera);
}

#[test]
fn without_a_scene_or_in_parallel_it_is_the_wheel_zoom() {
    let scene = floors(&[1.0, 3.0]);
    let mut s = top(10.0, Projection::Parallel);
    let mut plain = top(10.0, Projection::Parallel);
    s.through_carry = 0.5;
    s.scroll_through(1.5, CENTER, Some(&scene), VP);
    plain.scroll_zoom(1.5, CENTER, Some(&scene), VP);
    assert_eq!(s.camera, plain.camera);
    assert_eq!(s.through_carry, 0.0);

    let mut s = top(10.0, Projection::Perspective);
    let mut plain = top(10.0, Projection::Perspective);
    s.through_carry = -0.5;
    s.scroll_through(0.5, CENTER, None, VP);
    plain.scroll_zoom(0.5, CENTER, None, VP);
    assert_eq!(s.camera, plain.camera);
    assert_eq!(s.through_carry, 0.0);
}

#[test]
fn the_drag_pivots_are_left_alone() {
    let scene = floors(&[1.0, 3.0]);
    let mut s = top(10.0, Projection::Perspective);
    s.pivot = Some(DVec3::new(1.0, 2.0, 3.0));
    s.zoom_pivot = Some(DVec3::new(-1.0, 0.0, 0.0));
    s.scroll_through(1.0, CENTER, Some(&scene), VP);
    s.scroll_through(-1.0, CENTER, Some(&scene), VP);
    assert_eq!(s.pivot, Some(DVec3::new(1.0, 2.0, 3.0)));
    assert_eq!(s.zoom_pivot, Some(DVec3::new(-1.0, 0.0, 0.0)));
}

#[test]
fn zero_lines_change_nothing() {
    let scene = floors(&[1.0, 3.0]);
    let mut s = top(10.0, Projection::Perspective);
    let before = s.camera;
    s.scroll_through(0.0, CENTER, Some(&scene), VP);
    assert_eq!(s.camera, before);
    assert_eq!(s.through_carry, 0.0);
}
