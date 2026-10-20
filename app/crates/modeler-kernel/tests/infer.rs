// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Snaps, axis inference, direction lock and picking (`docs/specs/m2-inference.md`).

use glam::{DVec2, DVec3};
use modeler_kernel::infer::*;
use modeler_kernel::select::Entity;
use modeler_kernel::topology::*;

const AXES: [DVec3; 3] = [DVec3::X, DVec3::Y, DVec3::Z];

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn close(a: DVec3, b: DVec3) -> bool {
    a.distance(b) < 1e-9
}

fn assert_close(a: DVec3, b: DVec3) {
    assert!(close(a, b), "{a:?} != {b:?}");
}

/// Looking straight down, parallel: 10 pixels per meter, screen = 10 * (x, y).
fn top_project(q: DVec3) -> Option<DVec2> {
    Some(DVec2::new(q.x, q.y) * 10.0)
}

/// Probe for the top view with the cursor over world point (x, y). Radius 5 px (0.5 m).
fn top(x: f64, y: f64) -> Probe<'static> {
    Probe {
        cursor: DVec2::new(x, y) * 10.0,
        origin: v(x, y, 100.0),
        dir: v(0.0, 0.0, -1.0),
        parallel: true,
        project: &top_project,
        radius: 5.0,
        anchor: None,
        axes: AXES,
        centers: &[],
        lock: None,
    }
}

fn iso_dir() -> DVec3 {
    v(-1.0, -1.0, -1.0).normalize()
}

fn iso_right() -> DVec3 {
    v(1.0, -1.0, 0.0).normalize()
}

fn iso_up() -> DVec3 {
    iso_dir().cross(iso_right())
}

/// Parallel view from (+1, +1, +1): 10 pixels per meter.
fn iso_project(q: DVec3) -> Option<DVec2> {
    Some(DVec2::new(q.dot(iso_right()), q.dot(iso_up())) * 10.0)
}

/// Probe for the iso view with the cursor at screen pixel `c`.
fn iso_at(c: DVec2) -> Probe<'static> {
    let origin = iso_right() * (c.x / 10.0) + iso_up() * (c.y / 10.0) - iso_dir() * 100.0;
    Probe {
        cursor: c,
        origin,
        dir: iso_dir(),
        parallel: true,
        project: &iso_project,
        radius: 5.0,
        anchor: None,
        axes: AXES,
        centers: &[],
        lock: None,
    }
}

/// Probe for the iso view with the cursor over world point `q`.
fn iso(q: DVec3) -> Probe<'static> {
    iso_at(iso_project(q).unwrap())
}

/// Perspective: eye at (0, 0, 10) looking down -Z, focal length 100 px.
fn persp_project(q: DVec3) -> Option<DVec2> {
    let w = 10.0 - q.z;
    if w <= 1e-9 {
        return None;
    }
    Some(DVec2::new(q.x, q.y) * (100.0 / w))
}

fn persp_at(c: DVec2) -> Probe<'static> {
    Probe {
        cursor: c,
        origin: v(0.0, 0.0, 10.0),
        dir: v(c.x / 100.0, c.y / 100.0, -1.0),
        parallel: false,
        project: &persp_project,
        radius: 5.0,
        anchor: None,
        axes: AXES,
        centers: &[],
        lock: None,
    }
}

fn add(ctx: &mut Context, a: DVec3, b: DVec3) {
    let mut log = Edit::new();
    ctx.add_edge(a, b, &mut log).unwrap();
}

fn polygon(ctx: &mut Context, pts: &[DVec3]) {
    for i in 0..pts.len() {
        add(ctx, pts[i], pts[(i + 1) % pts.len()]);
    }
}

/// Horizontal square with corner (x0, y0, z) and side `size`.
fn square(x0: f64, y0: f64, z: f64, size: f64) -> Vec<DVec3> {
    vec![
        v(x0, y0, z),
        v(x0 + size, y0, z),
        v(x0 + size, y0 + size, z),
        v(x0, y0 + size, z),
    ]
}

fn vertex(ctx: &Context, p: DVec3) -> VertexId {
    ctx.find_vertex(p, 1e-9).expect("vertex")
}

fn edge(ctx: &Context, a: DVec3, b: DVec3) -> EdgeId {
    ctx.edge_between(vertex(ctx, a), vertex(ctx, b))
        .expect("edge")
}

fn only_face(ctx: &Context) -> FaceId {
    let faces: Vec<FaceId> = ctx.faces().map(|(id, _)| id).collect();
    assert_eq!(faces.len(), 1, "expected one face");
    faces[0]
}

