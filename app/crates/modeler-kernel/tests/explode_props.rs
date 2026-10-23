// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Explode properties (`docs/specs/m4-explode.md`): a cube instance on a grid offset from a
//! cube in the root (apart, touching at a corner, an edge or a face, or in the same place),
//! turned by quarter turns and maybe mirrored.

use std::collections::{BTreeMap, BTreeSet};

use glam::{DMat4, DVec3};
use modeler_kernel::explode::explode;
use modeler_kernel::model::*;
use modeler_kernel::topology::*;
use proptest::prelude::*;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn cube_points(at: DVec3) -> Vec<DVec3> {
    let mut pts = Vec::new();
    for z in [0.0, 1.0] {
        for (x, y) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            pts.push(at + v(x, y, z));
        }
    }
    pts
}

const FACES: [&[usize]; 6] = [
    &[0, 3, 2, 1],
    &[4, 5, 6, 7],
    &[0, 1, 5, 4],
    &[2, 3, 7, 6],
    &[1, 2, 6, 5],
    &[3, 0, 4, 7],
];

fn mesh_in(model: &mut Model, body: BodyId, points: &[DVec3]) {
    let mut edit = ModelEdit::new();
    model
        .edit_geometry(body, &mut edit, |ctx, log| {
            let vs: Vec<VertexId> = points.iter().map(|&p| ctx.insert_vertex(p, log)).collect();
            let mut edges: BTreeMap<(usize, usize), EdgeId> = BTreeMap::new();
            for face in FACES {
                let mut uses = Vec::new();
                for i in 0..face.len() {
                    let (a, b) = (face[i], face[(i + 1) % face.len()]);
                    let key = (a.min(b), a.max(b));
                    let e = *edges
                        .entry(key)
                        .or_insert_with(|| ctx.insert_edge(vs[key.0], vs[key.1], log).unwrap());
                    uses.push(if a < b {
                        EdgeUse::forward(e)
                    } else {
                        EdgeUse::backward(e)
                    });
                }
                ctx.insert_face(
                    Face {
                        outer: Loop { uses },
                        inner: vec![],
                    },
                    log,
                )
                .unwrap();
            }
        })
        .unwrap();
}

type Key = [i64; 3];
type Cells = (BTreeSet<Key>, BTreeSet<(Key, Key)>, BTreeSet<Vec<Key>>);

fn key(p: DVec3) -> Key {
    [
        (p.x * 1e6).round() as i64,
        (p.y * 1e6).round() as i64,
        (p.z * 1e6).round() as i64,
    ]
}

/// Grid points, unit edges and unit squares of the unit cube at integer corner `at`.
fn cube_cells(at: [i64; 3]) -> Cells {
    let c = v(at[0] as f64, at[1] as f64, at[2] as f64);
    let pts = cube_points(c);
    let points: BTreeSet<Key> = pts.iter().map(|&p| key(p)).collect();
    let mut edges = BTreeSet::new();
    let mut squares = BTreeSet::new();
    for face in FACES {
        let mut corners: Vec<Key> = face.iter().map(|&i| key(pts[i])).collect();
        for i in 0..4 {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            edges.insert((a.min(b), a.max(b)));
        }
        corners.sort();
        squares.insert(corners);
    }
    (points, edges, squares)
}

fn instance_matrix(offset: [i64; 3], quarter_turns: u32, mirror: bool) -> DMat4 {
    let centre = v(0.5, 0.5, 0.5);
    let turn = DMat4::from_rotation_z(f64::from(quarter_turns) * std::f64::consts::FRAC_PI_2);
    let flip = if mirror {
        DMat4::from_scale(v(-1.0, 1.0, 1.0))
    } else {
        DMat4::IDENTITY
    };
    DMat4::from_translation(v(offset[0] as f64, offset[1] as f64, offset[2] as f64) + centre)
        * turn
        * flip
        * DMat4::from_translation(-centre)
}

fn face_key(ctx: &Context, f: FaceId) -> Vec<Key> {
    let mut pts: Vec<Key> = ctx
        .loop_points(&ctx.face(f).unwrap().outer)
        .into_iter()
        .map(key)
        .collect();
    pts.sort();
    pts
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn explode_next_to_a_cube(
        offset in prop::array::uniform3(-1i64..=2),
        quarter_turns in 0u32..4,
        mirror in any::<bool>(),
    ) {
        let mut model = Model::new();
        mesh_in(&mut model, BodyId::Root, &cube_points(DVec3::ZERO));
        let mut edit = ModelEdit::new();
        let d = model.add_definition(
            DefinitionInfo { name: "box".into(), group: false },
            &mut edit,
        );
        mesh_in(&mut model, BodyId::Definition(d), &cube_points(DVec3::ZERO));
        let m = instance_matrix(offset, quarter_turns, mirror);
        let inst = model
            .add_instance(
                BodyId::Root,
                Instance { definition: d, transform: m, name: String::new(), material: None },
                &mut edit,
            )
            .unwrap();
        let before = model.clone();
        let mut undo = UndoStack::new();
        explode(&mut model, BodyId::Root, inst, &mut undo, "Explode").expect("explode");
        prop_assert_eq!(model.validate(), vec![]);

        let (mut points, mut edges, mut squares) = cube_cells([0, 0, 0]);
        let (p2, e2, s2) = cube_cells(offset);
        points.extend(p2);
        edges.extend(e2);
        squares.extend(s2);
        let ctx = model.root().geometry();
        let got_points: BTreeSet<Key> = ctx.vertices().map(|(_, x)| key(x.position)).collect();
        prop_assert_eq!(got_points, points);
        prop_assert_eq!(ctx.vertex_count(), ctx.vertices().count());
        let got_edges: BTreeSet<(Key, Key)> = ctx
            .edges()
            .map(|(_, e)| {
                let a = key(ctx.vertex(e.ends[0]).unwrap().position);
                let b = key(ctx.vertex(e.ends[1]).unwrap().position);
                (a.min(b), a.max(b))
            })
            .collect();
        prop_assert_eq!(ctx.edge_count(), edges.len());
        prop_assert_eq!(got_edges, edges);
        let got_squares: BTreeSet<Vec<Key>> = ctx.faces().map(|(f, _)| face_key(ctx, f)).collect();
        prop_assert_eq!(ctx.face_count(), squares.len());
        prop_assert_eq!(got_squares, squares.clone());

        // A face on only one cube points out of that cube.
        let (_, _, first) = cube_cells([0, 0, 0]);
        let (_, _, second) = cube_cells(offset);
        let o = v(offset[0] as f64, offset[1] as f64, offset[2] as f64);
        for (f, face) in ctx.faces() {
            let k = face_key(ctx, f);
            let centre = match (first.contains(&k), second.contains(&k)) {
                (true, false) => v(0.5, 0.5, 0.5),
                (false, true) => o + v(0.5, 0.5, 0.5),
                _ => continue,
            };
            let pts = ctx.loop_points(&face.outer);
            let mid = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
            let n = ctx.face_plane(f).unwrap().normal;
            prop_assert!(n.dot(mid - centre) > 0.0, "face {:?} points inward", k);
        }

        let after = model.clone();
        undo.undo(&mut model).unwrap();
        prop_assert_eq!(&model, &before);
        undo.redo(&mut model).unwrap();
        prop_assert_eq!(&model, &after);
    }
}
