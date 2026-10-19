// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use std::collections::BTreeSet;

use glam::{DQuat, DVec3};
use modeler_kernel::topology::*;
use proptest::prelude::*;

/// One build step: a regular polygon (optionally with a hole) in its own cell, or a delete.
#[derive(Clone, Debug)]
enum Step {
    Polygon {
        sides: usize,
        radius: f64,
        rot: (f64, f64, f64),
        hole: bool,
    },
    Delete(usize),
    Move(usize, f64),
}

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        3 => (3usize..9, 0.5f64..2.0, (-3.0f64..3.0, -3.0f64..3.0, -3.0f64..3.0), any::<bool>())
            .prop_map(|(sides, radius, rot, hole)| Step::Polygon { sides, radius, rot, hole }),
        1 => any::<usize>().prop_map(Step::Delete),
        1 => (any::<usize>(), -0.2f64..0.2).prop_map(|(i, dz)| Step::Move(i, dz)),
    ]
}

fn ring(center: DVec3, q: DQuat, radius: f64, sides: usize) -> Vec<DVec3> {
    (0..sides)
        .map(|i| {
            let t = i as f64 / sides as f64 * std::f64::consts::TAU;
            center + q * DVec3::new(radius * t.cos(), radius * t.sin(), 0.0)
        })
        .collect()
}

fn chain(ctx: &mut Context, pts: &[DVec3], log: &mut Edit) -> Vec<EdgeId> {
    let vs: Vec<_> = pts.iter().map(|p| ctx.insert_vertex(*p, log)).collect();
    (0..vs.len())
        .map(|i| ctx.insert_edge(vs[i], vs[(i + 1) % vs.len()], log).unwrap())
        .collect()
}

/// Delete a face, then its edges, then its vertices, keeping the context valid.
fn delete_face_and_boundary(ctx: &mut Context, f: FaceId, log: &mut Edit) {
    let face = ctx.face(f).unwrap().clone();
    ctx.delete_face(f, log).unwrap();
    let mut vs = BTreeSet::new();
    for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
        for u in &l.uses {
            let e = *ctx.edge(u.edge).unwrap();
            vs.extend(e.ends);
            ctx.delete_edge(u.edge, log).unwrap();
        }
    }
    for v in vs {
        ctx.delete_vertex(v, log).unwrap();
    }
}

/// Every face lists in `faces_of_edge` of its edges, every edge in `edges_of_vertex` of its ends,
/// and nothing else is listed.
fn adjacency_agrees(ctx: &Context) -> Result<(), TestCaseError> {
    for (e, edge) in ctx.edges() {
        for v in edge.ends {
            prop_assert!(ctx.edges_of_vertex(v).contains(&e));
        }
        for f in ctx.faces_of_edge(e) {
            let face = ctx.face(f).unwrap();
            let mut uses = std::iter::once(&face.outer)
                .chain(face.inner.iter())
                .flat_map(|l| l.uses.iter());
            prop_assert!(uses.any(|u| u.edge == e));
        }
    }
    for (v, _) in ctx.vertices() {
        for e in ctx.edges_of_vertex(v) {
            prop_assert!(ctx.edge(e).unwrap().ends.contains(&v));
        }
    }
    for (f, face) in ctx.faces() {
        for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
            for u in &l.uses {
                prop_assert!(ctx.faces_of_edge(u.edge).contains(&f));
            }
        }
    }
    Ok(())
}

/// The result of running steps: one edit per step, the snapshot after each step, and every
/// id handed out as (kind, id).
struct Built {
    ctx: Context,
    edits: Vec<Edit>,
    snaps: Vec<Context>,
    ids: BTreeSet<(u8, u64)>,
}