/// The face whose outer loop has exactly the vertices at `pts` (any order).
fn face(ctx: &Context, pts: &[DVec3]) -> FaceId {
    let mut want: Vec<VertexId> = pts.iter().map(|q| vertex(ctx, *q)).collect();
    want.sort();
    for (id, f) in ctx.faces() {
        let mut have = ctx.loop_vertices(&f.outer);
        have.sort();
        if have == want {
            return id;
        }
    }
    panic!("no face with outer loop {pts:?}");
}

fn infer_ok(ctx: &Context, probe: &Probe) -> Inference {
    infer(ctx, probe).expect("an inference")
}

// ---------------------------------------------------------------- ray_face

#[test]
fn ray_face_hits_square_from_above() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(0.0, 0.0, 0.0, 10.0));
    let f = only_face(&ctx);
    let (id, t) = ray_face(&ctx, v(3.0, 4.0, 7.0), v(0.0, 0.0, -2.0)).unwrap();
    assert_eq!(id, f);
    assert!((t - 7.0).abs() < 1e-9, "t = {t}");
}

#[test]
fn ray_face_misses_outside_and_hits_boundary() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(0.0, 0.0, 0.0, 10.0));
    let down = v(0.0, 0.0, -1.0);
    assert_eq!(ray_face(&ctx, v(11.0, 4.0, 5.0), down), None);
    assert!(ray_face(&ctx, v(10.0, 4.0, 5.0), down).is_some());
    assert!(ray_face(&ctx, v(0.0, 0.0, 5.0), down).is_some());
}

#[test]
fn ray_face_from_below_hits_too() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(0.0, 0.0, 0.0, 10.0));
    let (_, t) = ray_face(&ctx, v(5.0, 5.0, -3.0), v(0.0, 0.0, 1.0)).unwrap();
    assert!((t - 3.0).abs() < 1e-9);
}

#[test]
fn ray_face_ignores_faces_behind_origin() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(0.0, 0.0, 0.0, 10.0));
    assert_eq!(ray_face(&ctx, v(5.0, 5.0, 5.0), v(0.0, 0.0, 1.0)), None);
}

#[test]
fn ray_face_misses_inside_hole_but_hits_hole_boundary() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(0.0, 0.0, 0.0, 10.0));
    polygon(&mut ctx, &square(3.0, 3.0, 0.0, 4.0));
    let outer = face(&ctx, &square(0.0, 0.0, 0.0, 10.0));
    let inner = face(&ctx, &square(3.0, 3.0, 0.0, 4.0));
    let down = v(0.0, 0.0, -1.0);
    assert_eq!(ray_face(&ctx, v(1.0, 1.0, 5.0), down).unwrap().0, outer);
    assert_eq!(ray_face(&ctx, v(5.0, 5.0, 5.0), down).unwrap().0, inner);
    let mut log = Edit::new();
    ctx.delete_face(inner, &mut log).unwrap();
    assert_eq!(ray_face(&ctx, v(5.0, 5.0, 5.0), down), None);
    assert_eq!(ray_face(&ctx, v(3.0, 5.0, 5.0), down).unwrap().0, outer);
}

#[test]
fn ray_face_takes_nearest_of_stacked_faces() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(0.0, 0.0, 0.0, 10.0));
    polygon(&mut ctx, &square(0.0, 0.0, 2.0, 10.0));
    let high = face(&ctx, &square(0.0, 0.0, 2.0, 10.0));
    let low = face(&ctx, &square(0.0, 0.0, 0.0, 10.0));
    let down = v(0.0, 0.0, -1.0);
    let (id, t) = ray_face(&ctx, v(5.0, 5.0, 5.0), down).unwrap();
    assert_eq!(id, high);
    assert!((t - 3.0).abs() < 1e-9);
    let (id, _) = ray_face(&ctx, v(5.0, 5.0, 1.0), down).unwrap();
    assert_eq!(id, low);
}

#[test]
fn ray_face_skips_edge_on_faces() {
    let mut ctx = Context::new();
    polygon(
        &mut ctx,
        &[
            v(0.0, 0.0, 0.0),
            v(10.0, 0.0, 0.0),
            v(10.0, 0.0, 10.0),
            v(0.0, 0.0, 10.0),
        ],
    );
    assert_eq!(ctx.face_count(), 1);
    assert_eq!(ray_face(&ctx, v(5.0, 0.0, 20.0), v(0.0, 0.0, -1.0)), None);
}

