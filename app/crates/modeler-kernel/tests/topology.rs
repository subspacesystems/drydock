// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use glam::DVec3;
use modeler_kernel::topology::*;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// Vertices, edges and a face for a closed polygon (new vertices, no sharing).
/// Returns the face and its edges in loop order.
fn polygon(ctx: &mut Context, pts: &[DVec3], log: &mut Edit) -> (FaceId, Vec<EdgeId>) {
    let vs: Vec<_> = pts.iter().map(|p| ctx.insert_vertex(*p, log)).collect();
    let es = chain(ctx, &vs, log);
    let face = Face {
        outer: forward_loop(&es),
        inner: vec![],
    };
    (ctx.insert_face(face, log).unwrap(), es)
}

/// Edges vs[0]-vs[1], ..., vs[n-1]-vs[0].
fn chain(ctx: &mut Context, vs: &[VertexId], log: &mut Edit) -> Vec<EdgeId> {
    (0..vs.len())
        .map(|i| ctx.insert_edge(vs[i], vs[(i + 1) % vs.len()], log).unwrap())
        .collect()
}

fn forward_loop(es: &[EdgeId]) -> Loop {
    Loop {
        uses: es.iter().map(|e| EdgeUse::forward(*e)).collect(),
    }
}

fn backward_loop(es: &[EdgeId]) -> Loop {
    Loop {
        uses: es.iter().rev().map(|e| EdgeUse::backward(*e)).collect(),
    }
}

fn square(x: f64, y: f64, s: f64) -> Vec<DVec3> {
    vec![
        v(x, y, 0.),
        v(x + s, y, 0.),
        v(x + s, y + s, 0.),
        v(x, y + s, 0.),
    ]
}

fn verts(ctx: &mut Context, pts: &[DVec3], log: &mut Edit) -> Vec<VertexId> {
    pts.iter().map(|p| ctx.insert_vertex(*p, log)).collect()
}

// -- ids, primitives, records ---------------------------------------------------------

#[test]
fn ids_count_from_zero_per_kind() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let a = ctx.insert_vertex(v(0., 0., 0.), &mut log);
    let b = ctx.insert_vertex(v(1., 0., 0.), &mut log);
    let c = ctx.insert_vertex(v(0., 1., 0.), &mut log);
    assert_eq!((a, b, c), (VertexId(0), VertexId(1), VertexId(2)));
    let e0 = ctx.insert_edge(a, b, &mut log).unwrap();
    let e1 = ctx.insert_edge(b, c, &mut log).unwrap();
    assert_eq!((e0, e1), (EdgeId(0), EdgeId(1)));
    let e2 = ctx.insert_edge(c, a, &mut log).unwrap();
    let f = ctx
        .insert_face(
            Face {
                outer: forward_loop(&[e0, e1, e2]),
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();
    assert_eq!(f, FaceId(0));
    assert_eq!(
        (ctx.vertex_count(), ctx.edge_count(), ctx.face_count()),
        (3, 3, 1)
    );
}

#[test]
fn primitives_log_their_records() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let a = ctx.insert_vertex(v(0., 0., 0.), &mut log);
    let b = ctx.insert_vertex(v(1., 0., 0.), &mut log);
    let e = ctx.insert_edge(b, a, &mut log).unwrap();
    ctx.set_vertex_position(a, v(0., 2., 0.), &mut log).unwrap();
    assert_eq!(
        log.records,
        vec![
            Record::AddVertex(
                a,
                Vertex {
                    position: v(0., 0., 0.)
                }
            ),
            Record::AddVertex(
                b,
                Vertex {
                    position: v(1., 0., 0.)
                }
            ),
            Record::AddEdge(e, Edge { ends: [b, a] }),
            Record::MoveVertex {
                id: a,
                from: v(0., 0., 0.),
                to: v(0., 2., 0.)
            },
        ]
    );
    assert_eq!(ctx.edge(e).unwrap().ends, [b, a]);
    assert_eq!(ctx.vertex(a).unwrap().position, v(0., 2., 0.));
}

#[test]
fn insert_edge_errors_leave_no_trace() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let a = ctx.insert_vertex(v(0., 0., 0.), &mut log);
    let before = ctx.clone();
    let n = log.records.len();
    assert_eq!(
        ctx.insert_edge(a, VertexId(7), &mut log),
        Err(TopoError::MissingVertex(VertexId(7)))
    );
    assert_eq!(
        ctx.insert_edge(VertexId(9), VertexId(7), &mut log),
        Err(TopoError::MissingVertex(VertexId(9)))
    );
    assert_eq!(
        ctx.insert_edge(a, a, &mut log),
        Err(TopoError::SameVertex(a))
    );
    assert_eq!(ctx, before);
    assert_eq!(log.records.len(), n);
    // A failed insert doesn't use up an id.
    let b = ctx.insert_vertex(v(1., 0., 0.), &mut log);
    assert_eq!(ctx.insert_edge(a, b, &mut log), Ok(EdgeId(0)));
}

