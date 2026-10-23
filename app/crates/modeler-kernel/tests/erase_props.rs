// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Erase a random part of a cube (`docs/specs/m3-erase.md`).

use modeler_kernel::erase::erase;
use modeler_kernel::model::*;
use modeler_kernel::select::Entity;
use modeler_kernel::topology::EdgeId;
use proptest::prelude::*;

mod transform_common;
use transform_common::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn erasing_part_of_a_cube(mask in 1u32..(1 << 18)) {
        let mut model = Model::new();
        let faces = cube(&mut model);
        let edges: Vec<EdgeId> = model.root().geometry().edges().map(|(id, _)| id).collect();
        prop_assert_eq!(edges.len(), 12);
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
        let before = model.clone();
        let mut undo = UndoStack::new();
        erase(&mut model, BodyId::Root, &sel, &mut undo, "Erase").unwrap();
        prop_assert!(model.validate().is_empty());
        prop_assert_eq!(undo.undo_count(), 1);

        let c = model.root().geometry();
        let old = before.root().geometry();
        // Selected edges are gone, the others stay.
        for e in &edges {
            prop_assert_eq!(c.edge(*e).is_some(), !sel.contains(&Entity::Edge(*e)));
        }
        // A face stays exactly when it isn't selected and none of its edges is.
        for f in &faces {
            let face = old.face(*f).unwrap();
            let hit = sel.contains(&Entity::Face(*f))
                || face.outer.uses.iter().any(|u| sel.contains(&Entity::Edge(u.edge)));
            prop_assert_eq!(c.face(*f).is_some(), !hit);
        }
        // No vertex is left without edges.
        for (id, _) in c.vertices() {
            prop_assert!(!c.edges_of_vertex(id).is_empty());
        }

        let after = model.clone();
        undo.undo(&mut model).unwrap();
        prop_assert_eq!(&model, &before);
        undo.redo(&mut model).unwrap();
        prop_assert_eq!(&model, &after);
    }
}
