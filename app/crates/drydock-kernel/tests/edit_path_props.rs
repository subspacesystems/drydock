// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Picking and inference inside an open instance agree with the world view
//! (`docs/specs/m4-edit-path.md`).

use drydock_kernel::editpath::EditPath;
use drydock_kernel::infer::*;
use drydock_kernel::model::*;
use drydock_kernel::select::Entity;
use glam::{DMat4, DQuat, DVec2, DVec3};
use proptest::prelude::*;

const AXES: [DVec3; 3] = [DVec3::X, DVec3::Y, DVec3::Z];

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn top_project(q: DVec3) -> Option<DVec2> {
    Some(DVec2::new(q.x, q.y) * 10.0)
}

/// Top-view probe over world (x, y) for a body whose coordinates map to the world with `m`.
fn local<R>(m: DMat4, x: f64, y: f64, f: impl FnOnce(&Probe) -> R) -> R {
    let inv = m.inverse();
    let project = move |q: DVec3| top_project(m.transform_point3(q));
    let probe = Probe {
        cursor: DVec2::new(x, y) * 10.0,
        origin: inv.transform_point3(v(x, y, 100.0)),
        dir: inv.transform_vector3(v(0.0, 0.0, -1.0)),
        parallel: true,
        project: &project,
        radius: 5.0,
        anchor: None,
        axes: AXES,
        centers: &[],
        lock: None,
    };
    f(&probe)
}

fn edges(m: &mut Model, body: BodyId, segments: &[(DVec3, DVec3)]) {
    let mut log = ModelEdit::new();
    m.edit_geometry(body, &mut log, |ctx, edit| {
        for &(a, b) in segments {
            ctx.add_edge(a, b, edit).unwrap();
        }
    })
    .unwrap();
}

fn cube_corners() -> Vec<DVec3> {
    let mut out = Vec::new();
    for z in [0.0, 1.0] {
        for y in [0.0, 1.0] {
            for x in [0.0, 1.0] {
                out.push(v(x, y, z));
            }
        }
    }
    out
}

fn cube_segments() -> Vec<(DVec3, DVec3)> {
    let c = cube_corners();
    let mut out = Vec::new();
    for i in 0..8 {
        for j in (i + 1)..8 {
            if (c[i] - c[j]).length() == 1.0 {
                out.push((c[i], c[j]));
            }
        }
    }
    out
}

fn snap_kind(s: Snap) -> u8 {
    match s {
        Snap::Endpoint(_) => 0,
        Snap::Midpoint(_) => 1,
        Snap::Center(_) => 2,
        Snap::OnEdge(_) => 3,
        Snap::OnAxis(_) => 4,
        Snap::OnFace(_) => 5,
        Snap::Ground => 6,
        Snap::Along => 7,
    }
}

/// Points worth aiming at, in the cube's coordinates: corners, edge midpoints, face centers.
fn targets() -> Vec<DVec3> {
    let mut out = cube_corners();
    out.extend(cube_segments().iter().map(|(a, b)| (*a + *b) / 2.0));
    for axis in 0..3 {
        for side in [0.0, 1.0] {
            let mut p = DVec3::splat(0.5);
            p[axis] = side;
            out.push(p);
        }
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn open_instance_agrees_with_the_world_view(
        turn in 0.0f64..std::f64::consts::TAU,
        tilt in prop::sample::select(vec![0.0f64, 0.35, 0.8]),
        scale in 0.5f64..2.0,
        tx in -5.0f64..5.0,
        ty in -5.0f64..5.0,
        tz in -1.0f64..1.0,
        target in 0usize..26,
        root_target in any::<bool>(),
        dx in -0.15f64..0.15,
        dy in -0.15f64..0.15,
    ) {
        let mut m = Model::new();
        let mut log = ModelEdit::new();
        let d = m.add_definition(DefinitionInfo { name: "cube".into(), group: false }, &mut log);
        edges(&mut m, BodyId::Definition(d), &cube_segments());
        prop_assert_eq!(m.body(BodyId::Definition(d)).unwrap().geometry().face_count(), 6);
        // A root square away from the cube, as the rest of the model.
        let sq = [v(20.0, 20.0, 0.0), v(21.0, 20.0, 0.0), v(21.0, 21.0, 0.0), v(20.0, 21.0, 0.0)];
        edges(&mut m, BodyId::Root, &[(sq[0], sq[1]), (sq[1], sq[2]), (sq[2], sq[3]), (sq[3], sq[0])]);
        let t = DMat4::from_translation(v(tx, ty, tz))
            * DMat4::from_quat(DQuat::from_rotation_z(turn) * DQuat::from_rotation_x(tilt))
            * DMat4::from_scale(DVec3::splat(scale));
        let a = m
            .add_instance(
                BodyId::Root,
                Instance { definition: d, transform: t, name: String::new(), material: None },
                &mut log,
            )
            .unwrap();
        let open = EditPath::root().open(&m, a).unwrap();
        prop_assert_eq!(open.body(&m), Some(BodyId::Definition(d)));

        let aim = if root_target { sq[target % 4] } else { t.transform_point3(targets()[target]) };
        let (x, y) = (aim.x + dx, aim.y + dy);

        let world = local(DMat4::IDENTITY, x, y, |pr| infer_model(&m, pr));
        let inside = local(t, x, y, |pr| infer_in(&m, &open, true, pr));
        match (world, inside) {
            (None, None) => {}
            (Some(w), Some(i)) => {
                prop_assert_eq!(snap_kind(w.snap), snap_kind(i.snap), "{:?} vs {:?}", w, i);
                if w.snap != Snap::Ground {
                    let back = t.transform_point3(i.point);
                    prop_assert!(back.distance(w.point) < 1e-6, "{:?} vs {:?}", w.point, back);
                }
            }
            other => prop_assert!(false, "{:?}", other),
        }

        let world_pick = local(DMat4::IDENTITY, x, y, |pr| pick_model(&m, pr));
        let inside_pick = local(t, x, y, |pr| pick_in(&m, &open, true, pr));
        let on_instance = world_pick == Some(Entity::Instance(a));
        let on_open = matches!(inside_pick, Some(Entity::Edge(_) | Entity::Face(_)));
        prop_assert_eq!(on_instance, on_open, "{:?} vs {:?}", world_pick, inside_pick);
        prop_assert!(inside_pick.is_none() || on_open);

        // Without the rest, the open body's own points still land where they show.
        if !root_target
            && let Some(i) = local(t, x, y, |pr| infer_in(&m, &open, false, pr))
            && matches!(i.snap, Snap::Endpoint(_) | Snap::Midpoint(_))
        {
            let w = world.unwrap();
            prop_assert_eq!(snap_kind(w.snap), snap_kind(i.snap));
            prop_assert!(t.transform_point3(i.point).distance(w.point) < 1e-6);
        }
    }
}
