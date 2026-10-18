// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m0-camera: worked examples. See docs/specs/m0-camera.md.

use std::f64::consts::{FRAC_PI_2, PI};

use glam::{DVec2, DVec3, DVec4};
use modeler_view::{
    Bounds, Camera, MIN_DISTANCE, ORBIT_DEGREES_PER_PIXEL, Projection, StandardView,
    ZOOM_STEP_FACTOR,
};

const EPS: f64 = 1e-9;

fn cam(view: StandardView, projection: Projection) -> Camera {
    let mut c = Camera {
        target: DVec3::ZERO,
        distance: 10.0,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: 35f64.to_radians(),
        projection,
    };
    c.set_standard_view(view);
    c
}

fn near(a: DVec3, b: DVec3) -> bool {
    (a - b).length() < 1e-9
}

#[test]
fn standard_views_have_the_documented_axes() {
    let (f, r, u) = cam(StandardView::Front, Projection::Perspective).basis();
    assert!(
        near(f, DVec3::Y) && near(r, DVec3::X) && near(u, DVec3::Z),
        "front {f} {r} {u}"
    );

    let (f, r, u) = cam(StandardView::Top, Projection::Perspective).basis();
    assert!(
        near(f, -DVec3::Z) && near(r, DVec3::X) && near(u, DVec3::Y),
        "top {f} {r} {u}"
    );

    let (f, r, _) = cam(StandardView::Right, Projection::Perspective).basis();
    assert!(near(f, -DVec3::X) && near(r, DVec3::Y), "right {f} {r}");

    let (f, r, _) = cam(StandardView::Left, Projection::Perspective).basis();
    assert!(near(f, DVec3::X) && near(r, -DVec3::Y), "left {f} {r}");

    let (f, _, _) = cam(StandardView::Back, Projection::Perspective).basis();
    assert!(near(f, -DVec3::Y), "back {f}");

    let (f, _, u) = cam(StandardView::Bottom, Projection::Perspective).basis();
    assert!(
        near(f, DVec3::Z) && (u.length() - 1.0).abs() < EPS,
        "bottom {f} {u}"
    );
}

#[test]
fn iso_shows_x_and_y_at_thirty_degrees() {
    let c = cam(StandardView::Iso, Projection::Parallel);
    let (o, _) = c.project(DVec3::ZERO, 1.0).unwrap();
    let (x, _) = c.project(DVec3::X, 1.0).unwrap();
    let (y, _) = c.project(DVec3::Y, 1.0).unwrap();
    let dx = x - o;
    let dy = y - o;
    assert!(dx.x > 0.0 && dx.y < 0.0, "+X goes down-right: {dx}");
    assert!(dy.x > 0.0 && dy.y > 0.0, "+Y goes up-right: {dy}");
    let thirty = 30f64.to_radians().tan();
    assert!((-dx.y / dx.x - thirty).abs() < 1e-9);
    assert!((dy.y / dy.x - thirty).abs() < 1e-9);
}

#[test]
fn eye_and_view_matrix() {
    let mut c = cam(StandardView::Front, Projection::Perspective);
    c.target = DVec3::new(1.0, 2.0, 3.0);
    assert!(near(c.eye(), DVec3::new(1.0, -8.0, 3.0)));
    let v = c.view_matrix();
    assert!(near(v.transform_point3(c.eye()), DVec3::ZERO));
    assert!(near(
        v.transform_point3(c.target),
        DVec3::new(0.0, 0.0, -10.0)
    ));
    assert!(near(
        v.transform_point3(c.target + DVec3::X),
        DVec3::new(1.0, 0.0, -10.0)
    ));
    assert!(near(
        v.transform_point3(c.target + DVec3::Z),
        DVec3::new(0.0, 1.0, -10.0)
    ));
}

#[test]
fn projection_edges() {
    let c = cam(StandardView::Front, Projection::Perspective);
    let half = 10.0 * (c.fov_y / 2.0).tan();
    let (ndc, depth) = c.project(DVec3::new(0.0, 0.0, half), 1.5).unwrap();
    assert!((ndc - DVec2::new(0.0, 1.0)).length() < 1e-9, "{ndc}");
    assert!((depth - 10.0).abs() < EPS);
    let (ndc, _) = c.project(DVec3::new(half * 1.5, 0.0, 0.0), 1.5).unwrap();
    assert!((ndc - DVec2::new(1.0, 0.0)).length() < 1e-9, "{ndc}");
    assert!(
        c.project(DVec3::new(0.0, -20.0, 0.0), 1.0).is_none(),
        "behind the eye"
    );

    let p = cam(StandardView::Front, Projection::Parallel);
    let (ndc, _) = p.project(DVec3::new(0.0, 100.0, half), 1.0).unwrap();
    assert!(
        (ndc - DVec2::new(0.0, 1.0)).length() < 1e-9,
        "parallel ignores depth: {ndc}"
    );
    assert!(
        p.project(DVec3::new(0.0, -20.0, 0.0), 1.0).is_some(),
        "parallel sees behind the eye plane"
    );
}