#[test]
fn ray_face_rejects_bad_direction() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(0.0, 0.0, 0.0, 10.0));
    assert_eq!(ray_face(&ctx, v(5.0, 5.0, 5.0), DVec3::ZERO), None);
    assert_eq!(
        ray_face(&ctx, v(5.0, 5.0, 5.0), v(0.0, f64::NAN, -1.0)),
        None
    );
}

// ---------------------------------------------------------------- ground and bad input

#[test]
fn empty_space_lands_on_ground() {
    let ctx = Context::new();
    let r = infer_ok(&ctx, &top(3.0, -4.0));
    assert_eq!(r.snap, Snap::Ground);
    assert_close(r.point, v(3.0, -4.0, 0.0));
    assert_eq!(r.axis, None);
}

#[test]
fn ground_goes_through_anchor() {
    let ctx = Context::new();
    let probe = Probe {
        anchor: Some(v(-7.0, -7.0, 2.0)),
        ..top(3.0, 4.0)
    };
    let r = infer_ok(&ctx, &probe);
    assert_eq!(r.snap, Snap::Ground);
    assert_close(r.point, v(3.0, 4.0, 2.0));
}

#[test]
fn ground_behind_or_parallel_is_none() {
    let ctx = Context::new();
    let up = Probe {
        dir: v(0.0, 0.0, 1.0),
        ..top(3.0, 4.0)
    };
    assert_eq!(infer(&ctx, &up), None);
    let sideways = Probe {
        dir: v(1.0, 0.0, 0.0),
        ..top(3.0, 4.0)
    };
    assert_eq!(infer(&ctx, &sideways), None);
}

#[test]
fn bad_probe_is_none() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(0.0, 0.0, 0.0, 10.0));
    let zero = Probe {
        dir: DVec3::ZERO,
        ..top(5.0, 5.0)
    };
    assert_eq!(infer(&ctx, &zero), None);
    assert_eq!(pick(&ctx, &zero), None);
    let nan = Probe {
        origin: v(f64::NAN, 0.0, 100.0),
        ..top(5.0, 5.0)
    };
    assert_eq!(infer(&ctx, &nan), None);
    assert_eq!(pick(&ctx, &nan), None);
    let cursor = Probe {
        cursor: DVec2::new(f64::INFINITY, 0.0),
        ..top(5.0, 5.0)
    };
    assert_eq!(infer(&ctx, &cursor), None);
}

#[test]
fn unnormalized_direction_is_fine() {
    let ctx = Context::new();
    let probe = Probe {
        dir: v(0.0, 0.0, -42.0),
        ..top(1.0, 2.0)
    };
    assert_close(infer_ok(&ctx, &probe).point, v(1.0, 2.0, 0.0));
}

// ---------------------------------------------------------------- point snaps

#[test]
fn endpoint_snaps_exactly() {
    let mut ctx = Context::new();
    add(&mut ctx, v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0));
    let r = infer_ok(&ctx, &top(10.3, 0.2));
    assert_eq!(r.snap, Snap::Endpoint(vertex(&ctx, v(10.0, 0.0, 0.0))));
    assert_eq!(r.point, v(10.0, 0.0, 0.0));
}

#[test]
fn endpoint_out_of_range_does_not_snap() {
    let mut ctx = Context::new();
    add(&mut ctx, v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0));
    let r = infer_ok(&ctx, &top(10.6, 0.0));
    assert_eq!(r.snap, Snap::Ground);
    let r = infer_ok(&ctx, &top(10.5, 0.0));
    assert_eq!(r.snap, Snap::Endpoint(vertex(&ctx, v(10.0, 0.0, 0.0))));
}

#[test]
fn nearest_endpoint_wins() {
    let mut ctx = Context::new();
    add(&mut ctx, v(0.0, 0.0, 0.0), v(0.6, 0.0, 0.0));
    let r = infer_ok(&ctx, &top(0.35, 0.0));
    assert_eq!(r.snap, Snap::Endpoint(vertex(&ctx, v(0.6, 0.0, 0.0))));
    let r = infer_ok(&ctx, &top(0.25, 0.0));
    assert_eq!(r.snap, Snap::Endpoint(vertex(&ctx, v(0.0, 0.0, 0.0))));
}

