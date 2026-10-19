// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use glam::DVec3;
use modeler_kernel::geom;
use modeler_kernel::sticky::*;
use modeler_kernel::topology::*;
use proptest::prelude::*;

#[derive(Clone, Debug)]
enum Step {
    Segment(DVec3, DVec3),
    Erase(usize),
}

/// Points on a half-meter grid: x and y in 0..=4 m; z is 0 when `flat`, else 0..=2 m.
fn grid_point(flat: bool) -> impl Strategy<Value = DVec3> {
    let z = if flat { 0..=0 } else { 0..=4 };
    (0..=8i32, 0..=8i32, z).prop_map(|(x, y, z)| DVec3::new(x as f64, y as f64, z as f64) * 0.5)
}

fn steps(flat: bool, erases: bool) -> impl Strategy<Value = Vec<Step>> {
    let segment = (grid_point(flat), grid_point(flat)).prop_map(|(a, b)| Step::Segment(a, b));
    let erase_weight = if erases { 1 } else { 0 };
    let step = prop_oneof![
        4 => segment,
        erase_weight => any::<usize>().prop_map(Step::Erase),
    ];
    prop::collection::vec(step, 1..14)
}

fn near_edge(ctx: &Context, q: DVec3, tol: f64) -> bool {
    ctx.edges().any(|(_, e)| {
        let a = ctx.vertex(e.ends[0]).unwrap().position;
        let b = ctx.vertex(e.ends[1]).unwrap().position;
        geom::point_segment_distance(q, a, b) <= tol
    })
}

fn near_segment(drawn: &[(DVec3, DVec3)], q: DVec3, tol: f64) -> bool {
    drawn
        .iter()
        .any(|(a, b)| geom::point_segment_distance(q, *a, *b) <= tol)
}

fn check_undo_redo(before: &Context, after: &Context, log: &Edit) -> Result<(), TestCaseError> {
    let mut undone = after.clone();
    prop_assert_eq!(undone.apply_edit(&log.inverse()), Ok(()));
    prop_assert_eq!(&undone, before);
    let mut redone = before.clone();
    prop_assert_eq!(redone.apply_edit(log), Ok(()));
    prop_assert_eq!(&redone, after);
    Ok(())
}

fn check_chain(ctx: &Context, chain: &EdgeChain, a: DVec3, b: DVec3) -> Result<(), TestCaseError> {
    prop_assert_eq!(chain.vertices.len(), chain.edges.len() + 1);
    let first = ctx.vertex(chain.vertices[0]).unwrap().position;
    let last = ctx
        .vertex(*chain.vertices.last().unwrap())
        .unwrap()
        .position;
    prop_assert!(first.distance(a) < 1e-9);
    prop_assert!(last.distance(b) < 1e-9);
    let mut t_last = f64::NEG_INFINITY;
    for (i, v) in chain.vertices.iter().enumerate() {
        let q = ctx.vertex(*v).unwrap().position;
        prop_assert!(geom::point_segment_distance(q, a, b) < 1e-9);
        let t = (q - a).dot(b - a);
        prop_assert!(t > t_last);
        t_last = t;
        if i < chain.edges.len() {
            let ends = ctx.edge(chain.edges[i]).unwrap().ends;
            let next = chain.vertices[i + 1];
            prop_assert!(ends == [*v, next] || ends == [next, *v]);
        }
    }
    Ok(())
}

/// Run the steps, checking every invariant after each one.
fn run(steps: &[Step], check_cover: bool) -> Result<(), TestCaseError> {
    let mut ctx = Context::new();
    let mut drawn = vec![];
    let mut edits = vec![];
    for step in steps {
        let before = ctx.clone();
        let mut log = Edit::new();
        match step {
            Step::Segment(a, b) => match ctx.add_edge(*a, *b, &mut log) {
                Ok(chain) => {
                    prop_assert!(a != b);
                    check_chain(&ctx, &chain, *a, *b)?;
                    drawn.push((*a, *b));
                }
                Err(err) => {
                    prop_assert_eq!(a, b);
                    prop_assert_eq!(err, StickyError::Degenerate);
                    prop_assert_eq!(&ctx, &before);
                    prop_assert!(log.is_empty());
                }
            },
            Step::Erase(i) => {
                if ctx.edge_count() == 0 {
                    continue;
                }
                let e = ctx.edges().nth(i % ctx.edge_count()).unwrap().0;
                prop_assert_eq!(ctx.erase_edge(e, &mut log), Ok(()));
                prop_assert!(ctx.edge(e).is_none());
            }
        }
        prop_assert_eq!(ctx.validate(), vec![]);
        prop_assert_eq!(ctx.face_count(), 0);
        check_undo_redo(&before, &ctx, &log)?;
        // Every edge lies on something that was drawn.
        for (_, e) in ctx.edges() {
            let a = ctx.vertex(e.ends[0]).unwrap().position;
            let b = ctx.vertex(e.ends[1]).unwrap().position;
            for t in [0.0, 0.5, 1.0] {
                prop_assert!(near_segment(&drawn, a.lerp(b, t), 1e-7));
            }
        }
        // Without erases, everything drawn is covered by edges.
        if check_cover {
            for (a, b) in &drawn {
                for k in 0..=16 {
                    let q = a.lerp(*b, k as f64 / 16.0);
                    prop_assert!(near_edge(&ctx, q, 1e-7), "{} not covered", q);
                }
            }
        }
        edits.push(log);
    }
    // Undo everything, then redo everything.
    let end = ctx.clone();
    for log in edits.iter().rev() {
        prop_assert_eq!(ctx.apply_edit(&log.inverse()), Ok(()));
    }
    prop_assert_eq!(&ctx, &Context::new());
    for log in &edits {
        prop_assert_eq!(ctx.apply_edit(log), Ok(()));
    }
    prop_assert_eq!(&ctx, &end);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn flat_segments_cover_and_validate(steps in steps(true, false)) {
        run(&steps, true)?;
    }

    #[test]
    fn spatial_segments_cover_and_validate(steps in steps(false, false)) {
        run(&steps, true)?;
    }

    #[test]
    fn segments_and_erases_validate(steps in steps(true, true)) {
        run(&steps, false)?;
    }

    #[test]
    fn spatial_segments_and_erases_validate(steps in steps(false, true)) {
        run(&steps, false)?;
    }
}