#[test]
fn insert_face_needs_its_edges() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let vs = verts(&mut ctx, &square(0., 0., 1.), &mut log);
    let es = chain(&mut ctx, &vs, &mut log);
    let face = Face {
        outer: forward_loop(&es),
        inner: vec![forward_loop(&[EdgeId(40), EdgeId(41)])],
    };
    assert_eq!(
        ctx.insert_face(face, &mut log),
        Err(TopoError::MissingEdge(EdgeId(40)))
    );
    let face = Face {
        outer: forward_loop(&[es[0], EdgeId(50)]),
        inner: vec![forward_loop(&[EdgeId(40)])],
    };
    assert_eq!(
        ctx.insert_face(face, &mut log),
        Err(TopoError::MissingEdge(EdgeId(50)))
    );
    assert_eq!(ctx.face_count(), 0);
}

#[test]
fn deletes_refuse_entities_still_in_use() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let (f, es) = polygon(&mut ctx, &square(0., 0., 1.), &mut log);
    let a = ctx.edge(es[0]).unwrap().ends[0];
    assert_eq!(
        ctx.delete_vertex(a, &mut log),
        Err(TopoError::VertexInUse(a))
    );
    assert_eq!(
        ctx.delete_edge(es[0], &mut log),
        Err(TopoError::EdgeInUse(es[0]))
    );
    assert_eq!(
        ctx.delete_face(FaceId(99), &mut log),
        Err(TopoError::MissingFace(FaceId(99)))
    );
    assert_eq!(
        ctx.delete_edge(EdgeId(99), &mut log),
        Err(TopoError::MissingEdge(EdgeId(99)))
    );
    assert_eq!(
        ctx.delete_vertex(VertexId(99), &mut log),
        Err(TopoError::MissingVertex(VertexId(99)))
    );
    assert_eq!(
        ctx.set_vertex_position(VertexId(99), v(0., 0., 0.), &mut log),
        Err(TopoError::MissingVertex(VertexId(99)))
    );

    let face = ctx.face(f).unwrap().clone();
    let mut del = Edit::new();
    ctx.delete_face(f, &mut del).unwrap();
    assert_eq!(del.records, vec![Record::RemoveFace(f, face)]);
    let e0 = *ctx.edge(es[0]).unwrap();
    ctx.delete_edge(es[0], &mut del).unwrap();
    assert_eq!(del.records[1], Record::RemoveEdge(es[0], e0));
    assert_eq!(ctx.faces_of_edge(es[1]), vec![]);
    assert_eq!(ctx.edges_of_vertex(a), vec![es[3]]);
}

#[test]
fn record_and_edit_inverses() {
    let a = VertexId(3);
    let data = Vertex {
        position: v(1., 2., 3.),
    };
    assert_eq!(
        Record::AddVertex(a, data).inverse(),
        Record::RemoveVertex(a, data)
    );
    assert_eq!(
        Record::RemoveVertex(a, data).inverse(),
        Record::AddVertex(a, data)
    );
    let e = Edge {
        ends: [VertexId(1), VertexId(2)],
    };
    assert_eq!(
        Record::AddEdge(EdgeId(4), e).inverse(),
        Record::RemoveEdge(EdgeId(4), e)
    );
    assert_eq!(
        Record::RemoveEdge(EdgeId(4), e).inverse(),
        Record::AddEdge(EdgeId(4), e)
    );
    let f = Face {
        outer: forward_loop(&[EdgeId(1), EdgeId(2), EdgeId(3)]),
        inner: vec![],
    };
    assert_eq!(
        Record::AddFace(FaceId(5), f.clone()).inverse(),
        Record::RemoveFace(FaceId(5), f.clone())
    );
    assert_eq!(
        Record::RemoveFace(FaceId(5), f.clone()).inverse(),
        Record::AddFace(FaceId(5), f)
    );
    let m = Record::MoveVertex {
        id: a,
        from: v(0., 0., 0.),
        to: v(1., 1., 1.),
    };
    assert_eq!(
        m.inverse(),
        Record::MoveVertex {
            id: a,
            from: v(1., 1., 1.),
            to: v(0., 0., 0.)
        }
    );

    let edit = Edit {
        records: vec![Record::AddVertex(a, data), m.clone()],
    };
    assert_eq!(
        edit.inverse().records,
        vec![m.inverse(), Record::RemoveVertex(a, data)]
    );
    assert!(Edit::new().inverse().is_empty());
}

#[test]
fn undo_and_redo_a_build() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    polygon(&mut ctx, &square(0., 0., 1.), &mut log);
    let built = ctx.clone();
    ctx.apply_edit(&log.inverse()).unwrap();
    assert_eq!(ctx, Context::new());
    ctx.apply_edit(&log).unwrap();
    assert_eq!(ctx, built);
    assert!(ctx.validate().is_empty());
}

