// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Make component from a random part of a cube (`docs/specs/m4-make-component.md`).

use glam::DVec3;
use modeler_kernel::component::make_component;
use modeler_kernel::model::*;
use modeler_kernel::select::Entity;
use modeler_kernel::topology::EdgeId;
use proptest::prelude::*;

mod transform_common;
use transform_common::*;

fn key(p: DVec3) -> [i64; 3] {
    [
        (p.x * 1e6).round() as i64,
        (p.y * 1e6).round() as i64,
        (p.z * 1e6).round() as i64,
    ]
}

/// Every edge of the model in world space, as sorted endpoint keys, root and instances.
fn world_edges(model: &Model) -> Vec<[[i64; 3]; 2]> {
    let mut out = Vec::new();
    let mut add = |ctx: &modeler_kernel::topology::Context, xf: glam::DMat4| {
        for (_, e) in ctx.edges() {
            let a = key(xf.transform_point3(ctx.vertex(e.ends[0]).unwrap().position));
            let b = key(xf.transform_point3(ctx.vertex(e.ends[1]).unwrap().position));
            out.push(if a <= b { [a, b] } else { [b, a] });
        }
    };
    add(model.root().geometry(), glam::DMat4::IDENTITY);
    for (_, i) in model.root().instances() {
        add(
            model
                .body(BodyId::Definition(i.definition))
                .unwrap()
                .geometry(),
            i.transform,
        );
    }
    out.sort();
    out.dedup();
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn making_a_component_from_part_of_a_cube(mask in 1u32..(1 << 18), group in any::<bool>()) {
        let mut model = Model::new();
        let faces = cube(&mut model);
        let edges: Vec<EdgeId> = model.root().geometry().edges().map(|(id, _)| id).collect();
        let mut sel = Vec::new();
        for (i, f) in faces.iter().enumerate() {
            if mask & (1 << i) != 0 {
                sel.push(Entity::Face(*f));
            }
        }
        for (i, e) in edges.iter().enumerate() {
            if mask & (1 << (6 + i)) != 0 {
                sel.push(Entity::Edge(*e));
            }
        }
        let n_faces = sel.iter().filter(|e| matches!(e, Entity::Face(_))).count();
        let before = model.clone();
        let before_edges = world_edges(&model);
        let mut undo = UndoStack::new();
        let info = DefinitionInfo { name: String::new(), group };
        let inst = make_component(&mut model, BodyId::Root, &sel, info, &mut undo, "M").unwrap();
        prop_assert!(model.validate().is_empty(), "{:?}", model.validate());
        prop_assert_eq!(undo.undo_count(), 1);

        let i = model.root().instance(inst).unwrap().clone();
        let def = model.body(BodyId::Definition(i.definition)).unwrap();
        // The selected faces moved; the others stayed.
        prop_assert_eq!(def.geometry().face_count(), n_faces);
        prop_assert_eq!(model.root().geometry().face_count(), 6 - n_faces);
        for (_, f) in def.geometry().faces() {
            prop_assert!(f.inner.is_empty());
        }
        // Nothing moved in world space: every edge is still there, and no new one appeared.
        prop_assert_eq!(world_edges(&model), before_edges);
        // The definition sits at its own origin: its box min corner is (0, 0, 0).
        let mut lo = DVec3::splat(f64::INFINITY);
        for (_, vx) in def.geometry().vertices() {
            lo = lo.min(vx.position);
        }
        prop_assert!(lo.length() < 1e-9, "{:?}", lo);

        undo.undo(&mut model).unwrap();
        prop_assert_eq!(&model, &before);
    }
}
