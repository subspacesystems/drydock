// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use glam::DVec3;
use modeler_kernel::geom;
use modeler_kernel::topology::*;
use proptest::prelude::*;

#[derive(Clone, Debug)]
enum Step {
    /// An axis-aligned segment from a grid point, `len` cells long, along x or y.
    Segment {
        x: i32,
        y: i32,
        along_x: bool,
        len: i32,
    },
    Erase(usize),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        5 => (0..=4i32, 0..=4i32, any::<bool>(), prop_oneof![3 => Just(1), 1 => -1..=3i32])
            .prop_filter("not empty", |(_, _, _, len)| *len != 0)
            .prop_map(|(x, y, along_x, len)| Step::Segment { x, y, along_x, len }),
        1 => any::<usize>().prop_map(Step::Erase),
    ]
}

fn area(ctx: &Context, f: FaceId) -> f64 {
    let face = ctx.face(f).unwrap();
    let mut a = geom::area_vector(&ctx.loop_points(&face.outer)).length();
    for l in &face.inner {
        a -= geom::area_vector(&ctx.loop_points(l)).length();
    }
    a
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

/// Flat faces made from grid edges face up and cover whole cells.
fn check_faces(ctx: &Context) -> Result<(), TestCaseError> {
    prop_assert_eq!(ctx.validate(), vec![]);
    prop_assert_eq!(ctx.face_overlaps(), vec![]);
    let mut total = 0.0;
    for (f, _) in ctx.faces() {
        let n = ctx.face_plane(f).unwrap().normal;
        prop_assert!(n.distance(DVec3::Z) < 1e-9, "face {:?} faces {}", f, n);
        let a = area(ctx, f);
        prop_assert!(
            a > 0.5 && (a - a.round()).abs() < 1e-9,
            "face {:?} area {}",
            f,
            a
        );
        total += a;
    }
    // Segments stay within -1..=7 on both axes.
    prop_assert!(total < 64.0 + 1e-9);
    Ok(())
}

fn draw(ctx: &mut Context, a: DVec3, b: DVec3) -> Result<Edit, TestCaseError> {
    let before = ctx.clone();
    let mut log = Edit::new();
    let chain = ctx.add_edge(a, b, &mut log);
    prop_assert!(chain.is_ok());
    for (f, _) in &chain.unwrap().faces {
        prop_assert!(ctx.face(*f).is_some());
        prop_assert!(before.face(*f).is_none());
    }
    check_undo_redo(&before, ctx, &log)?;
    Ok(log)
}

type GridEdge = ((i32, i32), (i32, i32));

/// Every unit edge of an `nx` by `ny` grid of 1 m cells: `(start, end)` corner coordinates.
fn grid_edges(nx: i32, ny: i32) -> Vec<GridEdge> {
    let mut out = vec![];
    for y in 0..=ny {
        for x in 0..nx {
            out.push(((x, y), (x + 1, y)));
        }
    }
    for x in 0..=nx {
        for y in 0..ny {
            out.push(((x, y), (x, y + 1)));
        }
    }
    out
}

fn pt((x, y): (i32, i32)) -> DVec3 {
    DVec3::new(x as f64, y as f64, 0.0)
}

fn full_grid(nx: i32, ny: i32) -> Context {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    for (a, b) in grid_edges(nx, ny) {
        ctx.add_edge(pt(a), pt(b), &mut log).unwrap();
    }
    ctx
}

fn shuffled_grid() -> impl Strategy<Value = (i32, i32, Vec<GridEdge>, Vec<bool>)> {
    (1..=4i32, 1..=4i32).prop_flat_map(|(nx, ny)| {
        let edges = grid_edges(nx, ny);
        let n = edges.len();
        (
            Just(nx),
            Just(ny),
            Just(edges).prop_shuffle(),
            prop::collection::vec(any::<bool>(), n),
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn grid_drawings_with_erases_keep_faces_valid(steps in prop::collection::vec(step(), 1..24)) {
        let mut ctx = Context::new();
        for s in steps {
            match s {
                Step::Segment { x, y, along_x, len } => {
                    let a = DVec3::new(x as f64, y as f64, 0.0);
                    let d = if along_x { DVec3::X } else { DVec3::Y };
                    let b = a + d * len as f64;
                    draw(&mut ctx, a, b)?;
                }
                Step::Erase(i) => {
                    if ctx.edge_count() == 0 {
                        continue;
                    }
                    let e = ctx.edges().nth(i % ctx.edge_count()).unwrap().0;
                    let before = ctx.clone();
                    let mut log = Edit::new();
                    prop_assert_eq!(ctx.erase_edge(e, &mut log), Ok(()));
                    prop_assert!(ctx.edge(e).is_none());
                    check_undo_redo(&before, &ctx, &log)?;
                }
            }
            check_faces(&ctx)?;
        }
    }

    #[test]
    fn full_grid_in_any_order_has_one_face_per_cell((nx, ny, edges, flips) in shuffled_grid()) {
        let mut ctx = Context::new();
        for ((a, b), flip) in edges.iter().zip(&flips) {
            let (a, b) = if *flip { (*b, *a) } else { (*a, *b) };
            draw(&mut ctx, pt(a), pt(b))?;
            check_faces(&ctx)?;
        }
        prop_assert_eq!(ctx.face_count(), (nx * ny) as usize);
        for (f, face) in ctx.faces() {
            prop_assert!((area(&ctx, f) - 1.0).abs() < 1e-9);
            prop_assert_eq!(face.outer.uses.len(), 4);
            prop_assert!(face.inner.is_empty());
        }
    }

    #[test]
    fn erasing_inner_vertical_edges_merges_cells_into_strips(
        nx in 2..=4i32,
        ny in 1..=3i32,
        picks in prop::collection::vec(any::<bool>(), 9),
    ) {
        let mut ctx = full_grid(nx, ny);
        check_faces(&ctx)?;
        let mut erased = 0;
        let mut k = 0;
        for x in 1..nx {
            for y in 0..ny {
                let pick = picks[k % picks.len()];
                k += 1;
                if !pick {
                    continue;
                }
                let e = ctx
                    .edge_between(
                        ctx.find_vertex(pt((x, y)), 1e-9).unwrap(),
                        ctx.find_vertex(pt((x, y + 1)), 1e-9).unwrap(),
                    )
                    .unwrap();
                let before = ctx.clone();
                let mut log = Edit::new();
                prop_assert_eq!(ctx.erase_edge(e, &mut log), Ok(()));
                check_undo_redo(&before, &ctx, &log)?;
                erased += 1;
                check_faces(&ctx)?;
            }
        }
        prop_assert_eq!(ctx.face_count(), (nx * ny - erased) as usize);
        let total: f64 = ctx.faces().map(|(f, _)| area(&ctx, f)).sum();
        prop_assert!((total - (nx * ny) as f64).abs() < 1e-9);
    }
}