#[test]
fn endpoint_beats_nearer_midpoint() {
    let mut ctx = Context::new();
    add(&mut ctx, v(0.0, 0.0, 0.0), v(0.8, 0.0, 0.0));
    // Midpoint (0.4, 0) is 0.05 away, the end (0.8, 0) is 0.35 away.
    let r = infer_ok(&ctx, &top(0.45, 0.0));
    assert_eq!(r.snap, Snap::Endpoint(vertex(&ctx, v(0.8, 0.0, 0.0))));
}

#[test]
fn equal_screen_distance_goes_to_smaller_depth() {
    let mut ctx = Context::new();
    add(&mut ctx, v(5.0, 5.0, 0.0), v(5.0, 5.0, 3.0));
    let r = infer_ok(&ctx, &top(5.1, 5.0));
    assert_eq!(r.snap, Snap::Endpoint(vertex(&ctx, v(5.0, 5.0, 3.0))));
    assert_eq!(r.point, v(5.0, 5.0, 3.0));
}

#[test]
fn midpoint_snaps_exactly() {
    let mut ctx = Context::new();
    add(&mut ctx, v(0.0, 0.0, 0.0), v(10.0, 2.0, 0.0));
    let e = edge(&ctx, v(0.0, 0.0, 0.0), v(10.0, 2.0, 0.0));
    let r = infer_ok(&ctx, &top(5.2, 1.3));
    assert_eq!(r.snap, Snap::Midpoint(e));
    assert_eq!(r.point, v(5.0, 1.0, 0.0));
}

#[test]
fn center_snaps_but_loses_to_endpoint_and_midpoint() {
    let mut ctx = Context::new();
    add(&mut ctx, v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0));
    let centers = [v(20.0, 0.0, 0.0), v(10.1, 0.0, 0.0), v(5.1, 0.0, 0.0)];
    let at = |x: f64, y: f64| Probe {
        centers: &centers,
        ..top(x, y)
    };
    let r = infer_ok(&ctx, &at(20.2, 0.1));
    assert_eq!(r.snap, Snap::Center(0));
    assert_eq!(r.point, v(20.0, 0.0, 0.0));
    let r = infer_ok(&ctx, &at(10.1, 0.0));
    assert_eq!(r.snap, Snap::Endpoint(vertex(&ctx, v(10.0, 0.0, 0.0))));
    let r = infer_ok(&ctx, &at(5.1, 0.0));
    assert!(matches!(r.snap, Snap::Midpoint(_)), "{:?}", r.snap);
}

#[test]
fn nearest_center_wins() {
    let ctx = Context::new();
    let centers = [v(0.0, 0.0, 0.0), v(0.5, 0.0, 0.0)];
    let probe = Probe {
        centers: &centers,
        ..top(0.3, 0.0)
    };
    assert_eq!(infer_ok(&ctx, &probe).snap, Snap::Center(1));
}

// ---------------------------------------------------------------- edges and faces

#[test]
fn on_edge_point_is_on_the_edge_under_cursor() {
    let mut ctx = Context::new();
    add(&mut ctx, v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0));
    let e = edge(&ctx, v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0));
    let r = infer_ok(&ctx, &top(2.0, 0.3));
    assert_eq!(r.snap, Snap::OnEdge(e));
    assert_close(r.point, v(2.0, 0.0, 0.0));
}

#[test]
fn on_edge_of_diagonal() {
    let mut ctx = Context::new();
    add(&mut ctx, v(0.0, 0.0, 0.0), v(10.0, 10.0, 0.0));
    let e = edge(&ctx, v(0.0, 0.0, 0.0), v(10.0, 10.0, 0.0));
    let r = infer_ok(&ctx, &top(3.2, 2.8));
    assert_eq!(r.snap, Snap::OnEdge(e));
    assert_close(r.point, v(3.0, 3.0, 0.0));
}

#[test]
fn pick_vertical_edge_in_top_view() {
    let mut ctx = Context::new();
    add(&mut ctx, v(5.0, 5.0, 0.0), v(5.0, 5.0, 3.0));
    // The edge runs along the view ray: its on-edge point is the end nearer the eye.
    let e = edge(&ctx, v(5.0, 5.0, 0.0), v(5.0, 5.0, 3.0));
    assert_eq!(pick(&ctx, &top(5.2, 5.0)), Some(Entity::Edge(e)));
}

#[test]
fn edge_beats_face_near_boundary() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(0.0, 0.0, 0.0, 10.0));
    let e = edge(&ctx, v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0));
    let r = infer_ok(&ctx, &top(3.0, 0.4));
    assert_eq!(r.snap, Snap::OnEdge(e));
    assert_close(r.point, v(3.0, 0.0, 0.0));
}