#[test]
fn ids_are_never_reused() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let a = ctx.insert_vertex(v(0., 0., 0.), &mut log);
    ctx.apply_edit(&log.inverse()).unwrap();
    assert_eq!(ctx.vertex_count(), 0);
    let b = ctx.insert_vertex(v(0., 0., 0.), &mut Edit::new());
    assert_ne!(a, b);
    assert_eq!(b, VertexId(1));
    let mut del = Edit::new();
    ctx.delete_vertex(b, &mut del).unwrap();
    assert_eq!(
        ctx.insert_vertex(v(0., 0., 0.), &mut Edit::new()),
        VertexId(2)
    );

    // Applying a record with a high id moves the counter past it.
    ctx.apply(&Record::AddVertex(
        VertexId(10),
        Vertex {
            position: v(5., 0., 0.),
        },
    ))
    .unwrap();
    assert_eq!(
        ctx.insert_vertex(v(6., 0., 0.), &mut Edit::new()),
        VertexId(11)
    );
}

#[test]
fn apply_checks_references_and_data() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let a = ctx.insert_vertex(v(0., 0., 0.), &mut log);
    let b = ctx.insert_vertex(v(1., 0., 0.), &mut log);
    let e = ctx.insert_edge(a, b, &mut log).unwrap();
    let before = ctx.clone();

    let p = Vertex {
        position: v(0., 0., 0.),
    };
    assert_eq!(ctx.apply(&Record::AddVertex(a, p)), Err(TopoError::IdInUse));
    assert_eq!(
        ctx.apply(&Record::AddEdge(e, Edge { ends: [a, b] })),
        Err(TopoError::IdInUse)
    );
    assert_eq!(
        ctx.apply(&Record::AddEdge(
            EdgeId(9),
            Edge {
                ends: [a, VertexId(8)]
            }
        )),
        Err(TopoError::MissingVertex(VertexId(8)))
    );
    assert_eq!(
        ctx.apply(&Record::AddEdge(EdgeId(9), Edge { ends: [b, b] })),
        Err(TopoError::SameVertex(b))
    );
    assert_eq!(
        ctx.apply(&Record::RemoveVertex(
            a,
            Vertex {
                position: v(9., 9., 9.)
            }
        )),
        Err(TopoError::Mismatch)
    );
    assert_eq!(
        ctx.apply(&Record::RemoveVertex(a, p)),
        Err(TopoError::VertexInUse(a))
    );
    assert_eq!(
        ctx.apply(&Record::RemoveEdge(e, Edge { ends: [b, a] })),
        Err(TopoError::Mismatch)
    );
    assert_eq!(
        ctx.apply(&Record::RemoveEdge(EdgeId(7), Edge { ends: [a, b] })),
        Err(TopoError::MissingEdge(EdgeId(7)))
    );
    let tri = Face {
        outer: forward_loop(&[e, e, e]),
        inner: vec![],
    };
    assert_eq!(
        ctx.apply(&Record::RemoveFace(FaceId(0), tri.clone())),
        Err(TopoError::MissingFace(FaceId(0)))
    );
    assert_eq!(
        ctx.apply(&Record::MoveVertex {
            id: b,
            from: v(2., 0., 0.),
            to: v(3., 0., 0.)
        }),
        Err(TopoError::Mismatch)
    );
    assert_eq!(
        ctx.apply(&Record::MoveVertex {
            id: VertexId(5),
            from: v(2., 0., 0.),
            to: v(3., 0., 0.)
        }),
        Err(TopoError::MissingVertex(VertexId(5)))
    );
    assert_eq!(ctx, before);

    // A face is removed only with matching data.
    ctx.apply(&Record::AddFace(FaceId(0), tri.clone())).unwrap();
    let other = Face {
        outer: backward_loop(&[e, e, e]),
        inner: vec![],
    };
    assert_eq!(
        ctx.apply(&Record::RemoveFace(FaceId(0), other)),
        Err(TopoError::Mismatch)
    );
    assert_eq!(
        ctx.apply(&Record::AddFace(FaceId(0), tri.clone())),
        Err(TopoError::IdInUse)
    );
    ctx.apply(&Record::RemoveFace(FaceId(0), tri)).unwrap();
    assert_eq!(ctx, before);
}

