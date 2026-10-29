// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Solid repair on random boxes (`docs/specs/m7-solid-repair.md`).

use glam::{DMat4, DVec3};
use modeler_kernel::model::*;
use modeler_kernel::repair::repair_body;
use modeler_kernel::solid::check_body;
use modeler_kernel::topology::FaceId;
use proptest::prelude::*;

mod transform_common;
use transform_common::*;

const FACES: [[usize; 4]; 6] = [
    [0, 3, 2, 1],
    [4, 5, 6, 7],
    [0, 1, 5, 4],
    [2, 3, 7, 6],
    [1, 2, 6, 5],
    [3, 0, 4, 7],
];

const UNIT: [DVec3; 8] = [
    DVec3::new(0.0, 0.0, 0.0),
    DVec3::new(1.0, 0.0, 0.0),
    DVec3::new(1.0, 1.0, 0.0),
    DVec3::new(0.0, 1.0, 0.0),
    DVec3::new(0.0, 0.0, 1.0),
    DVec3::new(1.0, 0.0, 1.0),
    DVec3::new(1.0, 1.0, 1.0),
    DVec3::new(0.0, 1.0, 1.0),
];

/// A box of the given size, turned and moved by `m`, with the faces in `flip` (bit i = face
/// i) stored back to front and face `drop` (if any) left out. Returns the face ids, `None`
/// for the dropped face.
fn placed_box(
    model: &mut Model,
    size: DVec3,
    m: DMat4,
    flip: u32,
    drop: Option<usize>,
) -> Vec<Option<FaceId>> {
    let pts: Vec<DVec3> = UNIT.iter().map(|p| m.transform_point3(*p * size)).collect();
    let faces: Vec<Vec<usize>> = FACES
        .iter()
        .enumerate()
        .filter(|(i, _)| drop != Some(*i))
        .map(|(i, f)| {
            let mut f = f.to_vec();
            if flip & (1 << i) != 0 {
                f.reverse();
            }
            f
        })
        .collect();
    let refs: Vec<&[usize]> = faces.iter().map(|f| f.as_slice()).collect();
    let mut ids = mesh(model, &pts, &refs).into_iter();
    (0..6)
        .map(|i| if drop == Some(i) { None } else { ids.next() })
        .collect()
}

/// Two boxes of the given size side by side along x, sharing the wall between them. Returns
/// the wall's id.
fn walled_pair(model: &mut Model, size: DVec3, m: DMat4) -> FaceId {
    let mut pts = Vec::new();
    for z in [0.0, 1.0] {
        for y in [0.0, 1.0] {
            for x in [0.0, 1.0, 2.0] {
                pts.push(m.transform_point3(DVec3::new(x, y, z) * size));
            }
        }
    }
    // Index = x + 3 y + 6 z.
    let faces: [&[usize]; 11] = [
        &[0, 3, 4, 1],
        &[1, 4, 5, 2],
        &[6, 7, 10, 9],
        &[7, 8, 11, 10],
        &[0, 1, 7, 6],
        &[1, 2, 8, 7],
        &[4, 3, 9, 10],
        &[5, 4, 10, 11],
        &[3, 0, 6, 9],
        &[2, 5, 11, 8],
        &[1, 4, 10, 7],
    ];
    mesh(model, &pts, &faces)[10]
}

fn placement() -> impl Strategy<Value = (DVec3, DMat4)> {
    (
        (0.001f64..5.0, 0.001f64..5.0, 0.001f64..5.0),
        (-3.2f64..3.2, -3.2f64..3.2, -3.2f64..3.2),
        (-100.0f64..100.0, -100.0f64..100.0, -100.0f64..100.0),
    )
        .prop_map(|((sx, sy, sz), (a, b, c), (tx, ty, tz))| {
            let m = DMat4::from_translation(v(tx, ty, tz))
                * DMat4::from_rotation_z(a)
                * DMat4::from_rotation_y(b)
                * DMat4::from_rotation_x(c);
            (v(sx, sy, sz), m)
        })
}