#[test]
fn face_interior_is_on_face() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(0.0, 0.0, 1.5, 10.0));
    let f = only_face(&ctx);
    let r = infer_ok(&ctx, &top(3.0, 4.0));
    assert_eq!(r.snap, Snap::OnFace(f));
    assert_close(r.point, v(3.0, 4.0, 1.5));
}

#[test]
fn face_beats_ground() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(0.0, 0.0, -2.0, 10.0));
    let r = infer_ok(&ctx, &top(3.0, 4.0));
    assert!(matches!(r.snap, Snap::OnFace(_)));
    assert_close(r.point, v(3.0, 4.0, -2.0));
}

// ---------------------------------------------------------------- axes

fn anchored(x: f64, y: f64) -> Probe<'static> {
    Probe {
        anchor: Some(v(0.0, 0.0, 0.0)),
        ..top(x, y)
    }
}

#[test]
fn red_axis_from_anchor() {
    let ctx = Context::new();
    let r = infer_ok(&ctx, &anchored(20.0, 0.3));
    assert_eq!(r.snap, Snap::OnAxis(0));
    assert_close(r.point, v(20.0, 0.0, 0.0));
    assert_eq!(r.axis, Some(0));
}

#[test]
fn green_axis_both_directions() {
    let ctx = Context::new();
    let r = infer_ok(&ctx, &anchored(-0.2, -15.0));
    assert_eq!(r.snap, Snap::OnAxis(1));
    assert_close(r.point, v(0.0, -15.0, 0.0));
    assert_eq!(r.axis, Some(1));
}

#[test]
fn off_axis_is_ground_without_axis() {
    let ctx = Context::new();
    let r = infer_ok(&ctx, &anchored(20.0, 0.6));
    assert_eq!(r.snap, Snap::Ground);
    assert_eq!(r.axis, None);
}

#[test]
fn no_axis_without_anchor() {
    let ctx = Context::new();
    let r = infer_ok(&ctx, &top(20.0, 0.1));
    assert_eq!(r.snap, Snap::Ground);
    assert_eq!(r.axis, None);
}

#[test]
fn axis_parallel_to_view_is_skipped() {
    // Blue axis points straight at a top-view camera; near the anchor only red and green
    // are candidates, and the nearer one wins.
    let ctx = Context::new();
    let r = infer_ok(
        &ctx,
        &Probe {
            anchor: Some(v(0.0, 0.0, 0.0)),
            ..top(0.1, 0.3)
        },
    );
    assert_eq!(r.snap, Snap::OnAxis(1));
}

#[test]
fn blue_axis_in_iso_view() {
    let ctx = Context::new();
    let probe = Probe {
        anchor: Some(v(0.0, 0.0, 0.0)),
        ..iso_at(iso_project(v(0.0, 0.0, 5.0)).unwrap() + DVec2::new(2.0, 0.0))
    };
    let r = infer_ok(&ctx, &probe);
    assert_eq!(r.snap, Snap::OnAxis(2));
    assert_eq!(r.axis, Some(2));
    assert!((r.point.x).abs() < 1e-9 && (r.point.y).abs() < 1e-9);
    assert!((r.point.z - 5.0).abs() < 0.3, "{:?}", r.point);
}

#[test]
fn rotated_axes() {
    let ctx = Context::new();
    let s = std::f64::consts::FRAC_1_SQRT_2;
    let probe = Probe {
        anchor: Some(v(0.0, 0.0, 0.0)),
        axes: [v(s, s, 0.0), v(-s, s, 0.0), DVec3::Z],
        ..top(10.0, 10.2)
    };
    let r = infer_ok(&ctx, &probe);
    assert_eq!(r.snap, Snap::OnAxis(0));
    assert_close(r.point, v(10.1, 10.1, 0.0));
}

#[test]
fn edge_and_midpoint_beat_axis() {
    let mut ctx = Context::new();
    add(&mut ctx, v(20.0, -2.0, 0.0), v(20.0, 8.0, 0.0));
    add(&mut ctx, v(30.0, -5.0, 0.0), v(30.0, 5.0, 0.0));
    let a = edge(&ctx, v(20.0, -2.0, 0.0), v(20.0, 8.0, 0.0));
    let b = edge(&ctx, v(30.0, -5.0, 0.0), v(30.0, 5.0, 0.0));
    // Cursor near the red axis where it crosses edge `a`: the edge wins.
    let r = infer_ok(&ctx, &anchored(20.2, 0.1));
    assert_eq!(r.snap, Snap::OnEdge(a));
    assert_close(r.point, v(20.0, 0.1, 0.0));
    // The midpoint of `b` sits on the red axis.
    let r = infer_ok(&ctx, &anchored(30.0, 0.2));
    assert_eq!(r.snap, Snap::Midpoint(b));
    assert_eq!(r.axis, Some(0));
}