// -- segments touching a grid of faces ---------------------------------------------------

/// A 3x3 grid of 1 m squares at z = 0, sharing edges, faces facing +z.
fn face_grid() -> Context {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let mut v = vec![];
    for j in 0..4 {
        for i in 0..4 {
            v.push(ctx.insert_vertex(DVec3::new(i as f64, j as f64, 0.0), &mut log));
        }
    }
    let at = |i: usize, j: usize| v[j * 4 + i];
    let mut h = vec![];
    let mut w = vec![];
    for j in 0..4 {
        for i in 0..3 {
            h.push(ctx.insert_edge(at(i, j), at(i + 1, j), &mut log).unwrap());
        }
    }
    for j in 0..3 {
        for i in 0..4 {
            w.push(ctx.insert_edge(at(i, j), at(i, j + 1), &mut log).unwrap());
        }
    }
    for j in 0..3 {
        for i in 0..3 {
            let uses = vec![
                EdgeUse::forward(h[j * 3 + i]),
                EdgeUse::forward(w[j * 4 + i + 1]),
                EdgeUse::backward(h[(j + 1) * 3 + i]),
                EdgeUse::backward(w[j * 4 + i]),
            ];
            ctx.insert_face(
                Face {
                    outer: Loop { uses },
                    inner: vec![],
                },
                &mut log,
            )
            .unwrap();
        }
    }
    ctx
}

/// Points on a half-meter grid around the faces, z in {-1, 0, 1}.
fn around_point() -> impl Strategy<Value = DVec3> {
    (-1..=7i32, -1..=7i32, -1..=1i32)
        .prop_map(|(x, y, z)| DVec3::new(x as f64 * 0.5, y as f64 * 0.5, z as f64))
}

/// Segments that meet the face plane in at most one point.
fn off_plane_segment() -> impl Strategy<Value = (DVec3, DVec3)> {
    (around_point(), around_point()).prop_map(|(a, mut b)| {
        if a.z == 0.0 && b.z == 0.0 {
            b.z = 1.0;
        }
        (a, b)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn faces_keep_their_shape(segs in prop::collection::vec(off_plane_segment(), 1..10)) {
        let mut ctx = face_grid();
        prop_assert_eq!(ctx.validate(), vec![]);
        let shapes: Vec<(FaceId, DVec3, Vec<DVec3>)> = ctx
            .faces()
            .map(|(f, face)| {
                let pts = ctx.loop_points(&face.outer);
                (f, geom::area_vector(&pts), pts)
            })
            .collect();
        for (a, b) in segs {
            let before = ctx.clone();
            let mut log = Edit::new();
            match ctx.add_edge(a, b, &mut log) {
                Ok(chain) => check_chain(&ctx, &chain, a, b)?,
                Err(err) => {
                    prop_assert_eq!(a, b);
                    prop_assert_eq!(err, StickyError::Degenerate);
                }
            }
            prop_assert_eq!(ctx.validate(), vec![]);
            check_undo_redo(&before, &ctx, &log)?;
            prop_assert_eq!(ctx.face_count(), 9);
            for (f, area, corners) in &shapes {
                let pts = ctx.loop_points(&ctx.face(*f).unwrap().outer);
                prop_assert!((geom::area_vector(&pts) - *area).length() < 1e-12);
                for c in corners {
                    prop_assert!(pts.iter().any(|q| q.distance(*c) < 1e-12));
                }
                for q in &pts {
                    prop_assert!(q.z == 0.0);
                }
            }
        }
    }
}