#[test]
fn apply_edit_is_all_or_nothing() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    polygon(&mut ctx, &square(0., 0., 1.), &mut log);
    let before = ctx.clone();
    let edit = Edit {
        records: vec![
            Record::AddVertex(
                VertexId(20),
                Vertex {
                    position: v(5., 5., 0.),
                },
            ),
            Record::AddVertex(
                VertexId(21),
                Vertex {
                    position: v(6., 5., 0.),
                },
            ),
            Record::AddEdge(
                EdgeId(20),
                Edge {
                    ends: [VertexId(20), VertexId(21)],
                },
            ),
            Record::MoveVertex {
                id: VertexId(0),
                from: v(0., 0., 0.),
                to: v(-1., 0., 0.),
            },
            Record::RemoveFace(FaceId(0), ctx.face(FaceId(0)).unwrap().clone()),
            Record::AddEdge(
                EdgeId(21),
                Edge {
                    ends: [VertexId(20), VertexId(99)],
                },
            ),
        ],
    };
    assert_eq!(
        ctx.apply_edit(&edit),
        Err(TopoError::MissingVertex(VertexId(99)))
    );
    assert_eq!(ctx, before);
    assert_eq!(ctx.edges_of_vertex(VertexId(20)), vec![]);
    assert_eq!(ctx.faces_of_edge(EdgeId(0)), vec![FaceId(0)]);
    assert!(ctx.validate().is_empty());
}

// -- queries ------------------------------------------------------------------------

#[test]
fn adjacency_queries() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    // Two squares sharing the edge x = 1.
    let vs = verts(
        &mut ctx,
        &[
            v(0., 0., 0.),
            v(1., 0., 0.),
            v(1., 1., 0.),
            v(0., 1., 0.),
            v(2., 0., 0.),
            v(2., 1., 0.),
        ],
        &mut log,
    );
    let left = chain(&mut ctx, &vs[0..4], &mut log); // 0-1, 1-2, 2-3, 3-0
    let shared = left[1];
    let r0 = ctx.insert_edge(vs[1], vs[4], &mut log).unwrap();
    let r1 = ctx.insert_edge(vs[4], vs[5], &mut log).unwrap();
    let r2 = ctx.insert_edge(vs[5], vs[2], &mut log).unwrap();
    let fl = ctx
        .insert_face(
            Face {
                outer: forward_loop(&left),
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();
    let right = Loop {
        uses: vec![
            EdgeUse::forward(r0),
            EdgeUse::forward(r1),
            EdgeUse::forward(r2),
            EdgeUse::backward(shared),
        ],
    };
    let fr = ctx
        .insert_face(
            Face {
                outer: right,
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();

    assert_eq!(ctx.faces_of_edge(shared), vec![fl, fr]);
    assert_eq!(ctx.faces_of_edge(left[0]), vec![fl]);
    assert_eq!(ctx.faces_of_edge(r1), vec![fr]);
    assert_eq!(ctx.faces_of_edge(EdgeId(77)), vec![]);
    assert_eq!(ctx.edges_of_vertex(vs[1]), vec![left[0], left[1], r0]);
    assert_eq!(ctx.edges_of_vertex(VertexId(77)), vec![]);
    assert_eq!(ctx.edge_between(vs[2], vs[1]), Some(shared));
    assert_eq!(ctx.edge_between(vs[1], vs[2]), Some(shared));
    assert_eq!(ctx.edge_between(vs[0], vs[2]), None);

    assert_eq!(ctx.use_start(EdgeUse::forward(shared)), Some(vs[1]));
    assert_eq!(ctx.use_start(EdgeUse::backward(shared)), Some(vs[2]));
    assert_eq!(ctx.use_start(EdgeUse::forward(EdgeId(77))), None);
    let fr_loop = ctx.face(fr).unwrap().outer.clone();
    assert_eq!(
        ctx.loop_vertices(&fr_loop),
        vec![vs[1], vs[4], vs[5], vs[2]]
    );
    assert_eq!(
        ctx.loop_points(&fr_loop),
        vec![v(1., 0., 0.), v(2., 0., 0.), v(2., 1., 0.), v(1., 1., 0.)]
    );

    let plane = ctx.face_plane(fr).unwrap();
    assert!((plane.normal - DVec3::Z).length() < 1e-12);
    assert!((plane.point - v(1.5, 0.5, 0.)).length() < 1e-12);
    assert_eq!(ctx.face_plane(FaceId(77)), None);
    assert!(ctx.validate().is_empty());

    // Moving a vertex keeps adjacency.
    ctx.set_vertex_position(vs[4], v(2., -1., 0.), &mut log)
        .unwrap();
    assert_eq!(ctx.edges_of_vertex(vs[4]), vec![r0, r1]);
}

#[test]
fn duplicate_edges_report_lowest_id() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let vs = verts(&mut ctx, &[v(0., 0., 0.), v(1., 0., 0.)], &mut log);
    let e0 = ctx.insert_edge(vs[1], vs[0], &mut log).unwrap();
    let e1 = ctx.insert_edge(vs[0], vs[1], &mut log).unwrap();
    assert_eq!(ctx.edge_between(vs[0], vs[1]), Some(e0));
    assert_eq!(ctx.edges_of_vertex(vs[0]), vec![e0, e1]);
    assert_eq!(ctx.validate(), vec![Violation::DuplicateEdge(e0, e1)]);
}

#[test]
fn find_vertex_within_tolerance() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let a = ctx.insert_vertex(v(0., 0., 0.), &mut log);
    let b = ctx.insert_vertex(v(1., 0., 0.), &mut log);
    let c = ctx.insert_vertex(v(1., 0., 0.), &mut log);
    assert_eq!(ctx.find_vertex(v(0.4, 0., 0.), 0.5), Some(a));
    assert_eq!(ctx.find_vertex(v(0.6, 0., 0.), 0.5), Some(b));
    assert_eq!(ctx.find_vertex(v(0.5, 0., 0.), 0.5), Some(a));
    assert_eq!(ctx.find_vertex(v(1., 0., 0.), 0.0), Some(b));
    assert_eq!(ctx.find_vertex(v(5., 0., 0.), 1.0), None);
    assert_ne!(ctx.find_vertex(v(1., 0., 0.), 1e-5), Some(c));
}

// -- validate: valid shapes -----------------------------------------------------------

#[test]
fn empty_context_is_valid() {
    assert!(Context::new().validate().is_empty());
}

#[test]
fn square_faces_either_way_are_valid() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let (f, es) = polygon(&mut ctx, &square(0., 0., 1.), &mut log);
    assert!(ctx.validate().is_empty());
    assert!((ctx.face_plane(f).unwrap().normal - DVec3::Z).length() < 1e-12);

    ctx.delete_face(f, &mut log).unwrap();
    let g = ctx
        .insert_face(
            Face {
                outer: backward_loop(&es),
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();
    assert!(ctx.validate().is_empty());
    assert!((ctx.face_plane(g).unwrap().normal + DVec3::Z).length() < 1e-12);
}

#[test]
fn face_with_a_clockwise_hole_is_valid() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let outer_v = verts(&mut ctx, &square(0., 0., 4.), &mut log);
    let outer = chain(&mut ctx, &outer_v, &mut log);
    let hole_v = verts(&mut ctx, &square(1., 1., 2.), &mut log);
    let hole = chain(&mut ctx, &hole_v, &mut log);
    let f = ctx
        .insert_face(
            Face {
                outer: forward_loop(&outer),
                inner: vec![backward_loop(&hole)],
            },
            &mut log,
        )
        .unwrap();
    assert_eq!(ctx.validate(), vec![]);
    assert_eq!(ctx.faces_of_edge(hole[2]), vec![f]);
}

#[test]
fn collinear_vertex_in_a_loop_is_valid() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    polygon(
        &mut ctx,
        &[
            v(0., 0., 0.),
            v(1., 0., 0.),
            v(2., 0., 0.),
            v(2., 1., 0.),
            v(0., 1., 0.),
        ],
        &mut log,
    );
    assert_eq!(ctx.validate(), vec![]);
}

