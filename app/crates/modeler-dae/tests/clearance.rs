// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-clearance: distance between triangles, the triangles of two parts that are too close,
//! and the tinted scene. See docs/specs/m7-clearance.md.

use std::time::Instant;

use glam::{DMat4, DVec3};
use modeler_dae::{
    Aabb, Batch, DEFAULT_CLEARANCE_M, FIT_COLOR, FlatScene, LineBatch, Material, Triangle,
    close_triangles, instance_triangles, kernel_scene, tint_fit, triangle_distance,
};
use modeler_kernel::model::{
    self as km, BodyId, DefinitionId, DefinitionInfo, Instance, InstanceId, ModelEdit,
};
use modeler_kernel::topology::{Context, EdgeUse, Edit, Face, Loop, VertexId};
use proptest::prelude::*;

fn p(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// The 12 triangles of the box from `lo` to `hi`, two per side.
fn box_triangles(lo: DVec3, hi: DVec3) -> Vec<Triangle> {
    let c = |i: usize| {
        p(
            if i & 1 == 0 { lo.x } else { hi.x },
            if i & 2 == 0 { lo.y } else { hi.y },
            if i & 4 == 0 { lo.z } else { hi.z },
        )
    };
    // Corner indices of each side, as a ring.
    let sides: [[usize; 4]; 6] = [
        [0, 1, 3, 2],
        [4, 6, 7, 5],
        [0, 4, 5, 1],
        [1, 5, 7, 3],
        [3, 7, 6, 2],
        [2, 6, 4, 0],
    ];
    let mut out = Vec::new();
    for s in sides {
        out.push([c(s[0]), c(s[1]), c(s[2])]);
        out.push([c(s[0]), c(s[2]), c(s[3])]);
    }
    out
}

fn tri(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> Triangle {
    [
        p(a[0], a[1], a[2]),
        p(b[0], b[1], b[2]),
        p(c[0], c[1], c[2]),
    ]
}

// ---------------------------------------------------------------------------------------
// triangle_distance

#[test]
fn identical_triangles_touch() {
    let t = tri([0., 0., 0.], [1., 0., 0.], [0., 1., 0.]);
    assert_eq!(triangle_distance(&t, &t), 0.0);
}

#[test]
fn triangles_sharing_a_corner_or_an_edge_touch() {
    let a = tri([0., 0., 0.], [1., 0., 0.], [0., 1., 0.]);
    let corner = tri([0., 0., 0.], [-1., 0., 1.], [0., -1., 1.]);
    let edge = tri([0., 0., 0.], [1., 0., 0.], [0., 0., 1.]);
    assert!(triangle_distance(&a, &corner) < 1e-12);
    assert!(triangle_distance(&a, &edge) < 1e-12);
}

#[test]
fn crossing_triangles_have_distance_zero() {
    // A vertical triangle pierces the horizontal one: no corner touches the other.
    let flat = tri([0., 0., 0.], [4., 0., 0.], [0., 4., 0.]);
    let upright = tri([1., 1., -1.], [1., 1., 1.], [2., 1., 0.5]);
    assert!(triangle_distance(&flat, &upright) < 1e-12);
    assert!(triangle_distance(&upright, &flat) < 1e-12);
}

#[test]
fn overlapping_coplanar_triangles_have_distance_zero() {
    let a = tri([0., 0., 0.], [4., 0., 0.], [0., 4., 0.]);
    let b = tri([1., 1., 0.], [5., 1., 0.], [1., 5., 0.]);
    assert!(triangle_distance(&a, &b) < 1e-12);
}

#[test]
fn a_triangle_inside_the_other_in_the_same_plane_touches() {
    let big = tri([0., 0., 0.], [10., 0., 0.], [0., 10., 0.]);
    let small = tri([1., 1., 0.], [2., 1., 0.], [1., 2., 0.]);
    assert!(triangle_distance(&big, &small) < 1e-12);
}

#[test]
fn parallel_triangles_face_to_face() {
    let a = tri([0., 0., 0.], [4., 0., 0.], [0., 4., 0.]);
    let b = tri([0.5, 0.5, 0.3], [2., 0.5, 0.3], [0.5, 2., 0.3]);
    assert!((triangle_distance(&a, &b) - 0.3).abs() < 1e-12);
}

#[test]
fn corner_over_the_middle_of_a_face() {
    let a = tri([0., 0., 0.], [4., 0., 0.], [0., 4., 0.]);
    let b = tri([1., 1., 0.25], [2., 1., 1.], [1., 2., 1.]);
    assert!((triangle_distance(&a, &b) - 0.25).abs() < 1e-12);
}

#[test]
fn skew_edges_are_nearest() {
    // An edge along x at z = 0 and one along y at z = 0.5 pass over each other.
    let a = tri([-1., 0., 0.], [1., 0., 0.], [0., -1., -1.]);
    let b = tri([0., -1., 0.5], [0., 1., 0.5], [1., 0., 1.5]);
    assert!((triangle_distance(&a, &b) - 0.5).abs() < 1e-12);
}

#[test]
fn corner_to_corner() {
    let a = tri([0., 0., 0.], [1., 0., 0.], [0., 1., 0.]);
    let b = tri([2., 2., 0.], [3., 2., 0.], [2., 3., 1.]);
    // The corners (1, 0) and (0, 1) are sqrt(5) from (2, 2); the point (0.5, 0.5) on the
    // long side is sqrt(4.5) away.
    assert!((triangle_distance(&a, &b) - 4.5f64.sqrt()).abs() < 1e-12);
}

#[test]
fn a_degenerate_triangle_counts_as_a_segment() {
    let a = tri([0., 0., 0.], [4., 0., 0.], [0., 4., 0.]);
    let sliver = tri([1., 1., 2.], [2., 1., 2.], [3., 1., 2.]);
    assert!((triangle_distance(&a, &sliver) - 2.0).abs() < 1e-12);
}

proptest! {
    #[test]
    fn distance_is_symmetric_and_not_negative(
        a in prop::array::uniform3(prop::array::uniform3(-5.0f64..5.0)),
        b in prop::array::uniform3(prop::array::uniform3(-5.0f64..5.0)),
    ) {
        let (a, b) = (tri(a[0], a[1], a[2]), tri(b[0], b[1], b[2]));
        let d = triangle_distance(&a, &b);
        let e = triangle_distance(&b, &a);
        prop_assert!(d >= 0.0);
        prop_assert!((d - e).abs() < 1e-9, "{d} vs {e}");
    }

    #[test]
    fn distance_is_at_most_the_nearest_corners(
        a in prop::array::uniform3(prop::array::uniform3(-5.0f64..5.0)),
        b in prop::array::uniform3(prop::array::uniform3(-5.0f64..5.0)),
    ) {
        let (a, b) = (tri(a[0], a[1], a[2]), tri(b[0], b[1], b[2]));
        let nearest = a
            .iter()
            .flat_map(|&u| b.iter().map(move |&v| u.distance(v)))
            .fold(f64::INFINITY, f64::min);
        prop_assert!(triangle_distance(&a, &b) <= nearest + 1e-9);
    }

    #[test]
    fn sliding_a_triangle_along_a_plane_normal_gives_that_distance(
        a in prop::array::uniform3(prop::array::uniform2(-5.0f64..5.0)),
        h in 0.001f64..3.0,
    ) {
        // Horizontal triangle and its copy lifted by h: the distance is h.
        let flat: Triangle = [p(a[0][0], a[0][1], 0.0), p(a[1][0], a[1][1], 0.0), p(a[2][0], a[2][1], 0.0)];
        let lifted = flat.map(|q| q + p(0.0, 0.0, h));
        prop_assert!((triangle_distance(&flat, &lifted) - h).abs() < 1e-9);
    }

    #[test]
    fn distance_does_not_change_when_both_move(
        a in prop::array::uniform3(prop::array::uniform3(-5.0f64..5.0)),
        b in prop::array::uniform3(prop::array::uniform3(-5.0f64..5.0)),
        by in prop::array::uniform3(-20.0f64..20.0),
    ) {
        let (a, b) = (tri(a[0], a[1], a[2]), tri(b[0], b[1], b[2]));
        let by = p(by[0], by[1], by[2]);
        let d = triangle_distance(&a, &b);
        let e = triangle_distance(&a.map(|q| q + by), &b.map(|q| q + by));
        prop_assert!((d - e).abs() < 1e-8, "{d} vs {e}");
    }
}

// ---------------------------------------------------------------------------------------
// close_triangles

const MM: f64 = 0.001;
const SIDE: f64 = 20.0 * MM;

/// Two boxes 20 mm wide with `gap` between the facing sides (A's +X and B's -X).
fn two_boxes(gap: f64) -> (Vec<Triangle>, Vec<Triangle>) {
    (
        box_triangles(p(0., 0., 0.), p(SIDE, SIDE, SIDE)),
        box_triangles(p(SIDE + gap, 0., 0.), p(2.0 * SIDE + gap, SIDE, SIDE)),
    )
}

/// Indices of the triangles whose three corners all have the given x.
fn on_x(list: &[Triangle], x: f64) -> Vec<usize> {
    (0..list.len())
        .filter(|&i| list[i].iter().all(|q| (q.x - x).abs() < 1e-12))
        .collect()
}

#[test]
fn the_default_clearance_is_two_tenths_of_a_millimeter() {
    assert_eq!(DEFAULT_CLEARANCE_M, 0.0002);
}

#[test]
fn boxes_closer_than_the_clearance_are_flagged() {
    let c = DEFAULT_CLEARANCE_M;
    let (a, b) = two_boxes(0.5 * c);
    let r = close_triangles(&a, &b, c);
    let facing_a = on_x(&a, SIDE);
    let facing_b = on_x(&b, SIDE + 0.5 * c);
    assert_eq!(facing_a.len(), 2);
    assert_eq!(facing_b.len(), 2);
    for i in &facing_a {
        assert!(r.a.contains(i), "facing triangle {i} of A not flagged");
    }
    for i in &facing_b {
        assert!(r.b.contains(i), "facing triangle {i} of B not flagged");
    }
}

#[test]
fn the_far_sides_are_not_flagged() {
    let c = DEFAULT_CLEARANCE_M;
    let (a, b) = two_boxes(0.5 * c);
    let r = close_triangles(&a, &b, c);
    for i in on_x(&a, 0.0) {
        assert!(!r.a.contains(&i), "far side of A flagged");
    }
    for i in on_x(&b, 2.0 * SIDE + 0.5 * c) {
        assert!(!r.b.contains(&i), "far side of B flagged");
    }
}

#[test]
fn boxes_farther_apart_than_the_clearance_are_not_flagged() {
    let c = DEFAULT_CLEARANCE_M;
    for gap in [1.5 * c, 5.0 * c, 1000.0 * c] {
        let (a, b) = two_boxes(gap);
        assert_eq!(close_triangles(&a, &b, c), Default::default(), "gap {gap}");
    }
}

#[test]
fn flagged_exactly_when_the_gap_is_under_the_clearance() {
    let c = DEFAULT_CLEARANCE_M;
    let (a, b) = two_boxes(c * (1.0 - 1e-3));
    assert!(!close_triangles(&a, &b, c).a.is_empty());
    let (a, b) = two_boxes(c * (1.0 + 1e-3));
    assert!(close_triangles(&a, &b, c).a.is_empty());
}

#[test]
fn the_clearance_is_a_parameter() {
    let (a, b) = two_boxes(0.001);
    assert!(close_triangles(&a, &b, 0.0005).a.is_empty());
    assert!(!close_triangles(&a, &b, 0.002).a.is_empty());
    assert!(!close_triangles(&a, &b, 0.002).b.is_empty());
}

#[test]
fn touching_and_overlapping_parts_are_flagged() {
    let c = DEFAULT_CLEARANCE_M;
    let (a, b) = two_boxes(0.0);
    assert!(!close_triangles(&a, &b, c).a.is_empty());
    let a = box_triangles(p(0., 0., 0.), p(SIDE, SIDE, SIDE));
    let b = box_triangles(
        p(SIDE / 2., SIDE / 2., SIDE / 2.),
        p(2. * SIDE, 2. * SIDE, 2. * SIDE),
    );
    let r = close_triangles(&a, &b, c);
    assert!(!r.a.is_empty());
    assert!(!r.b.is_empty());
}

#[test]
fn a_box_in_a_cavity_with_a_tight_fit() {
    // A hollow shell (outer and inner skins, inner facing in) around a core 0.1 mm smaller
    // on every side: the core's sides and the shell's inner skin are flagged, its outer
    // skin is not.
    let c = DEFAULT_CLEARANCE_M;
    let wall = 5.0 * MM;
    let shell_outer = box_triangles(p(0., 0., 0.), p(SIDE, SIDE, SIDE));
    let shell_inner = box_triangles(
        p(wall, wall, wall),
        p(SIDE - wall, SIDE - wall, SIDE - wall),
    );
    let slack = 0.5 * c;
    let core = box_triangles(
        p(wall + slack, wall + slack, wall + slack),
        p(
            SIDE - wall - slack,
            SIDE - wall - slack,
            SIDE - wall - slack,
        ),
    );
    let mut shell = shell_outer.clone();
    shell.extend(shell_inner.iter().copied());
    let r = close_triangles(&shell, &core, c);
    assert_eq!(r.b, (0..12).collect::<Vec<_>>());
    assert_eq!(r.a, (12..24).collect::<Vec<_>>());
}

#[test]
fn a_loose_fit_in_a_cavity_is_not_flagged() {
    let c = DEFAULT_CLEARANCE_M;
    let wall = 5.0 * MM;
    let mut shell = box_triangles(p(0., 0., 0.), p(SIDE, SIDE, SIDE));
    shell.extend(box_triangles(
        p(wall, wall, wall),
        p(SIDE - wall, SIDE - wall, SIDE - wall),
    ));
    let slack = 3.0 * c;
    let core = box_triangles(
        p(wall + slack, wall + slack, wall + slack),
        p(
            SIDE - wall - slack,
            SIDE - wall - slack,
            SIDE - wall - slack,
        ),
    );
    assert_eq!(close_triangles(&shell, &core, c), Default::default());
}

#[test]
fn the_lists_play_the_same_part_in_either_order() {
    let c = DEFAULT_CLEARANCE_M;
    let (a, b) = two_boxes(0.5 * c);
    let ab = close_triangles(&a, &b, c);
    let ba = close_triangles(&b, &a, c);
    assert_eq!(ab.a, ba.b);
    assert_eq!(ab.b, ba.a);
}

#[test]
fn indices_are_ascending_and_in_range() {
    let c = DEFAULT_CLEARANCE_M;
    let (a, b) = two_boxes(0.5 * c);
    let r = close_triangles(&a, &b, c);
    assert!(r.a.windows(2).all(|w| w[0] < w[1]));
    assert!(r.b.windows(2).all(|w| w[0] < w[1]));
    assert!(r.a.iter().all(|&i| i < a.len()));
    assert!(r.b.iter().all(|&i| i < b.len()));
}

#[test]
fn nothing_to_compare_flags_nothing() {
    let c = DEFAULT_CLEARANCE_M;
    let (a, _) = two_boxes(0.0);
    assert_eq!(close_triangles(&a, &[], c), Default::default());
    assert_eq!(close_triangles(&[], &a, c), Default::default());
    assert_eq!(close_triangles(&[], &[], c), Default::default());
}

#[test]
fn a_clearance_that_is_not_positive_flags_nothing() {
    let (a, b) = two_boxes(0.0);
    for c in [0.0, -1.0, f64::NAN] {
        assert_eq!(
            close_triangles(&a, &b, c),
            Default::default(),
            "clearance {c}"
        );
    }
}

#[test]
fn triangles_with_bad_numbers_are_never_flagged() {
    let c = DEFAULT_CLEARANCE_M;
    let (mut a, b) = two_boxes(0.0);
    a.push([p(f64::NAN, 0., 0.), p(0., 0., 0.), p(1., 0., 0.)]);
    a.push([p(f64::INFINITY, 0., 0.), p(SIDE, 0., 0.), p(SIDE, SIDE, 0.)]);
    let r = close_triangles(&a, &b, c);
    assert!(!r.a.contains(&12));
    assert!(!r.a.contains(&13));
    assert!(!r.a.is_empty());
}

fn brute(a: &[Triangle], b: &[Triangle], c: f64) -> (Vec<usize>, Vec<usize>) {
    let ia = (0..a.len())
        .filter(|&i| b.iter().any(|t| triangle_distance(&a[i], t) < c))
        .collect();
    let ib = (0..b.len())
        .filter(|&j| a.iter().any(|t| triangle_distance(t, &b[j]) < c))
        .collect();
    (ia, ib)
}

/// A square sheet of n x n cells (two triangles each) at height z, cells of `cell`.
fn sheet(n: usize, cell: f64, z: f64, bump: Option<((usize, usize), f64)>) -> Vec<Triangle> {
    let v = |i: usize, j: usize| {
        let zz = match bump {
            Some(((bi, bj), bz)) if (bi, bj) == (i, j) => bz,
            _ => z,
        };
        p(i as f64 * cell, j as f64 * cell, zz)
    };
    let mut out = Vec::with_capacity(2 * n * n);
    for i in 0..n {
        for j in 0..n {
            out.push([v(i, j), v(i + 1, j), v(i + 1, j + 1)]);
            out.push([v(i, j), v(i + 1, j + 1), v(i, j + 1)]);
        }
    }
    out
}

#[test]
fn a_bumpy_pair_of_sheets_matches_the_brute_force() {
    let c = 0.0002;
    let a = sheet(12, 0.001, 0.0, None);
    let b = sheet(12, 0.0013, 0.0008, Some(((5, 5), 0.0001)));
    let (ia, ib) = brute(&a, &b, c);
    assert!(!ia.is_empty() && ia.len() < a.len());
    let r = close_triangles(&a, &b, c);
    assert_eq!((r.a, r.b), (ia, ib));
}

#[test]
fn big_parts_are_not_compared_pair_by_pair() {
    // 2 x 45 000 triangles, 1 mm apart except one corner that dips to 0.1 mm. Comparing every
    // pair would take billions of distance calls.
    let c = 0.0002;
    let n = 150;
    let a = sheet(n, 0.0005, 0.0, None);
    let b = sheet(n, 0.0005, 0.001, Some(((75, 75), 0.0001)));
    assert_eq!(a.len(), 45_000);
    let start = Instant::now();
    let r = close_triangles(&a, &b, c);
    let took = start.elapsed();
    assert!(took.as_secs_f64() < 10.0, "took {took:?}");
    // The six triangles that meet at the dipping corner are flagged, and little else.
    let corner = p(75.0 * 0.0005, 75.0 * 0.0005, 0.0001);
    let touching: Vec<usize> = (0..b.len())
        .filter(|&j| b[j].iter().any(|q| q.distance(corner) < 1e-12))
        .collect();
    assert_eq!(touching.len(), 6);
    for j in &touching {
        assert!(r.b.contains(j));
    }
    assert!(r.b.len() < 100, "{} flagged in B", r.b.len());
    assert!(
        !r.a.is_empty() && r.a.len() < 100,
        "{} flagged in A",
        r.a.len()
    );
}

#[test]
fn big_parts_far_apart_flag_nothing() {
    let n = 150;
    let a = sheet(n, 0.0005, 0.0, None);
    let b = sheet(n, 0.0005, 0.001, None);
    let start = Instant::now();
    let r = close_triangles(&a, &b, 0.0002);
    assert!(start.elapsed().as_secs_f64() < 10.0);
    assert_eq!(r, Default::default());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn close_triangles_agrees_with_triangle_distance(
        a in prop::collection::vec(prop::array::uniform3(prop::array::uniform3(0.0f64..0.01)), 0..12),
        b in prop::collection::vec(prop::array::uniform3(prop::array::uniform3(0.0f64..0.01)), 0..12),
        c in 0.0005f64..0.004,
    ) {
        let a: Vec<Triangle> = a.iter().map(|t| tri(t[0], t[1], t[2])).collect();
        let b: Vec<Triangle> = b.iter().map(|t| tri(t[0], t[1], t[2])).collect();
        let (ia, ib) = brute(&a, &b, c);
        let r = close_triangles(&a, &b, c);
        prop_assert_eq!(r.a, ia);
        prop_assert_eq!(r.b, ib);
    }

    #[test]
    fn separated_boxes_are_flagged_exactly_when_the_gap_is_under_the_clearance(
        gap in 0.00001f64..0.0006,
        c in 0.0001f64..0.0006,
        axis in 0usize..3,
    ) {
        prop_assume!((gap - c).abs() > 1e-6);
        let mut lo = [0.0; 3];
        let mut hi = [SIDE; 3];
        let a = box_triangles(p(0., 0., 0.), p(SIDE, SIDE, SIDE));
        lo[axis] = SIDE + gap;
        hi[axis] = 2.0 * SIDE + gap;
        let b = box_triangles(p(lo[0], lo[1], lo[2]), p(hi[0], hi[1], hi[2]));
        let r = close_triangles(&a, &b, c);
        prop_assert_eq!(!r.a.is_empty(), gap < c);
        prop_assert_eq!(!r.b.is_empty(), gap < c);
    }
}

// ---------------------------------------------------------------------------------------
// instance_triangles

struct World {
    m: km::Model,
    log: ModelEdit,
}

fn new_loop(ctx: &mut Context, log: &mut Edit, pts: &[DVec3]) -> Loop {
    let vs: Vec<VertexId> = pts.iter().map(|&q| ctx.insert_vertex(q, log)).collect();
    let uses = (0..vs.len())
        .map(|i| {
            let e = ctx
                .insert_edge(vs[i], vs[(i + 1) % vs.len()], log)
                .expect("edge inserts");
            EdgeUse::forward(e)
        })
        .collect();
    Loop { uses }
}

impl World {
    fn new() -> World {
        World {
            m: km::Model::new(),
            log: ModelEdit::new(),
        }
    }

    /// A definition holding a box from the origin to `size`, as six square faces.
    fn box_definition(&mut self, name: &str, size: DVec3) -> DefinitionId {
        let def = self.m.add_definition(
            DefinitionInfo {
                name: name.into(),
                group: true,
            },
            &mut self.log,
        );
        let c = |i: usize| {
            p(
                if i & 1 == 0 { 0.0 } else { size.x },
                if i & 2 == 0 { 0.0 } else { size.y },
                if i & 4 == 0 { 0.0 } else { size.z },
            )
        };
        let sides: [[usize; 4]; 6] = [
            [0, 2, 3, 1],
            [4, 5, 7, 6],
            [0, 1, 5, 4],
            [1, 3, 7, 5],
            [3, 2, 6, 7],
            [2, 0, 4, 6],
        ];
        self.m
            .edit_geometry(BodyId::Definition(def), &mut self.log, |ctx, log| {
                for s in sides {
                    let pts: Vec<DVec3> = s.iter().map(|&i| c(i)).collect();
                    let outer = new_loop(ctx, log, &pts);
                    ctx.insert_face(
                        Face {
                            outer,
                            inner: vec![],
                        },
                        log,
                    )
                    .expect("face inserts");
                }
            })
            .expect("body exists");
        def
    }

    fn place(&mut self, body: BodyId, def: DefinitionId, transform: DMat4) -> InstanceId {
        self.m
            .add_instance(
                body,
                Instance {
                    definition: def,
                    transform,
                    name: String::new(),
                    material: None,
                },
                &mut self.log,
            )
            .expect("instance adds")
    }
}

fn bounds_of(list: &[Triangle]) -> (DVec3, DVec3) {
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    for q in list.iter().flatten() {
        lo = lo.min(*q);
        hi = hi.max(*q);
    }
    (lo, hi)
}

fn close(a: DVec3, b: DVec3) -> bool {
    a.distance(b) < 1e-9
}

#[test]
fn an_instance_gives_the_triangles_of_its_faces_in_place() {
    let mut w = World::new();
    let def = w.box_definition("box", p(0.02, 0.03, 0.04));
    let id = w.place(BodyId::Root, def, DMat4::from_translation(p(0.1, 0.2, 0.3)));
    let t = instance_triangles(&w.m, BodyId::Root, id, DMat4::IDENTITY);
    assert_eq!(t.len(), 12);
    let (lo, hi) = bounds_of(&t);
    assert!(close(lo, p(0.1, 0.2, 0.3)));
    assert!(close(hi, p(0.12, 0.23, 0.34)));
}

#[test]
fn the_outer_matrix_is_applied_after_the_instance_transform() {
    let mut w = World::new();
    let def = w.box_definition("box", p(0.02, 0.03, 0.04));
    let id = w.place(BodyId::Root, def, DMat4::from_translation(p(0.1, 0.0, 0.0)));
    let outer = DMat4::from_rotation_z(std::f64::consts::FRAC_PI_2);
    let t = instance_triangles(&w.m, BodyId::Root, id, outer);
    let (lo, hi) = bounds_of(&t);
    // x -> y, y -> -x: the box spans x in [-0.03, 0], y in [0.1, 0.12].
    assert!(close(lo, p(-0.03, 0.1, 0.0)));
    assert!(close(hi, p(0.0, 0.12, 0.04)));
}

#[test]
fn only_the_asked_instance_is_returned() {
    let mut w = World::new();
    let def = w.box_definition("box", p(0.01, 0.01, 0.01));
    let a = w.place(BodyId::Root, def, DMat4::IDENTITY);
    let b = w.place(BodyId::Root, def, DMat4::from_translation(p(1.0, 0.0, 0.0)));
    let ta = instance_triangles(&w.m, BodyId::Root, a, DMat4::IDENTITY);
    let tb = instance_triangles(&w.m, BodyId::Root, b, DMat4::IDENTITY);
    assert_eq!(ta.len(), 12);
    assert_eq!(tb.len(), 12);
    assert!(bounds_of(&ta).1.x < 0.5);
    assert!(bounds_of(&tb).0.x > 0.5);
}

#[test]
fn nested_instances_are_included() {
    let mut w = World::new();
    let inner = w.box_definition("inner", p(0.01, 0.01, 0.01));
    let outer_def = w.box_definition("outer", p(0.05, 0.05, 0.05));
    w.place(
        BodyId::Definition(outer_def),
        inner,
        DMat4::from_translation(p(0.02, 0.02, 0.02)),
    );
    let id = w.place(
        BodyId::Root,
        outer_def,
        DMat4::from_translation(p(1.0, 0.0, 0.0)),
    );
    let t = instance_triangles(&w.m, BodyId::Root, id, DMat4::IDENTITY);
    assert_eq!(t.len(), 24);
    // The inner box sits inside the outer one at (1.02, 0.02, 0.02).
    assert!(t.iter().flatten().any(|q| close(*q, p(1.02, 0.02, 0.02))));
}

#[test]
fn a_missing_instance_gives_nothing() {
    let mut w = World::new();
    let def = w.box_definition("box", p(0.01, 0.01, 0.01));
    let id = w.place(BodyId::Root, def, DMat4::IDENTITY);
    assert!(
        instance_triangles(&w.m, BodyId::Root, InstanceId(id.0 + 99), DMat4::IDENTITY).is_empty()
    );
    assert!(instance_triangles(&w.m, BodyId::Definition(def), id, DMat4::IDENTITY).is_empty());
}

#[test]
fn two_placed_parts_end_to_end() {
    // Two 10 mm cubes 0.1 mm apart: the facing sides are flagged at the default clearance,
    // and tinting the scene with them leaves the other sides untinted.
    let mut w = World::new();
    let def = w.box_definition("cube", p(0.01, 0.01, 0.01));
    let a = w.place(BodyId::Root, def, DMat4::IDENTITY);
    let b = w.place(
        BodyId::Root,
        def,
        DMat4::from_translation(p(0.0101, 0.0, 0.0)),
    );
    let ta = instance_triangles(&w.m, BodyId::Root, a, DMat4::IDENTITY);
    let tb = instance_triangles(&w.m, BodyId::Root, b, DMat4::IDENTITY);
    let r = close_triangles(&ta, &tb, DEFAULT_CLEARANCE_M);
    assert!(r.a.len() >= 2 && r.b.len() >= 2);
    let mut flagged: Vec<Triangle> = r.a.iter().map(|&i| ta[i]).collect();
    flagged.extend(r.b.iter().map(|&i| tb[i]));

    let scene = kernel_scene(&w.m);
    let total = scene.triangle_count();
    assert_eq!(total, 24);
    let tinted = tint_fit(&scene, &flagged);
    assert_eq!(tinted.triangle_count(), total);
    let last = tinted.batches.last().unwrap();
    assert_eq!(last.indices.len() / 3, flagged.len());
    assert!(last.indices.len() / 3 < total);
}

#[test]
fn every_triangle_of_an_instance_is_found_in_the_scene() {
    let mut w = World::new();
    let def = w.box_definition("box", p(0.02, 0.03, 0.04));
    let id = w.place(
        BodyId::Root,
        def,
        DMat4::from_rotation_z(0.3) * DMat4::from_translation(p(0.1, 0.2, 0.3)),
    );
    let t = instance_triangles(&w.m, BodyId::Root, id, DMat4::IDENTITY);
    let tinted = tint_fit(&kernel_scene(&w.m), &t);
    assert_eq!(
        tinted.batches.len(),
        1,
        "everything moved into the fit batch"
    );
    assert_eq!(tinted.batches[0].indices.len(), 36);
}

// ---------------------------------------------------------------------------------------
// tint_fit

fn material(id: &str) -> Material {
    Material {
        id: id.into(),
        name: id.into(),
        color: [0.5, 0.5, 0.5, 1.0],
        texture: None,
        opacity: 1.0,
        unlit: false,
    }
}

fn f(q: DVec3) -> [f32; 3] {
    [q.x as f32, q.y as f32, q.z as f32]
}

/// A batch of the given triangles, three vertices each, normal +Z, texcoords set.
fn batch_of(list: &[Triangle], mat: Option<usize>) -> Batch {
    let mut b = Batch {
        material: mat,
        ..Batch::default()
    };
    for (i, t) in list.iter().enumerate() {
        for q in t {
            b.positions.push(f(*q));
            b.normals.push([0.0, 0.0, 1.0]);
            b.texcoords.push([q.x as f32, q.y as f32]);
        }
        let base = 3 * i as u32;
        b.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }
    b
}

fn same_batch(a: &Batch, b: &Batch) -> bool {
    a.material == b.material
        && a.positions == b.positions
        && a.normals == b.normals
        && a.texcoords == b.texcoords
        && a.indices == b.indices
}

fn assert_same_scene(a: &FlatScene, b: &FlatScene) {
    assert_eq!(a.materials, b.materials, "materials");
    assert_eq!(a.batches.len(), b.batches.len(), "batch count");
    for (x, y) in a.batches.iter().zip(&b.batches) {
        assert!(same_batch(x, y), "batch {x:?} vs {y:?}");
    }
    assert_eq!(a.lines.len(), b.lines.len());
    for (x, y) in a.lines.iter().zip(&b.lines) {
        assert_eq!(x.material, y.material);
        assert_eq!(x.positions, y.positions);
    }
    assert_eq!(a.bounds, b.bounds);
}

fn scene_of(batches: Vec<Batch>) -> FlatScene {
    FlatScene {
        materials: vec![material("m0"), material("m1")],
        batches,
        lines: vec![LineBatch {
            material: None,
            positions: vec![[0.0; 3], [1.0, 0.0, 0.0]],
        }],
        bounds: Some(Aabb {
            min: [0.0; 3],
            max: [1.0; 3],
        }),
    }
}

fn t0() -> Triangle {
    tri([0., 0., 0.], [0.1, 0., 0.], [0., 0.1, 0.])
}
fn t1() -> Triangle {
    tri([0.2, 0., 0.], [0.3, 0., 0.], [0.2, 0.1, 0.])
}
fn t2() -> Triangle {
    tri([0.4, 0., 0.], [0.5, 0., 0.], [0.4, 0.1, 0.])
}

#[test]
fn no_match_gives_an_identical_scene() {
    let scene = scene_of(vec![batch_of(&[t0(), t1()], Some(0))]);
    assert_same_scene(&tint_fit(&scene, &[]), &scene);
    assert_same_scene(&tint_fit(&scene, &[t2()]), &scene);
}

#[test]
fn a_matching_triangle_moves_to_a_new_last_batch() {
    let scene = scene_of(vec![
        batch_of(&[t0(), t1()], Some(0)),
        batch_of(&[t2()], Some(1)),
    ]);
    let out = tint_fit(&scene, &[t1()]);
    assert_eq!(out.materials.len(), 3);
    assert_eq!(out.materials[..2], scene.materials[..]);
    assert_eq!(
        out.materials[2],
        Material {
            id: "fit".into(),
            name: "Fit".into(),
            color: FIT_COLOR,
            texture: None,
            opacity: 1.0,
            unlit: false,
        }
    );
    assert_eq!(out.batches.len(), 3);
    // The first batch keeps its vertices and loses the triangle.
    assert_eq!(out.batches[0].material, Some(0));
    assert_eq!(out.batches[0].positions, scene.batches[0].positions);
    assert_eq!(out.batches[0].normals, scene.batches[0].normals);
    assert_eq!(out.batches[0].texcoords, scene.batches[0].texcoords);
    assert_eq!(out.batches[0].indices, vec![0, 1, 2]);
    assert!(same_batch(&out.batches[1], &scene.batches[1]));
    let fit = &out.batches[2];
    assert_eq!(fit.material, Some(2));
    assert_eq!(fit.positions, scene.batches[0].positions[3..6].to_vec());
    assert_eq!(fit.normals, vec![[0.0, 0.0, 1.0]; 3]);
    assert!(fit.texcoords.is_empty());
    assert_eq!(fit.indices, vec![0, 1, 2]);
    assert_eq!(out.lines.len(), 1);
    assert_eq!(out.lines[0].positions, scene.lines[0].positions);
    assert_eq!(out.bounds, scene.bounds);
}

#[test]
fn tinted_triangles_keep_walk_order_and_emptied_batches_are_dropped() {
    let scene = scene_of(vec![
        batch_of(&[t0()], Some(0)),
        batch_of(&[t1(), t2()], Some(1)),
    ]);
    // Flag in the opposite order: the batch still follows the scene's order.
    let out = tint_fit(&scene, &[t2(), t0()]);
    assert_eq!(
        out.batches.len(),
        2,
        "the first batch had only a tinted triangle"
    );
    assert_eq!(out.batches[0].material, Some(1));
    assert_eq!(out.batches[0].indices, vec![0, 1, 2]);
    let fit = out.batches.last().unwrap();
    assert_eq!(fit.indices, vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(fit.positions[0], f(t0()[0]));
    assert_eq!(fit.positions[3], f(t2()[0]));
}

#[test]
fn corners_may_come_in_any_order_and_either_winding() {
    let scene = scene_of(vec![batch_of(&[t0(), t1()], Some(0))]);
    let t = t1();
    for flagged in [
        [t[1], t[2], t[0]],
        [t[2], t[0], t[1]],
        [t[0], t[2], t[1]],
        [t[2], t[1], t[0]],
    ] {
        let out = tint_fit(&scene, &[flagged]);
        assert_eq!(out.batches.last().unwrap().indices.len(), 3, "{flagged:?}");
    }
}

#[test]
fn matching_has_a_small_tolerance() {
    let scene = scene_of(vec![batch_of(&[t0(), t1()], Some(0))]);
    let nudge = |t: Triangle, d: f64| t.map(|q| q + p(d, -d, d));
    // About 8.7e-6 m of drift on every corner still matches; 8.7e-5 does not.
    assert_eq!(tint_fit(&scene, &[nudge(t1(), 5e-6)]).batches.len(), 2);
    assert_same_scene(&tint_fit(&scene, &[nudge(t1(), 5e-5)]), &scene);
}

#[test]
fn all_three_corners_must_match() {
    let scene = scene_of(vec![batch_of(&[t0()], Some(0))]);
    let t = t0();
    let two = [t[0], t[1], p(0.0, 0.5, 0.0)];
    assert_same_scene(&tint_fit(&scene, &[two]), &scene);
}

#[test]
fn flagging_a_triangle_twice_tints_it_once() {
    let scene = scene_of(vec![batch_of(&[t0(), t1()], Some(0))]);
    let out = tint_fit(&scene, &[t0(), t0()]);
    assert_eq!(out.batches.last().unwrap().indices.len(), 3);
}

#[test]
fn identical_triangles_in_the_scene_are_all_tinted() {
    let scene = scene_of(vec![batch_of(&[t0(), t0()], Some(0))]);
    let out = tint_fit(&scene, &[t0()]);
    assert_eq!(out.batches.len(), 1);
    assert_eq!(out.batches[0].indices.len(), 6);
}

#[test]
fn the_source_scene_is_not_changed() {
    let scene = scene_of(vec![batch_of(&[t0(), t1()], Some(0))]);
    let before = scene.clone();
    let _ = tint_fit(&scene, &[t0()]);
    assert_same_scene(&scene, &before);
}

#[test]
fn many_flagged_triangles_on_a_big_scene_are_fast() {
    let sheet_a = sheet(150, 0.0005, 0.0, None);
    let scene = scene_of(vec![batch_of(&sheet_a, Some(0))]);
    let flagged: Vec<Triangle> = sheet_a.iter().step_by(2).copied().collect();
    let start = Instant::now();
    let out = tint_fit(&scene, &flagged);
    assert!(start.elapsed().as_secs_f64() < 10.0);
    assert_eq!(out.batches.last().unwrap().indices.len(), 3 * 22_500);
    assert_eq!(out.batches[0].indices.len(), 3 * 22_500);
}