fn volume_close(got: f64, want: f64) -> bool {
    (got - want).abs() <= 1e-9 * (1.0 + want) + 1e-6
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Any box with some faces flipped and at most one face missing repairs to a clean
    /// solid of the right volume, in one undo step that undoes exactly.
    #[test]
    fn broken_box_repairs_to_a_clean_solid(
        (size, m) in placement(),
        flip in 0u32..64,
        drop in proptest::option::of(0usize..6),
    ) {
        let mut model = Model::new();
        let ids = placed_box(&mut model, size, m, flip, drop);
        let before = model.clone();
        let mut undo = UndoStack::new();
        let r = repair_body(&mut model, BodyId::Root, &mut undo, "Repair Solid").unwrap();

        prop_assert!(model.validate().is_empty(), "{:?}", model.validate());
        prop_assert!(r.after.is_clean(), "{:?}", r);
        prop_assert_eq!(Some(r.after.clone()), check_body(&model, BodyId::Root));
        let vol = size.x * size.y * size.z;
        prop_assert!(volume_close(r.after.volume.unwrap(), vol), "{:?} vs {}", r.after.volume, vol);
        prop_assert_eq!(r.filled.len(), usize::from(drop.is_some()));
        prop_assert!(r.unfixed.is_empty(), "{:?}", r.unfixed);
        prop_assert!(r.removed_internal.is_empty() && r.removed_stray.is_empty());
        prop_assert!(r.removed_edges.is_empty());

        // Every flipped face is turned; the fill may be turned too, nothing else.
        let flipped: Vec<FaceId> = (0..6)
            .filter(|i| flip & (1 << i) != 0)
            .filter_map(|i| ids[i])
            .collect();
        for f in &flipped {
            prop_assert!(r.reversed.contains(f), "{:?} not in {:?}", f, r.reversed);
        }
        for f in &r.reversed {
            prop_assert!(flipped.contains(f) || r.filled.contains(f), "{:?}", f);
        }

        let changed = !flipped.is_empty() || drop.is_some();
        prop_assert_eq!(r.changed(), changed);
        prop_assert_eq!(undo.undo_count(), usize::from(changed));
        if changed {
            undo.undo(&mut model).unwrap();
        }
        prop_assert_eq!(&model, &before);
    }

    /// Repairing a clean box changes nothing at all.
    #[test]
    fn clean_box_is_left_alone((size, m) in placement()) {
        let mut model = Model::new();
        placed_box(&mut model, size, m, 0, None);
        let before = model.clone();
        let revision = model.revision();
        let mut undo = UndoStack::new();
        let r = repair_body(&mut model, BodyId::Root, &mut undo, "Repair Solid").unwrap();
        prop_assert!(!r.changed());
        prop_assert!(r.unfixed.is_empty());
        prop_assert!(r.after.is_clean());
        prop_assert_eq!(&model, &before);
        prop_assert_eq!(model.revision(), revision);
        prop_assert_eq!(undo.undo_count(), 0);
    }

    /// A second repair finds nothing left to do.
    #[test]
    fn repair_is_idempotent(
        (size, m) in placement(),
        flip in 0u32..64,
        drop in proptest::option::of(0usize..6),
    ) {
        let mut model = Model::new();
        placed_box(&mut model, size, m, flip, drop);
        let mut undo = UndoStack::new();
        repair_body(&mut model, BodyId::Root, &mut undo, "Repair Solid").unwrap();
        let once = model.clone();
        let steps = undo.undo_count();
        let r = repair_body(&mut model, BodyId::Root, &mut undo, "Repair Solid").unwrap();
        prop_assert!(!r.changed(), "{:?}", r);
        prop_assert_eq!(&model, &once);
        prop_assert_eq!(undo.undo_count(), steps);
    }

    /// The wall between two boxes is removed and the pair becomes one solid.
    #[test]
    fn internal_wall_is_removed((size, m) in placement()) {
        let mut model = Model::new();
        let wall = walled_pair(&mut model, size, m);
        let mut undo = UndoStack::new();
        let r = repair_body(&mut model, BodyId::Root, &mut undo, "Repair Solid").unwrap();
        prop_assert_eq!(&r.removed_internal, &vec![wall]);
        prop_assert!(r.after.is_clean(), "{:?}", r);
        prop_assert_eq!(r.after.shells, 1);
        let vol = 2.0 * size.x * size.y * size.z;
        prop_assert!(volume_close(r.after.volume.unwrap(), vol), "{:?} vs {}", r.after.volume, vol);
        prop_assert!(model.validate().is_empty());
    }
}