#[test]
fn concave_tilted_face_is_valid() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    // An L shape in the plane x + z = 1.
    let l = [(0., 0.), (2., 0.), (2., 1.), (1., 1.), (1., 2.), (0., 2.)];
    let pts: Vec<_> = l.iter().map(|(a, b)| v(*a, *b, 1. - *a)).collect();
    polygon(&mut ctx, &pts, &mut log);
    assert_eq!(ctx.validate(), vec![]);
}

#[test]
fn three_faces_on_one_edge_are_valid() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let a = ctx.insert_vertex(v(0., 0., 0.), &mut log);
    let b = ctx.insert_vertex(v(0., 0., 1.), &mut log);
    let spine = ctx.insert_edge(a, b, &mut log).unwrap();
    for dir in [v(1., 0., 0.), v(0., 1., 0.), v(-1., -1., 0.)] {
        let c = ctx.insert_vertex(dir, &mut log);
        let e1 = ctx.insert_edge(b, c, &mut log).unwrap();
        let e2 = ctx.insert_edge(c, a, &mut log).unwrap();
        ctx.insert_face(
            Face {
                outer: forward_loop(&[spine, e1, e2]),
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();
    }
    assert_eq!(ctx.faces_of_edge(spine).len(), 3);
    assert_eq!(ctx.validate(), vec![]);
}

#[test]
fn edges_meeting_at_a_shared_vertex_are_valid() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let vs = verts(
        &mut ctx,
        &[
            v(0., 0., 0.),
            v(1., 0., 0.),
            v(2., 0., 0.),
            v(1., 1., 0.),
            v(1., 0., 1.),
        ],
        &mut log,
    );
    // Collinear end to end, plus two at right angles, all through vs[1].
    ctx.insert_edge(vs[0], vs[1], &mut log).unwrap();
    ctx.insert_edge(vs[1], vs[2], &mut log).unwrap();
    ctx.insert_edge(vs[3], vs[1], &mut log).unwrap();
    ctx.insert_edge(vs[1], vs[4], &mut log).unwrap();
    assert_eq!(ctx.validate(), vec![]);
}

// -- validate: one test per violation ---------------------------------------------------

#[test]
fn stray_vertices() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    polygon(&mut ctx, &square(0., 0., 1.), &mut log);
    let s1 = ctx.insert_vertex(v(5., 5., 5.), &mut log);
    let s0 = ctx.insert_vertex(v(6., 5., 5.), &mut log);
    assert_eq!(
        ctx.validate(),
        vec![Violation::StrayVertex(s1), Violation::StrayVertex(s0)]
    );
}