#[test]
fn near_and_far_enclose_the_scene() {
    let c = cam(StandardView::Front, Projection::Perspective);
    let m = c.projection_matrix(1.0, Some((DVec3::ZERO, 1.0))) * c.view_matrix();
    let clip = |p: DVec3| {
        let v = m * DVec4::new(p.x, p.y, p.z, 1.0);
        v.z / v.w
    };
    assert!(
        clip(DVec3::new(0.0, -1.0, 0.0)).abs() < 1e-6,
        "near plane at depth 9"
    );
    assert!(
        (clip(DVec3::new(0.0, 1.0, 0.0)) - 1.0).abs() < 1e-6,
        "far plane at depth 11"
    );
}

#[test]
fn orbit_turns_about_the_target() {
    let mut c = cam(StandardView::Front, Projection::Perspective);
    let px = 90.0 / ORBIT_DEGREES_PER_PIXEL;
    c.orbit(px, 0.0);
    assert!(
        (c.yaw - (-PI)).abs() < 1e-9 || (c.yaw - PI).abs() < 1e-9,
        "yaw {}",
        c.yaw
    );
    assert!(
        near(c.eye(), DVec3::new(-10.0, 0.0, 0.0)),
        "eye {}",
        c.eye()
    );
    c.orbit(0.0, 10_000.0);
    assert_eq!(c.pitch, FRAC_PI_2, "pitch clamps at straight down");
    c.orbit(0.0, -100_000.0);
    assert_eq!(c.pitch, -FRAC_PI_2);
}

#[test]
fn pan_moves_the_target_in_the_view_plane() {
    let mut c = cam(StandardView::Front, Projection::Perspective);
    let upp = c.units_per_pixel(600.0);
    assert!((upp - 2.0 * 10.0 * (c.fov_y / 2.0).tan() / 600.0).abs() < 1e-12);
    c.pan(100.0, 50.0, 600.0);
    assert!(
        near(c.target, DVec3::new(-100.0 * upp, 0.0, 50.0 * upp)),
        "{}",
        c.target
    );
    assert_eq!(c.distance, 10.0);
}

#[test]
fn zoom_scales_distance_and_keeps_the_cursor_point() {
    let mut c = cam(StandardView::Front, Projection::Perspective);
    c.zoom(1.0, DVec2::ZERO, 1.0);
    assert!((c.distance - 10.0 * ZOOM_STEP_FACTOR).abs() < 1e-12);
    assert!(
        near(c.target, DVec3::ZERO),
        "zooming at the center keeps the target"
    );

    let mut c = cam(StandardView::Front, Projection::Parallel);
    let cursor = DVec2::new(0.5, -0.25);
    let p = c.target_plane_point(cursor, 2.0);
    c.zoom(3.0, cursor, 2.0);
    let (after, _) = c.project(p, 2.0).unwrap();
    assert!((after - cursor).length() < 1e-9, "{after}");

    c.zoom(1000.0, DVec2::ZERO, 1.0);
    assert_eq!(c.distance, MIN_DISTANCE);
}

#[test]
fn target_plane_point_at_center_is_the_target() {
    let mut c = cam(StandardView::Iso, Projection::Perspective);
    c.target = DVec3::new(3.0, -4.0, 5.0);
    assert!(near(c.target_plane_point(DVec2::ZERO, 1.3), c.target));
}

#[test]
fn zoom_extents_centers_and_fits() {
    let b = Bounds {
        min: DVec3::ZERO,
        max: DVec3::new(2.0, 4.0, 6.0),
    };
    for projection in [Projection::Perspective, Projection::Parallel] {
        for aspect in [0.5, 1.0, 2.0] {
            let mut c = cam(StandardView::Iso, projection);
            c.zoom_extents(&b, aspect);
            assert!(near(c.target, DVec3::new(1.0, 2.0, 3.0)));
            for i in 0..8 {
                let p = DVec3::new(
                    if i & 1 == 0 { b.min.x } else { b.max.x },
                    if i & 2 == 0 { b.min.y } else { b.max.y },
                    if i & 4 == 0 { b.min.z } else { b.max.z },
                );
                let (ndc, _) = c.project(p, aspect).unwrap();
                assert!(
                    ndc.abs().max_element() <= 1.0 + 1e-9,
                    "{projection:?} {aspect}: {p} -> {ndc}"
                );
            }
        }
    }
}

#[test]
fn default_is_an_iso_view_of_the_origin() {
    let c = Camera::default();
    assert!(near(c.target, DVec3::ZERO));
    assert!((c.yaw - (-45f64).to_radians()).abs() < 1e-12);
    assert!((c.pitch - (1.0 / 3f64.sqrt()).asin()).abs() < 1e-12);
    assert_eq!(c.projection, Projection::Perspective);
}