#[test]
fn axis_beats_face() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(-10.0, -10.0, 0.0, 20.0));
    let r = infer_ok(&ctx, &anchored(5.0, 0.3));
    assert_eq!(r.snap, Snap::OnAxis(0));
    assert_close(r.point, v(5.0, 0.0, 0.0));
    let r = infer_ok(&ctx, &anchored(5.0, 3.0));
    assert!(matches!(r.snap, Snap::OnFace(_)));
    assert_eq!(r.axis, None);
}

#[test]
fn endpoint_on_axis_reports_axis() {
    let mut ctx = Context::new();
    add(&mut ctx, v(8.0, 0.0, 0.0), v(8.0, 4.0, 0.0));
    let r = infer_ok(&ctx, &anchored(8.1, 0.0));
    assert_eq!(r.snap, Snap::Endpoint(vertex(&ctx, v(8.0, 0.0, 0.0))));
    assert_eq!(r.axis, Some(0));
    let r = infer_ok(&ctx, &anchored(8.1, 4.0));
    assert_eq!(r.axis, None);
}

#[test]
fn point_at_anchor_has_no_axis() {
    let mut ctx = Context::new();
    add(&mut ctx, v(0.0, 0.0, 0.0), v(8.0, 0.0, 0.0));
    let r = infer_ok(&ctx, &anchored(0.1, 0.1));
    assert_eq!(r.snap, Snap::Endpoint(vertex(&ctx, v(0.0, 0.0, 0.0))));
    assert_eq!(r.axis, None);
}

// ---------------------------------------------------------------- hidden points

#[test]
fn vertex_under_a_face_is_hidden() {
    let mut ctx = Context::new();
    add(&mut ctx, v(5.0, 5.0, 0.0), v(5.0, 8.0, 0.0));
    polygon(&mut ctx, &square(0.0, 0.0, 2.0, 10.0));
    let roof = face(&ctx, &square(0.0, 0.0, 2.0, 10.0));
    let r = infer_ok(&ctx, &top(5.0, 5.0));
    assert_eq!(r.snap, Snap::OnFace(roof));
    assert_close(r.point, v(5.0, 5.0, 2.0));
    assert_eq!(pick(&ctx, &top(5.0, 6.0)), Some(Entity::Face(roof)));
}

#[test]
fn own_face_does_not_hide_its_corner() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(0.0, 0.0, 0.0, 10.0));
    let corner = v(10.0, 10.0, 0.0);
    let r = infer_ok(&ctx, &iso(corner));
    assert_eq!(r.snap, Snap::Endpoint(vertex(&ctx, corner)));
}

#[test]
fn far_corner_behind_wall_is_hidden_in_iso() {
    // Wall in the plane x = 0; the viewer sits at +x, so the segment at x = -5 is behind it.
    let mut ctx = Context::new();
    polygon(
        &mut ctx,
        &[
            v(0.0, -20.0, -20.0),
            v(0.0, 20.0, -20.0),
            v(0.0, 20.0, 20.0),
            v(0.0, -20.0, 20.0),
        ],
    );
    let wall = only_face(&ctx);
    add(&mut ctx, v(-5.0, 0.0, 0.0), v(-5.0, 3.0, 0.0));
    let r = infer_ok(&ctx, &iso(v(-5.0, 0.0, 0.0)));
    assert_eq!(r.snap, Snap::OnFace(wall));
    // The same segment in front of the wall snaps.
    add(&mut ctx, v(5.0, 0.0, 0.0), v(5.0, 3.0, 0.0));
    let r = infer_ok(&ctx, &iso(v(5.0, 0.0, 0.0)));
    assert_eq!(r.snap, Snap::Endpoint(vertex(&ctx, v(5.0, 0.0, 0.0))));
}