#[test]
fn duplicate_vertices() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let vs = verts(
        &mut ctx,
        &[
            v(0., 0., 0.),
            v(1., 0., 0.),
            v(5., 0., 0.),
            v(5., 0., 0.000_005),
            v(6., 0., 0.),
        ],
        &mut log,
    );
    ctx.insert_edge(vs[0], vs[1], &mut log).unwrap();
    ctx.insert_edge(vs[2], vs[4], &mut log).unwrap();
    ctx.insert_edge(vs[3], vs[4], &mut log).unwrap();
    let got = ctx.validate();
    assert_eq!(got[0], Violation::DuplicateVertex(vs[2], vs[3]));
}

#[test]
fn crossing_edges() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let vs = verts(
        &mut ctx,
        &[v(0., 0., 0.), v(2., 2., 0.), v(0., 2., 0.), v(2., 0., 0.)],
        &mut log,
    );
    let a = ctx.insert_edge(vs[0], vs[1], &mut log).unwrap();
    let b = ctx.insert_edge(vs[2], vs[3], &mut log).unwrap();
    assert_eq!(ctx.validate(), vec![Violation::EdgesIntersect(a, b)]);
}

#[test]
fn t_junction() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let vs = verts(
        &mut ctx,
        &[v(0., 0., 0.), v(2., 0., 0.), v(1., 0., 0.), v(1., 1., 0.)],
        &mut log,
    );
    let stem = ctx.insert_edge(vs[3], vs[2], &mut log).unwrap();
    let bar = ctx.insert_edge(vs[0], vs[1], &mut log).unwrap();
    assert_eq!(ctx.validate(), vec![Violation::EdgesIntersect(stem, bar)]);
}

#[test]
fn skew_edges_that_miss_are_valid_and_touching_ones_are_not() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let vs = verts(
        &mut ctx,
        &[
            v(0., 0., 0.),
            v(2., 0., 0.),
            v(1., -1., 0.001),
            v(1., 1., 0.001),
        ],
        &mut log,
    );
    ctx.insert_edge(vs[0], vs[1], &mut log).unwrap();
    ctx.insert_edge(vs[2], vs[3], &mut log).unwrap();
    assert_eq!(ctx.validate(), vec![]);
    ctx.set_vertex_position(vs[2], v(1., -1., 0.000_001), &mut log)
        .unwrap();
    ctx.set_vertex_position(vs[3], v(1., 1., 0.000_001), &mut log)
        .unwrap();
    assert_eq!(
        ctx.validate(),
        vec![Violation::EdgesIntersect(EdgeId(0), EdgeId(1))]
    );
}

#[test]
fn overlapping_edges_with_a_shared_vertex() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let vs = verts(
        &mut ctx,
        &[v(0., 0., 0.), v(2., 0., 0.), v(1., 0., 0.)],
        &mut log,
    );
    let long = ctx.insert_edge(vs[0], vs[1], &mut log).unwrap();
    let short = ctx.insert_edge(vs[0], vs[2], &mut log).unwrap();
    assert_eq!(ctx.validate(), vec![Violation::EdgesIntersect(long, short)]);
}

#[test]
fn overlapping_edges_without_a_shared_vertex() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let vs = verts(
        &mut ctx,
        &[v(0., 0., 0.), v(2., 0., 0.), v(1., 0., 0.), v(3., 0., 0.)],
        &mut log,
    );
    let a = ctx.insert_edge(vs[0], vs[1], &mut log).unwrap();
    let b = ctx.insert_edge(vs[3], vs[2], &mut log).unwrap();
    assert_eq!(ctx.validate(), vec![Violation::EdgesIntersect(a, b)]);
}

