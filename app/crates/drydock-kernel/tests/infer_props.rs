// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Inference invariants (`docs/specs/m2-inference.md`).

use drydock_kernel::infer::*;
use drydock_kernel::topology::*;
use glam::{DVec2, DVec3};
use proptest::prelude::*;

const AXES: [DVec3; 3] = [DVec3::X, DVec3::Y, DVec3::Z];
const RADIUS: f64 = 5.0;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// Looking straight down, parallel: 10 pixels per meter.
fn top_project(q: DVec3) -> Option<DVec2> {
    Some(DVec2::new(q.x, q.y) * 10.0)
}

fn top(x: f64, y: f64) -> Probe<'static> {
    Probe {
        cursor: DVec2::new(x, y) * 10.0,
        origin: v(x, y, 100.0),
        dir: v(0.0, 0.0, -1.0),
        parallel: true,
        project: &top_project,
        radius: RADIUS,
        anchor: None,
        axes: AXES,
        centers: &[],
        lock: None,
    }
}

fn grid(c: i32) -> f64 {
    f64::from(c) * 0.5
}

/// A segment between two grid points (half-meter steps).
type Seg = ((i32, i32, i32), (i32, i32, i32));

fn segments() -> impl Strategy<Value = Vec<Seg>> {
    let pt = (-20i32..=20, -20i32..=20, 0i32..=6);
    prop::collection::vec((pt.clone(), pt), 0..8)
}

fn build(segs: &[Seg]) -> Context {
    let mut ctx = Context::new();
    for (a, b) in segs {
        let a = v(grid(a.0), grid(a.1), grid(a.2));
        let b = v(grid(b.0), grid(b.1), grid(b.2));
        if a.distance(b) < 0.1 {
            continue;
        }
        let mut log = Edit::new();
        let _ = ctx.add_edge(a, b, &mut log);
    }
    ctx
}

fn screen_dist(probe: &Probe, p: DVec3) -> f64 {
    (probe.project)(p).unwrap().distance(probe.cursor)
}