#[test]
fn hidden_center_and_midpoint() {
    let mut ctx = Context::new();
    add(&mut ctx, v(1.0, 5.0, 0.0), v(9.0, 5.0, 0.0));
    polygon(&mut ctx, &square(0.0, 0.0, 2.0, 10.0));
    let centers = [v(3.0, 3.0, 0.0), v(3.0, 3.0, 4.0)];
    let roof = face(&ctx, &square(0.0, 0.0, 2.0, 10.0));
    let r = infer_ok(&ctx, &top(5.0, 5.0));
    assert_eq!(r.snap, Snap::OnFace(roof));
    let probe = Probe {
        centers: &centers,
        ..top(3.0, 3.0)
    };
    assert_eq!(infer_ok(&ctx, &probe).snap, Snap::Center(1));
    let probe = Probe {
        centers: &centers[..1],
        ..top(3.0, 3.0)
    };
    assert_eq!(infer_ok(&ctx, &probe).snap, Snap::OnFace(roof));
}

#[test]
fn parallel_points_behind_origin_are_skipped() {
    // The ray starts at z = 1; the segment at z = 3 projects onto the cursor but is behind it.
    let mut ctx = Context::new();
    add(&mut ctx, v(5.0, 5.0, 3.0), v(5.0, 8.0, 3.0));
    let probe = Probe {
        origin: v(5.0, 5.0, 1.0),
        ..top(5.0, 5.0)
    };
    let r = infer_ok(&ctx, &probe);
    assert_eq!(r.snap, Snap::Ground);
    assert_close(r.point, v(5.0, 5.0, 0.0));
    assert_eq!(pick(&ctx, &probe), None);
}

// ---------------------------------------------------------------- perspective

#[test]
fn perspective_endpoint_and_face() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(-2.0, -2.0, 0.0, 4.0));
    let corner = v(2.0, 2.0, 0.0);
    let c = persp_project(corner).unwrap();
    let r = infer_ok(&ctx, &persp_at(c + DVec2::new(1.0, -2.0)));
    assert_eq!(r.snap, Snap::Endpoint(vertex(&ctx, corner)));
    let r = infer_ok(&ctx, &persp_at(DVec2::new(5.0, 7.0)));
    assert!(matches!(r.snap, Snap::OnFace(_)));
    assert_close(r.point, v(0.5, 0.7, 0.0));
}

#[test]
fn perspective_ignores_points_behind_eye() {
    let mut ctx = Context::new();
    add(&mut ctx, v(0.0, 0.0, 20.0), v(0.0, 1.0, 20.0));
    let r = infer_ok(&ctx, &persp_at(DVec2::new(0.0, 0.0)));
    assert_eq!(r.snap, Snap::Ground);
    assert_close(r.point, v(0.0, 0.0, 0.0));
}

#[test]
fn perspective_ground_hidden_vertex() {
    // A vertex straight below a face, seen through the eye: hidden.
    let mut ctx = Context::new();
    add(&mut ctx, v(0.0, 0.0, -1.0), v(0.0, 0.5, -1.0));
    polygon(&mut ctx, &square(-2.0, -2.0, 0.0, 4.0));
    let r = infer_ok(&ctx, &persp_at(DVec2::new(0.0, 0.0)));
    assert!(matches!(r.snap, Snap::OnFace(_)), "{:?}", r.snap);
}

// ---------------------------------------------------------------- lock

fn locked(x: f64, y: f64, dir: DVec3) -> Probe<'static> {
    Probe {
        anchor: Some(v(0.0, 0.0, 0.0)),
        lock: Some(dir),
        ..top(x, y)
    }
}

#[test]
fn lock_keeps_point_on_line() {
    let ctx = Context::new();
    let r = infer_ok(&ctx, &locked(7.0, 4.0, v(3.0, 0.0, 0.0)));
    assert_eq!(r.snap, Snap::Along);
    assert_close(r.point, v(7.0, 0.0, 0.0));
    assert_eq!(r.axis, Some(0));
}

#[test]
fn lock_projects_endpoint_onto_line() {
    let mut ctx = Context::new();
    add(&mut ctx, v(3.0, 6.0, 0.0), v(4.0, 9.0, 0.0));
    let r = infer_ok(&ctx, &locked(3.1, 6.0, DVec3::X));
    assert_eq!(r.snap, Snap::Endpoint(vertex(&ctx, v(3.0, 6.0, 0.0))));
    assert_close(r.point, v(3.0, 0.0, 0.0));
}

#[test]
fn lock_ignores_edges_faces_and_axes() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(2.0, 2.0, 0.0, 10.0));
    let d = v(1.0, 1.0, 0.0);
    let r = infer_ok(&ctx, &locked(5.0, 2.1, d));
    assert_eq!(r.snap, Snap::Along);
    assert_close(r.point, v(3.55, 3.55, 0.0));
    assert_eq!(r.axis, None);
}