#[test]
fn open_loops() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let vs = verts(&mut ctx, &square(0., 0., 1.), &mut log);
    let es = chain(&mut ctx, &vs, &mut log);
    // Uses out of order: 0-1 then 2-3.
    let f0 = ctx
        .insert_face(
            Face {
                outer: forward_loop(&[es[0], es[2], es[1], es[3]]),
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();
    // Fewer than three uses.
    let f1 = ctx
        .insert_face(
            Face {
                outer: Loop {
                    uses: vec![EdgeUse::forward(es[0]), EdgeUse::backward(es[0])],
                },
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();
    // Wrong direction on one use (three uses, so not a duplicate of f0).
    let f2 = ctx
        .insert_face(
            Face {
                outer: Loop {
                    uses: vec![
                        EdgeUse::forward(es[0]),
                        EdgeUse::backward(es[1]),
                        EdgeUse::forward(es[2]),
                    ],
                },
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();
    assert_eq!(
        ctx.validate(),
        vec![
            Violation::OpenLoop(f0),
            Violation::OpenLoop(f1),
            Violation::OpenLoop(f2)
        ]
    );
}

#[test]
fn open_inner_loop() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let outer_v = verts(&mut ctx, &square(0., 0., 4.), &mut log);
    let outer = chain(&mut ctx, &outer_v, &mut log);
    let hole_v = verts(&mut ctx, &square(1., 1., 2.), &mut log);
    let hole = chain(&mut ctx, &hole_v, &mut log);
    let f = ctx
        .insert_face(
            Face {
                outer: forward_loop(&outer),
                inner: vec![backward_loop(&hole[..3])],
            },
            &mut log,
        )
        .unwrap();
    assert_eq!(ctx.validate(), vec![Violation::OpenLoop(f)]);
}

#[test]
fn loop_through_a_vertex_twice() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    // Bow tie: two triangles meeting at c, as one loop a-b-c-d-e-c-a.
    let vs = verts(
        &mut ctx,
        &[
            v(0., 0., 0.),
            v(1., 0., 0.),
            v(1., 1., 0.),
            v(2., 2., 0.),
            v(1., 2., 0.),
        ],
        &mut log,
    );
    let (a, b, c, d, e) = (vs[0], vs[1], vs[2], vs[3], vs[4]);
    let ab = ctx.insert_edge(a, b, &mut log).unwrap();
    let bc = ctx.insert_edge(b, c, &mut log).unwrap();
    let cd = ctx.insert_edge(c, d, &mut log).unwrap();
    let de = ctx.insert_edge(d, e, &mut log).unwrap();
    let ec = ctx.insert_edge(e, c, &mut log).unwrap();
    let ca = ctx.insert_edge(c, a, &mut log).unwrap();
    let f = ctx
        .insert_face(
            Face {
                outer: forward_loop(&[ab, bc, cd, de, ec, ca]),
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();
    assert_eq!(ctx.validate(), vec![Violation::LoopNotSimple(f)]);
}

#[test]
fn edge_used_by_both_loops_of_a_face() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let vs = verts(&mut ctx, &square(0., 0., 1.), &mut log);
    let es = chain(&mut ctx, &vs, &mut log);
    let f = ctx
        .insert_face(
            Face {
                outer: forward_loop(&es),
                inner: vec![backward_loop(&es)],
            },
            &mut log,
        )
        .unwrap();
    assert_eq!(ctx.validate(), vec![Violation::LoopNotSimple(f)]);
}

#[test]
fn non_planar_face() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let mut pts = square(0., 0., 1.);
    pts[2].z = 0.001;
    let (f, _) = polygon(&mut ctx, &pts, &mut log);
    assert_eq!(ctx.validate(), vec![Violation::NonPlanarFace(f)]);
    // Within tolerance is planar.
    let vid = ctx.loop_vertices(&ctx.face(f).unwrap().outer.clone())[2];
    ctx.set_vertex_position(vid, v(1., 1., 0.000_004), &mut log)
        .unwrap();
    assert_eq!(ctx.validate(), vec![]);
}

#[test]
fn degenerate_face_has_no_plane() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    // Three collinear vertices: the edges overlap and the face has no area.
    let vs = verts(
        &mut ctx,
        &[v(0., 0., 0.), v(1., 0., 0.), v(2., 0., 0.)],
        &mut log,
    );
    let es = chain(&mut ctx, &vs, &mut log);
    let f = ctx
        .insert_face(
            Face {
                outer: forward_loop(&es),
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();
    let got = ctx.validate();
    assert_eq!(ctx.face_plane(f), None);
    assert!(got.contains(&Violation::NonPlanarFace(f)), "{got:?}");
}

#[test]
fn self_intersecting_loop_in_plane() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    // A twisted quad (nonzero net area) whose loop crosses itself at (1, 0.5).
    let (f, _) = polygon(
        &mut ctx,
        &[v(0., 0., 0.), v(2., 0., 0.), v(0., 1., 0.), v(3., 1.5, 0.)],
        &mut log,
    );
    let got = ctx.validate();
    assert!(got.contains(&Violation::LoopNotSimple(f)), "{got:?}");
    assert!(
        got.contains(&Violation::EdgesIntersect(EdgeId(1), EdgeId(3))),
        "{got:?}"
    );
}

#[test]
fn hole_running_the_same_way_as_the_outer_loop() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let outer_v = verts(&mut ctx, &square(0., 0., 4.), &mut log);
    let outer = chain(&mut ctx, &outer_v, &mut log);
    let hole_v = verts(&mut ctx, &square(1., 1., 2.), &mut log);
    let hole = chain(&mut ctx, &hole_v, &mut log);
    let f = ctx
        .insert_face(
            Face {
                outer: forward_loop(&outer),
                inner: vec![forward_loop(&hole)],
            },
            &mut log,
        )
        .unwrap();
    assert_eq!(ctx.validate(), vec![Violation::HoleWinding(f)]);
}

#[test]
fn hole_outside_the_outer_loop() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let outer_v = verts(&mut ctx, &square(0., 0., 4.), &mut log);
    let outer = chain(&mut ctx, &outer_v, &mut log);
    let hole_v = verts(&mut ctx, &square(5., 1., 1.), &mut log);
    let hole = chain(&mut ctx, &hole_v, &mut log);
    let f = ctx
        .insert_face(
            Face {
                outer: forward_loop(&outer),
                inner: vec![backward_loop(&hole)],
            },
            &mut log,
        )
        .unwrap();
    assert_eq!(ctx.validate(), vec![Violation::HoleOutside(f)]);
}

#[test]
fn hole_touching_the_outer_loop_at_a_vertex() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let outer_v = verts(&mut ctx, &square(0., 0., 4.), &mut log);
    let outer = chain(&mut ctx, &outer_v, &mut log);
    // A triangle hole with one corner on the outer loop's corner (0, 0).
    let p = verts(&mut ctx, &[v(1., 2., 0.), v(2., 1., 0.)], &mut log);
    let hole = chain(&mut ctx, &[outer_v[0], p[0], p[1]], &mut log);
    let f = ctx
        .insert_face(
            Face {
                outer: forward_loop(&outer),
                inner: vec![forward_loop(&hole)],
            },
            &mut log,
        )
        .unwrap();
    assert_eq!(ctx.validate(), vec![Violation::HoleOutside(f)]);
}