fn cursor() -> impl Strategy<Value = (f64, f64)> {
    (-11.0f64..11.0, -11.0f64..11.0)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Snapped points are within the radius on screen; snap points are exact.
    #[test]
    fn snapped_points_are_near_cursor(segs in segments(), (x, y) in cursor()) {
        let ctx = build(&segs);
        let probe = top(x, y);
        let r = infer(&ctx, &probe).expect("top view always lands somewhere");
        match r.snap {
            Snap::Endpoint(id) => {
                prop_assert_eq!(r.point, ctx.vertex(id).unwrap().position);
                prop_assert!(screen_dist(&probe, r.point) <= RADIUS);
            }
            Snap::Midpoint(id) => {
                let e = ctx.edge(id).unwrap();
                let a = ctx.vertex(e.ends[0]).unwrap().position;
                let b = ctx.vertex(e.ends[1]).unwrap().position;
                prop_assert_eq!(r.point, (a + b) / 2.0);
                prop_assert!(screen_dist(&probe, r.point) <= RADIUS);
            }
            Snap::OnEdge(id) => {
                let e = ctx.edge(id).unwrap();
                let a = ctx.vertex(e.ends[0]).unwrap().position;
                let b = ctx.vertex(e.ends[1]).unwrap().position;
                let on = drydock_kernel::geom::point_segment_distance(r.point, a, b);
                prop_assert!(on < 1e-9, "off the edge by {}", on);
                prop_assert!(screen_dist(&probe, r.point) <= RADIUS + 1e-9);
            }
            Snap::OnFace(_) | Snap::Ground => {
                prop_assert!(screen_dist(&probe, r.point) < 1e-6);
                if r.snap == Snap::Ground {
                    prop_assert!(r.point.z.abs() < 1e-9);
                }
            }
            other => prop_assert!(false, "unexpected snap {:?}", other),
        }
    }

    /// With no faces nothing is hidden: when some vertex is in range, the result is the
    /// endpoint nearest on screen.
    #[test]
    fn endpoints_win_when_in_range(segs in segments(), (x, y) in cursor()) {
        let ctx = build(&segs);
        prop_assume!(ctx.face_count() == 0);
        let probe = top(x, y);
        let best = ctx
            .vertices()
            .map(|(_, vx)| screen_dist(&probe, vx.position))
            .fold(f64::INFINITY, f64::min);
        let r = infer(&ctx, &probe).unwrap();
        if best <= RADIUS {
            match r.snap {
                Snap::Endpoint(_) => {
                    prop_assert!((screen_dist(&probe, r.point) - best).abs() < 1e-12);
                }
                other => prop_assert!(false, "expected an endpoint, got {:?}", other),
            }
        } else {
            prop_assert!(!matches!(r.snap, Snap::Endpoint(_)));
        }
    }

    /// Under a lock the point is always on the lock line.
    #[test]
    fn lock_stays_on_line(
        segs in segments(),
        (x, y) in cursor(),
        anchor in (-5.0f64..5.0, -5.0f64..5.0, 0.0f64..3.0),
        dir in (-1.0f64..1.0, -1.0f64..1.0, -1.0f64..1.0),
    ) {
        let d = v(dir.0, dir.1, dir.2);
        prop_assume!(d.length() > 0.05);
        let ctx = build(&segs);
        let a = v(anchor.0, anchor.1, anchor.2);
        let probe = Probe { anchor: Some(a), lock: Some(d), ..top(x, y) };
        let r = infer(&ctx, &probe).expect("a lock always answers");
        let u = d.normalize();
        let off = (r.point - a) - u * (r.point - a).dot(u);
        prop_assert!(off.length() < 1e-9, "off the lock line by {}", off.length());
        prop_assert!(matches!(
            r.snap,
            Snap::Endpoint(_) | Snap::Midpoint(_) | Snap::Center(_) | Snap::Along
        ));
    }

    /// `ray_face` against stacked squares returns the highest square under the origin
    /// containing the ray's xy.
    #[test]
    fn ray_face_matches_brute_force(
        squares in prop::collection::vec((-10i32..10, -10i32..10, 1i32..8), 1..5),
        (x, y) in (-12.0f64..12.0, -12.0f64..12.0),
        height in 0.0f64..20.0,
    ) {
        let mut ctx = Context::new();
        let mut placed = Vec::new();
        for (i, (x0, y0, size)) in squares.iter().enumerate() {
            let z = i as f64 * 2.0;
            let (x0, y0, s) = (f64::from(*x0), f64::from(*y0), f64::from(*size));
            let pts = [v(x0, y0, z), v(x0 + s, y0, z), v(x0 + s, y0 + s, z), v(x0, y0 + s, z)];
            let mut log = Edit::new();
            for k in 0..4 {
                ctx.add_edge(pts[k], pts[(k + 1) % 4], &mut log).unwrap();
            }
            placed.push((x0, y0, s, z));
        }
        // Keep clear of boundaries.
        for (x0, y0, s, _) in &placed {
            for edge in [*x0, x0 + s] {
                prop_assume!((x - edge).abs() > 1e-6);
            }
            for edge in [*y0, y0 + s] {
                prop_assume!((y - edge).abs() > 1e-6);
            }
        }
        let want = placed
            .iter()
            .filter(|(x0, y0, s, z)| {
                *z < height - 1e-6 && x > *x0 && x < x0 + s && y > *y0 && y < y0 + s
            })
            .map(|(_, _, _, z)| *z)
            .fold(None, |m: Option<f64>, z| Some(m.map_or(z, |m| m.max(z))));
        let got = ray_face(&ctx, v(x, y, height), v(0.0, 0.0, -1.0));
        match (want, got) {
            (None, None) => {}
            (Some(z), Some((_, t))) => prop_assert!((t - (height - z)).abs() < 1e-9),
            (w, g) => prop_assert!(false, "want {:?}, got {:?}", w, g),
        }
    }

    /// Any finite view, perspective or parallel, never panics.
    #[test]
    fn any_view_is_safe(
        segs in segments(),
        eye in (-30.0f64..30.0, -30.0f64..30.0, -30.0f64..30.0),
        dir in (-1.0f64..1.0, -1.0f64..1.0, -1.0f64..1.0),
        parallel in any::<bool>(),
        anchor in prop::option::of((-5.0f64..5.0, -5.0f64..5.0, -5.0f64..5.0)),
        radius in -1.0f64..50.0,
    ) {
        let ctx = build(&segs);
        let e = v(eye.0, eye.1, eye.2);
        let project = move |q: DVec3| Some(DVec2::new(q.x - e.x, q.y - e.y) * 10.0);
        let centers = [v(0.0, 0.0, 0.0)];
        let probe = Probe {
            cursor: DVec2::ZERO,
            origin: e,
            dir: v(dir.0, dir.1, dir.2),
            parallel,
            project: &project,
            radius,
            anchor: anchor.map(|a| v(a.0, a.1, a.2)),
            axes: AXES,
            centers: &centers,
            lock: None,
        };
        let _ = infer(&ctx, &probe);
        let _ = pick(&ctx, &probe);
        let locked = Probe { lock: Some(DVec3::X), ..probe };
        let _ = infer(&ctx, &locked);
    }
}