#[test]
fn lock_parallel_to_view_gives_anchor() {
    let ctx = Context::new();
    let r = infer_ok(&ctx, &locked(4.0, 4.0, DVec3::Z));
    assert_eq!(r.snap, Snap::Along);
    assert_close(r.point, v(0.0, 0.0, 0.0));
    assert_eq!(r.axis, None);
}

#[test]
fn lock_vertical_in_iso() {
    let ctx = Context::new();
    let probe = Probe {
        anchor: Some(v(0.0, 0.0, 0.0)),
        lock: Some(DVec3::Z),
        ..iso(v(0.0, 0.0, 4.0))
    };
    let r = infer_ok(&ctx, &probe);
    assert_eq!(r.snap, Snap::Along);
    assert_close(r.point, v(0.0, 0.0, 4.0));
    assert_eq!(r.axis, Some(2));
}

#[test]
fn lock_without_anchor_or_with_zero_direction_is_ignored() {
    let ctx = Context::new();
    let probe = Probe {
        lock: Some(DVec3::X),
        ..top(7.0, 4.0)
    };
    assert_eq!(infer_ok(&ctx, &probe).snap, Snap::Ground);
    let r = infer_ok(&ctx, &locked(7.0, 4.0, DVec3::ZERO));
    assert_eq!(r.snap, Snap::Ground);
    let r = infer_ok(&ctx, &locked(7.0, 4.0, v(f64::NAN, 0.0, 0.0)));
    assert_eq!(r.snap, Snap::Ground);
}

#[test]
fn lock_always_answers_even_looking_up() {
    let ctx = Context::new();
    let probe = Probe {
        dir: v(0.0, 0.0, 1.0),
        ..locked(7.0, 4.0, DVec3::X)
    };
    let r = infer_ok(&ctx, &probe);
    assert_eq!(r.snap, Snap::Along);
}

// ---------------------------------------------------------------- pick

#[test]
fn pick_edge_then_face_then_nothing() {
    let mut ctx = Context::new();
    polygon(&mut ctx, &square(0.0, 0.0, 0.0, 10.0));
    let e = edge(&ctx, v(10.0, 0.0, 0.0), v(10.0, 10.0, 0.0));
    let f = only_face(&ctx);
    assert_eq!(pick(&ctx, &top(9.7, 4.0)), Some(Entity::Edge(e)));
    assert_eq!(pick(&ctx, &top(10.4, 4.0)), Some(Entity::Edge(e)));
    assert_eq!(pick(&ctx, &top(5.0, 4.0)), Some(Entity::Face(f)));
    assert_eq!(pick(&ctx, &top(11.0, 4.0)), None);
}

#[test]
fn pick_takes_nearest_edge_on_screen() {
    let mut ctx = Context::new();
    add(&mut ctx, v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0));
    add(&mut ctx, v(0.0, 0.6, 0.0), v(10.0, 0.6, 0.0));
    let low = edge(&ctx, v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0));
    let high = edge(&ctx, v(0.0, 0.6, 0.0), v(10.0, 0.6, 0.0));
    assert_eq!(pick(&ctx, &top(5.0, 0.2)), Some(Entity::Edge(low)));
    assert_eq!(pick(&ctx, &top(5.0, 0.4)), Some(Entity::Edge(high)));
}

#[test]
fn pick_ignores_anchor_lock_and_centers() {
    let mut ctx = Context::new();
    add(&mut ctx, v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0));
    let e = edge(&ctx, v(0.0, 0.0, 0.0), v(10.0, 0.0, 0.0));
    let centers = [v(5.0, 0.0, 0.0)];
    let probe = Probe {
        anchor: Some(v(0.0, 3.0, 0.0)),
        lock: Some(DVec3::Y),
        centers: &centers,
        ..top(5.0, 0.1)
    };
    assert_eq!(pick(&ctx, &probe), Some(Entity::Edge(e)));
}

#[test]
fn pick_hidden_edge_is_face_in_front() {
    let mut ctx = Context::new();
    add(&mut ctx, v(1.0, 5.0, 0.0), v(9.0, 5.0, 0.0));
    polygon(&mut ctx, &square(0.0, 0.0, 2.0, 10.0));
    let roof = face(&ctx, &square(0.0, 0.0, 2.0, 10.0));
    assert_eq!(pick(&ctx, &top(3.0, 5.1)), Some(Entity::Face(roof)));
}