#[test]
fn nested_holes() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let outer_v = verts(&mut ctx, &square(0., 0., 10.), &mut log);
    let outer = chain(&mut ctx, &outer_v, &mut log);
    let big_v = verts(&mut ctx, &square(1., 1., 6.), &mut log);
    let big = chain(&mut ctx, &big_v, &mut log);
    let small_v = verts(&mut ctx, &square(2., 2., 1.), &mut log);
    let small = chain(&mut ctx, &small_v, &mut log);
    let side_v = verts(&mut ctx, &square(8., 8., 1.), &mut log);
    let side = chain(&mut ctx, &side_v, &mut log);
    let ok = ctx
        .insert_face(
            Face {
                outer: forward_loop(&outer),
                inner: vec![backward_loop(&big), backward_loop(&side)],
            },
            &mut log,
        )
        .unwrap();
    assert_eq!(ctx.validate(), vec![]);
    ctx.delete_face(ok, &mut log).unwrap();
    let f = ctx
        .insert_face(
            Face {
                outer: forward_loop(&outer),
                inner: vec![
                    backward_loop(&side),
                    backward_loop(&small),
                    backward_loop(&big),
                ],
            },
            &mut log,
        )
        .unwrap();
    assert_eq!(ctx.validate(), vec![Violation::HolesTouch(f)]);
}

#[test]
fn duplicate_faces() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let (f0, es) = polygon(&mut ctx, &square(0., 0., 1.), &mut log);
    let (f1, _) = polygon(&mut ctx, &square(3., 0., 1.), &mut log);
    let f2 = ctx
        .insert_face(
            Face {
                outer: backward_loop(&es),
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();
    let rotated = Loop {
        uses: [es[2], es[3], es[0], es[1]]
            .iter()
            .map(|e| EdgeUse::forward(*e))
            .collect(),
    };
    let f3 = ctx
        .insert_face(
            Face {
                outer: rotated,
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();
    assert_ne!(f1, f2);
    assert_eq!(
        ctx.validate(),
        vec![
            Violation::DuplicateFace(f0, f2),
            Violation::DuplicateFace(f0, f3),
            Violation::DuplicateFace(f2, f3),
        ]
    );
}

#[test]
fn violations_come_in_the_documented_order() {
    let mut ctx = Context::new();
    let mut log = Edit::new();
    let mut pts = square(0., 0., 1.);
    pts[2].z = 0.5;
    let (bent, es) = polygon(&mut ctx, &pts, &mut log);
    let dup = ctx
        .insert_face(
            Face {
                outer: forward_loop(&es),
                inner: vec![],
            },
            &mut log,
        )
        .unwrap();
    let a = ctx.insert_vertex(v(5., 0., 0.), &mut log);
    let b = ctx.insert_vertex(v(5., 1., 0.), &mut log);
    let e1 = ctx.insert_edge(a, b, &mut log).unwrap();
    let e2 = ctx.insert_edge(b, a, &mut log).unwrap();
    let stray = ctx.insert_vertex(v(9., 9., 9.), &mut log);
    let near = ctx.insert_vertex(v(9., 9., 9.000_001), &mut log);
    assert_eq!(
        ctx.validate(),
        vec![
            Violation::StrayVertex(stray),
            Violation::StrayVertex(near),
            Violation::DuplicateVertex(stray, near),
            Violation::DuplicateEdge(e1, e2),
            Violation::NonPlanarFace(bent),
            Violation::NonPlanarFace(dup),
            Violation::DuplicateFace(bent, dup),
        ]
    );
}