fn build(steps: &[Step]) -> Built {
    let mut ctx = Context::new();
    let mut edits = vec![];
    let mut snaps = vec![];
    let mut ids = BTreeSet::new();
    for (k, s) in steps.iter().enumerate() {
        let mut log = Edit::new();
        match s {
            Step::Polygon {
                sides,
                radius,
                rot,
                hole,
            } => {
                let center = DVec3::new(10.0 * k as f64, 0.0, 0.0);
                let q = DQuat::from_euler(glam::EulerRot::XYZ, rot.0, rot.1, rot.2);
                let outer = chain(&mut ctx, &ring(center, q, *radius, *sides), &mut log);
                let mut inner = vec![];
                if *hole {
                    let es = chain(&mut ctx, &ring(center, q, radius * 0.4, *sides), &mut log);
                    inner.push(Loop {
                        uses: es.iter().rev().map(|e| EdgeUse::backward(*e)).collect(),
                    });
                }
                let face = Face {
                    outer: Loop {
                        uses: outer.iter().map(|e| EdgeUse::forward(*e)).collect(),
                    },
                    inner,
                };
                ctx.insert_face(face, &mut log).unwrap();
            }
            Step::Delete(i) => {
                let faces: Vec<_> = ctx.faces().map(|(f, _)| f).collect();
                if !faces.is_empty() {
                    delete_face_and_boundary(&mut ctx, faces[i % faces.len()], &mut log);
                }
            }
            Step::Move(i, dz) => {
                // Move a whole face's vertices along its normal (stays valid: cells are apart).
                let faces: Vec<_> = ctx.faces().map(|(f, _)| f).collect();
                if !faces.is_empty() {
                    let f = faces[i % faces.len()];
                    let n = ctx.face_plane(f).unwrap().normal;
                    let face = ctx.face(f).unwrap().clone();
                    let mut vs = BTreeSet::new();
                    for l in std::iter::once(&face.outer).chain(face.inner.iter()) {
                        vs.extend(ctx.loop_vertices(l));
                    }
                    for v in vs {
                        let p = ctx.vertex(v).unwrap().position;
                        ctx.set_vertex_position(v, p + n * *dz, &mut log).unwrap();
                    }
                }
            }
        }
        for r in &log.records {
            match r {
                Record::AddVertex(id, _) => assert!(ids.insert((0, id.0)), "vertex id reused"),
                Record::AddEdge(id, _) => assert!(ids.insert((1, id.0)), "edge id reused"),
                Record::AddFace(id, _) => assert!(ids.insert((2, id.0)), "face id reused"),
                _ => {}
            }
        }
        edits.push(log);
        snaps.push(ctx.clone());
    }
    Built {
        ctx,
        edits,
        snaps,
        ids,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn random_builds_stay_valid_and_undo_exactly(steps in prop::collection::vec(step(), 1..12)) {
        let Built { mut ctx, edits, snaps, .. } = build(&steps);
        prop_assert_eq!(ctx.validate(), vec![]);
        adjacency_agrees(&ctx)?;
        let last = ctx.clone();

        // Undo step by step back to empty.
        for k in (0..edits.len()).rev() {
            prop_assert_eq!(&ctx, &snaps[k]);
            ctx.apply_edit(&edits[k].inverse()).unwrap();
            prop_assert_eq!(ctx.validate(), vec![]);
            adjacency_agrees(&ctx)?;
        }
        prop_assert_eq!(&ctx, &Context::new());
        prop_assert_eq!(ctx.vertex_count() + ctx.edge_count() + ctx.face_count(), 0);

        // Redo everything in one edit.
        let all = Edit { records: edits.iter().flat_map(|e| e.records.clone()).collect() };
        ctx.apply_edit(&all).unwrap();
        prop_assert_eq!(&ctx, &last);
        adjacency_agrees(&ctx)?;
    }

    #[test]
    fn ids_are_never_reused_after_undo(steps in prop::collection::vec(step(), 1..8)) {
        let Built { mut ctx, edits, ids, .. } = build(&steps);
        for e in edits.iter().rev() {
            ctx.apply_edit(&e.inverse()).unwrap();
        }
        let mut log = Edit::new();
        let pts = [DVec3::ZERO, DVec3::X, DVec3::Y];
        let es = chain(&mut ctx, &pts, &mut log);
        let f = ctx.insert_face(Face {
            outer: Loop { uses: es.iter().map(|e| EdgeUse::forward(*e)).collect() },
            inner: vec![],
        }, &mut log).unwrap();
        for (v, _) in ctx.vertices() {
            prop_assert!(!ids.contains(&(0, v.0)));
        }
        for e in es {
            prop_assert!(!ids.contains(&(1, e.0)));
        }
        prop_assert!(!ids.contains(&(2, f.0)));
    }

    #[test]
    fn a_bad_record_leaves_the_context_unchanged(
        steps in prop::collection::vec(step(), 1..8),
        bad in 0usize..5,
        pick in any::<usize>(),
    ) {
        let mut ctx = build(&steps).ctx;
        let before = ctx.clone();
        let adjacency_before: Vec<_> = ctx.edges().map(|(e, _)| ctx.faces_of_edge(e)).collect();
        let far = VertexId(1 << 40);
        // Some good records first.
        let mut records = vec![
            Record::AddVertex(far, Vertex { position: DVec3::new(-50.0, 0.0, 0.0) }),
            Record::AddVertex(VertexId(far.0 + 1), Vertex { position: DVec3::new(-51.0, 0.0, 0.0) }),
            Record::AddEdge(EdgeId(1 << 40), Edge { ends: [far, VertexId(far.0 + 1)] }),
        ];
        let vs: Vec<_> = ctx.vertices().map(|(v, d)| (v, *d)).collect();
        let es: Vec<_> = ctx.edges().map(|(e, d)| (e, *d)).collect();
        let fs: Vec<_> = ctx.faces().map(|(f, d)| (f, d.clone())).collect();
        if let Some((f, face)) = fs.get(pick % fs.len().max(1)) {
            records.push(Record::RemoveFace(*f, face.clone()));
        }
        if let Some((v, d)) = vs.get(pick % vs.len().max(1)) {
            records.push(Record::MoveVertex { id: *v, from: d.position, to: d.position + DVec3::Z });
        }
        records.push(match bad {
            0 => Record::AddVertex(far, Vertex { position: DVec3::ZERO }),
            1 => Record::AddEdge(EdgeId(7 << 40), Edge { ends: [far, VertexId(9 << 40)] }),
            2 => Record::RemoveVertex(far, Vertex { position: DVec3::ZERO }),
            3 => Record::RemoveVertex(far, Vertex { position: DVec3::new(-50.0, 0.0, 0.0) }),
            _ => match es.first() {
                Some((e, d)) => Record::RemoveEdge(*e, Edge { ends: [d.ends[1], d.ends[0]] }),
                None => Record::RemoveFace(FaceId(5 << 40), Face { outer: Loop::default(), inner: vec![] }),
            },
        });
        let edit = Edit { records };
        prop_assert!(ctx.apply_edit(&edit).is_err());
        prop_assert_eq!(&ctx, &before);
        let adjacency_after: Vec<_> = ctx.edges().map(|(e, _)| ctx.faces_of_edge(e)).collect();
        prop_assert_eq!(adjacency_after, adjacency_before);
        prop_assert_eq!(ctx.edges_of_vertex(far), vec![]);
        prop_assert_eq!(ctx.validate(), vec![]);
    }

    #[test]
    fn grids_of_boxes_are_valid(
        boxes in prop::collection::vec(
            ((0.1f64..3.0, 0.1f64..3.0, 0.1f64..3.0), (-3.0f64..3.0, -3.0f64..3.0, -3.0f64..3.0)),
            1..6,
        ),
    ) {
        let mut ctx = Context::new();
        let mut log = Edit::new();
        for (k, (size, rot)) in boxes.iter().enumerate() {
            let q = DQuat::from_euler(glam::EulerRot::XYZ, rot.0, rot.1, rot.2);
            let origin = DVec3::new(0.0, 12.0 * k as f64, -3.0);
            let corner = |i: usize| {
                let x = if i & 1 != 0 { size.0 } else { 0.0 };
                let y = if i & 2 != 0 { size.1 } else { 0.0 };
                let z = if i & 4 != 0 { size.2 } else { 0.0 };
                origin + q * DVec3::new(x, y, z)
            };
            let vs: Vec<_> = (0..8).map(|i| ctx.insert_vertex(corner(i), &mut log)).collect();
            for (a, b) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
                ctx.insert_edge(vs[a], vs[b], &mut log).unwrap();
            }
            // Corner quads, counter-clockwise seen from outside.
            for quad in [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]] {
                let uses = (0..4)
                    .map(|i| {
                        let (a, b) = (vs[quad[i]], vs[quad[(i + 1) % 4]]);
                        let e = ctx.edge_between(a, b).unwrap();
                        EdgeUse { edge: e, reversed: ctx.edge(e).unwrap().ends[0] != a }
                    })
                    .collect();
                let f = ctx.insert_face(Face { outer: Loop { uses }, inner: vec![] }, &mut log).unwrap();
                let n = ctx.face_plane(f).unwrap().normal;
                let centre = origin + q * DVec3::new(size.0, size.1, size.2) * 0.5;
                prop_assert!(n.dot(ctx.face_plane(f).unwrap().point - centre) > 0.0, "face points inward");
            }
        }
        prop_assert_eq!(ctx.validate(), vec![]);
        adjacency_agrees(&ctx)?;
        for (e, _) in ctx.edges() {
            prop_assert_eq!(ctx.faces_of_edge(e).len(), 2);
        }
        for (v, _) in ctx.vertices() {
            prop_assert_eq!(ctx.edges_of_vertex(v).len(), 3);
        }
    }
}
