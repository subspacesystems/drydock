// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use glam::{DMat4, DVec3};
use modeler_kernel::model::*;
use modeler_kernel::topology::*;

fn mat(name: &str) -> Material {
    Material {
        name: name.to_string(),
        color: [0.5, 0.25, 1.0, 1.0],
        texture: None,
    }
}

fn info(name: &str, group: bool) -> DefinitionInfo {
    DefinitionInfo {
        name: name.to_string(),
        group,
    }
}

fn inst(d: DefinitionId) -> Instance {
    Instance {
        definition: d,
        transform: DMat4::from_translation(DVec3::new(1.0, 2.0, 3.0)),
        name: String::new(),
        material: None,
    }
}

fn body(d: DefinitionId) -> BodyId {
    BodyId::Definition(d)
}

/// `log` (the records of one call, starting from `before`) undoes to `before` and redoes to
/// `after`.
fn assert_undo_redo(before: &Model, after: &Model, log: &ModelEdit) {
    let mut undone = after.clone();
    undone.apply_edit(&log.inverse()).unwrap();
    assert_eq!(&undone, before, "undo");
    let mut redone = before.clone();
    redone.apply_edit(log).unwrap();
    assert_eq!(&redone, after, "redo");
}

/// A triangle in the body's geometry, built with topology primitives. Returns its face.
fn triangle(m: &mut Model, b: BodyId, log: &mut ModelEdit) -> FaceId {
    m.edit_geometry(b, log, |ctx, edit| {
        let v: Vec<_> = [
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(0.0, 1.0, 0.0),
        ]
        .iter()
        .map(|p| ctx.insert_vertex(*p, edit))
        .collect();
        let uses = (0..3)
            .map(|i| EdgeUse::forward(ctx.insert_edge(v[i], v[(i + 1) % 3], edit).unwrap()))
            .collect();
        ctx.insert_face(
            Face {
                outer: Loop { uses },
                inner: vec![],
            },
            edit,
        )
        .unwrap()
    })
    .unwrap()
}

// -- materials --------------------------------------------------------------------------

#[test]
fn new_model_is_empty_and_valid() {
    let m = Model::new();
    assert!(m.root().is_empty());
    assert_eq!(m.definitions().count(), 0);
    assert_eq!(m.materials().count(), 0);
    assert_eq!(m.validate(), vec![]);
    assert!(m.body(BodyId::Definition(DefinitionId(0))).is_none());
}

#[test]
fn add_set_remove_material() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_material(mat("a"), &mut log);
    let b = m.add_material(mat("b"), &mut log);
    assert_eq!((a, b), (MaterialId(0), MaterialId(1)));
    assert_eq!(m.material(b).unwrap().name, "b");
    assert_eq!(
        log.records,
        vec![
            ModelRecord::AddMaterial(a, mat("a")),
            ModelRecord::AddMaterial(b, mat("b"))
        ]
    );
    let before = m.clone();
    let mut log = ModelEdit::new();
    let mut red = mat("red");
    red.color = [1.0, 0.0, 0.0, 0.5];
    red.texture = Some("brick.png".into());
    m.set_material(a, red.clone(), &mut log).unwrap();
    assert_eq!(m.material(a), Some(&red));
    assert_eq!(
        log.records,
        vec![ModelRecord::SetMaterial {
            id: a,
            from: mat("a"),
            to: red.clone()
        }]
    );
    assert_undo_redo(&before, &m, &log);
    let before = m.clone();
    let mut log = ModelEdit::new();
    m.remove_material(a, &mut log).unwrap();
    assert!(m.material(a).is_none());
    assert_eq!(log.records, vec![ModelRecord::RemoveMaterial(a, red)]);
    assert_undo_redo(&before, &m, &log);
}

#[test]
fn material_errors() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_material(mat("a"), &mut log);
    let before = m.clone();
    let mut log = ModelEdit::new();
    assert_eq!(
        m.set_material(MaterialId(5), mat("x"), &mut log),
        Err(ModelError::MissingMaterial(MaterialId(5)))
    );
    assert_eq!(
        m.remove_material(MaterialId(5), &mut log),
        Err(ModelError::MissingMaterial(MaterialId(5)))
    );
    assert_eq!(
        m.apply(&ModelRecord::AddMaterial(a, mat("x"))),
        Err(ModelError::IdInUse)
    );
    assert_eq!(
        m.apply(&ModelRecord::RemoveMaterial(a, mat("x"))),
        Err(ModelError::Mismatch)
    );
    assert_eq!(
        m.apply(&ModelRecord::SetMaterial {
            id: a,
            from: mat("x"),
            to: mat("y")
        }),
        Err(ModelError::Mismatch)
    );
    assert!(log.is_empty());
    assert_eq!(m, before);
}

#[test]
fn ids_are_never_reused() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_material(mat("a"), &mut log);
    m.apply_edit(&log.inverse()).unwrap();
    assert_eq!(m, Model::new());
    let mut log = ModelEdit::new();
    assert_eq!(m.add_material(mat("b"), &mut log), MaterialId(a.0 + 1));
    let d = m.add_definition(info("d", false), &mut log);
    m.remove_definition(d, &mut log).unwrap();
    assert_eq!(
        m.add_definition(info("e", false), &mut log),
        DefinitionId(d.0 + 1)
    );
    // Applying a record with a high id moves the counter past it.
    m.apply(&ModelRecord::AddMaterial(MaterialId(10), mat("z")))
        .unwrap();
    assert_eq!(m.add_material(mat("w"), &mut log), MaterialId(11));
    m.apply(&ModelRecord::AddDefinition(
        DefinitionId(20),
        info("z", false),
    ))
    .unwrap();
    assert_eq!(
        m.add_definition(info("w", false), &mut log),
        DefinitionId(21)
    );
}

#[test]
fn failed_call_does_not_use_an_id() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let d = m.add_definition(info("d", false), &mut log);
    assert_eq!(
        m.add_instance(BodyId::Root, inst(DefinitionId(9)), &mut log),
        Err(ModelError::MissingDefinition(DefinitionId(9)))
    );
    assert_eq!(
        m.add_instance(BodyId::Root, inst(d), &mut log),
        Ok(InstanceId(0))
    );
}

#[test]
fn clone_keeps_counters() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_material(mat("a"), &mut log);
    m.remove_material(a, &mut log).unwrap();
    let mut c = m.clone();
    assert_eq!(c.add_material(mat("b"), &mut log), MaterialId(1));
}

#[test]
fn equality_ignores_counters() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_material(mat("a"), &mut log);
    m.remove_material(a, &mut log).unwrap();
    let d = m.add_definition(info("d", false), &mut log);
    m.remove_definition(d, &mut log).unwrap();
    assert_eq!(m, Model::new());
}

#[test]
fn material_in_use_by_instance_or_face_entry() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_material(mat("a"), &mut log);
    let b = m.add_material(mat("b"), &mut log);
    let d = m.add_definition(info("d", false), &mut log);
    let mut i = inst(d);
    i.material = Some(a);
    let id = m.add_instance(BodyId::Root, i, &mut log).unwrap();
    // A face entry for a face that doesn't exist still counts.
    let fm = FaceMaterials {
        front: None,
        back: Some(b),
        ..Default::default()
    };
    m.set_face_materials(body(d), FaceId(42), fm, &mut log)
        .unwrap();
    assert_eq!(
        m.remove_material(a, &mut log),
        Err(ModelError::MaterialInUse(a))
    );
    assert_eq!(
        m.remove_material(b, &mut log),
        Err(ModelError::MaterialInUse(b))
    );
    m.remove_instance(BodyId::Root, id, &mut log).unwrap();
    m.set_face_materials(body(d), FaceId(42), FaceMaterials::default(), &mut log)
        .unwrap();
    m.remove_material(a, &mut log).unwrap();
    m.remove_material(b, &mut log).unwrap();
}

// -- definitions ------------------------------------------------------------------------

#[test]
fn add_set_remove_definition() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let d = m.add_definition(info("Chair", false), &mut log);
    assert_eq!(d, DefinitionId(0));
    assert_eq!(m.definition(d), Some(&info("Chair", false)));
    assert!(m.body(body(d)).unwrap().is_empty());
    assert_eq!(
        log.records,
        vec![ModelRecord::AddDefinition(d, info("Chair", false))]
    );
    let before = m.clone();
    let mut log = ModelEdit::new();
    m.set_definition(d, info("Group#1", true), &mut log)
        .unwrap();
    assert_eq!(m.definition(d).unwrap().name, "Group#1");
    assert_eq!(
        log.records,
        vec![ModelRecord::SetDefinition {
            id: d,
            from: info("Chair", false),
            to: info("Group#1", true)
        }]
    );
    assert_undo_redo(&before, &m, &log);
    let before = m.clone();
    let mut log = ModelEdit::new();
    m.remove_definition(d, &mut log).unwrap();
    assert!(m.definition(d).is_none());
    assert!(m.body(body(d)).is_none());
    assert_undo_redo(&before, &m, &log);
}

#[test]
fn definition_errors() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let d = m.add_definition(info("d", false), &mut log);
    let e = m.add_definition(info("e", false), &mut log);
    let mut log = ModelEdit::new();
    assert_eq!(
        m.set_definition(DefinitionId(9), info("x", false), &mut log),
        Err(ModelError::MissingDefinition(DefinitionId(9)))
    );
    assert_eq!(
        m.remove_definition(DefinitionId(9), &mut log),
        Err(ModelError::MissingDefinition(DefinitionId(9)))
    );
    assert_eq!(
        m.apply(&ModelRecord::AddDefinition(d, info("x", false))),
        Err(ModelError::IdInUse)
    );
    assert_eq!(
        m.apply(&ModelRecord::RemoveDefinition(d, info("x", false))),
        Err(ModelError::Mismatch)
    );
    assert_eq!(
        m.apply(&ModelRecord::SetDefinition {
            id: d,
            from: info("x", false),
            to: info("y", false)
        }),
        Err(ModelError::Mismatch)
    );
    // In use: an instance of `d` inside `e`.
    let i = m.add_instance(body(e), inst(d), &mut log).unwrap();
    assert_eq!(
        m.remove_definition(d, &mut log),
        Err(ModelError::DefinitionInUse(d))
    );
    // Not empty: `e` holds that instance.
    assert_eq!(
        m.remove_definition(e, &mut log),
        Err(ModelError::DefinitionNotEmpty(e))
    );
    m.remove_instance(body(e), i, &mut log).unwrap();
    // Not empty: geometry.
    let mut g = ModelEdit::new();
    triangle(&mut m, body(e), &mut g);
    assert_eq!(
        m.remove_definition(e, &mut log),
        Err(ModelError::DefinitionNotEmpty(e))
    );
    m.apply_edit(&g.inverse()).unwrap();
    // Not empty: a face material entry alone.
    let a = m.add_material(mat("a"), &mut log);
    let fm = FaceMaterials {
        front: Some(a),
        back: None,
        ..Default::default()
    };
    m.set_face_materials(body(e), FaceId(0), fm, &mut log)
        .unwrap();
    assert_eq!(
        m.remove_definition(e, &mut log),
        Err(ModelError::DefinitionNotEmpty(e))
    );
    m.set_face_materials(body(e), FaceId(0), FaceMaterials::default(), &mut log)
        .unwrap();
    m.remove_definition(e, &mut log).unwrap();
    m.remove_definition(d, &mut log).unwrap();
}

// -- instances --------------------------------------------------------------------------

#[test]
fn add_set_remove_instance() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let d = m.add_definition(info("d", false), &mut log);
    let a = m.add_material(mat("a"), &mut log);
    let before = m.clone();
    let mut log = ModelEdit::new();
    let i = m.add_instance(BodyId::Root, inst(d), &mut log).unwrap();
    assert_eq!(i, InstanceId(0));
    assert_eq!(m.root().instance(i), Some(&inst(d)));
    assert_eq!(
        log.records,
        vec![ModelRecord::AddInstance(BodyId::Root, i, inst(d))]
    );
    assert_undo_redo(&before, &m, &log);

    let before = m.clone();
    let mut log = ModelEdit::new();
    let mut moved = inst(d);
    moved.transform = DMat4::from_rotation_z(0.5) * moved.transform;
    moved.material = Some(a);
    moved.name = "left".into();
    m.set_instance(BodyId::Root, i, moved.clone(), &mut log)
        .unwrap();
    assert_eq!(m.root().instance(i), Some(&moved));
    assert_eq!(
        log.records,
        vec![ModelRecord::SetInstance {
            body: BodyId::Root,
            id: i,
            from: inst(d),
            to: moved.clone()
        }]
    );
    assert_undo_redo(&before, &m, &log);

    let before = m.clone();
    let mut log = ModelEdit::new();
    m.remove_instance(BodyId::Root, i, &mut log).unwrap();
    assert!(m.root().instance(i).is_none());
    assert_eq!(
        log.records,
        vec![ModelRecord::RemoveInstance(BodyId::Root, i, moved)]
    );
    assert_undo_redo(&before, &m, &log);
}

#[test]
fn instance_ids_are_unique_across_bodies() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let d = m.add_definition(info("d", false), &mut log);
    let e = m.add_definition(info("e", false), &mut log);
    let i0 = m.add_instance(BodyId::Root, inst(e), &mut log).unwrap();
    let i1 = m.add_instance(body(d), inst(e), &mut log).unwrap();
    let i2 = m.add_instance(BodyId::Root, inst(d), &mut log).unwrap();
    assert_eq!((i0, i1, i2), (InstanceId(0), InstanceId(1), InstanceId(2)));
    // The same id in another body is in use.
    assert_eq!(
        m.apply(&ModelRecord::AddInstance(body(d), i0, inst(e))),
        Err(ModelError::IdInUse)
    );
    assert_eq!(m.instances_of(e), vec![(BodyId::Root, i0), (body(d), i1)]);
    assert_eq!(m.instances_of(d), vec![(BodyId::Root, i2)]);
    assert_eq!(m.instances_of(DefinitionId(99)), vec![]);
    assert_eq!(m.body(body(d)).unwrap().instances().count(), 1);
}

#[test]
fn instances_of_orders_root_then_definitions() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let leaf = m.add_definition(info("leaf", false), &mut log);
    let b = m.add_definition(info("b", false), &mut log);
    let a = m.add_definition(info("a", false), &mut log);
    let x = m.add_instance(body(a), inst(leaf), &mut log).unwrap();
    let y = m.add_instance(body(b), inst(leaf), &mut log).unwrap();
    let z = m.add_instance(BodyId::Root, inst(leaf), &mut log).unwrap();
    let w = m.add_instance(body(b), inst(leaf), &mut log).unwrap();
    assert_eq!(
        m.instances_of(leaf),
        vec![(BodyId::Root, z), (body(b), y), (body(b), w), (body(a), x)]
    );
}

#[test]
fn instance_errors() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let d = m.add_definition(info("d", false), &mut log);
    let i = m.add_instance(BodyId::Root, inst(d), &mut log).unwrap();
    let before = m.clone();
    let mut log = ModelEdit::new();
    let nowhere = body(DefinitionId(77));
    assert_eq!(
        m.add_instance(nowhere, inst(d), &mut log),
        Err(ModelError::MissingBody(nowhere))
    );
    let mut bad = inst(d);
    bad.material = Some(MaterialId(3));
    assert_eq!(
        m.add_instance(BodyId::Root, bad.clone(), &mut log),
        Err(ModelError::MissingMaterial(MaterialId(3)))
    );
    assert_eq!(
        m.set_instance(BodyId::Root, i, bad, &mut log),
        Err(ModelError::MissingMaterial(MaterialId(3)))
    );
    assert_eq!(
        m.set_instance(BodyId::Root, i, inst(DefinitionId(8)), &mut log),
        Err(ModelError::MissingDefinition(DefinitionId(8)))
    );
    assert_eq!(
        m.set_instance(body(d), i, inst(d), &mut log),
        Err(ModelError::MissingInstance(body(d), i))
    );
    assert_eq!(
        m.remove_instance(BodyId::Root, InstanceId(5), &mut log),
        Err(ModelError::MissingInstance(BodyId::Root, InstanceId(5)))
    );
    assert_eq!(
        m.remove_instance(nowhere, i, &mut log),
        Err(ModelError::MissingBody(nowhere))
    );
    assert_eq!(
        m.apply(&ModelRecord::RemoveInstance(
            BodyId::Root,
            i,
            inst(DefinitionId(1))
        )),
        Err(ModelError::Mismatch)
    );
    assert_eq!(
        m.apply(&ModelRecord::SetInstance {
            body: BodyId::Root,
            id: i,
            from: inst(DefinitionId(1)),
            to: inst(d)
        }),
        Err(ModelError::Mismatch)
    );
    assert!(log.is_empty());
    assert_eq!(m, before);
}

#[test]
fn cycles_are_refused() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_definition(info("a", false), &mut log);
    let b = m.add_definition(info("b", false), &mut log);
    let c = m.add_definition(info("c", false), &mut log);
    // a contains b, b contains c.
    m.add_instance(body(a), inst(b), &mut log).unwrap();
    let bc = m.add_instance(body(b), inst(c), &mut log).unwrap();
    let before = m.clone();
    let mut log = ModelEdit::new();
    assert_eq!(
        m.add_instance(body(a), inst(a), &mut log),
        Err(ModelError::Cycle)
    );
    assert_eq!(
        m.add_instance(body(b), inst(a), &mut log),
        Err(ModelError::Cycle)
    );
    assert_eq!(
        m.add_instance(body(c), inst(a), &mut log),
        Err(ModelError::Cycle)
    );
    assert_eq!(
        m.add_instance(body(c), inst(b), &mut log),
        Err(ModelError::Cycle)
    );
    assert_eq!(
        m.set_instance(body(b), bc, inst(a), &mut log),
        Err(ModelError::Cycle)
    );
    assert_eq!(
        m.set_instance(body(b), bc, inst(b), &mut log),
        Err(ModelError::Cycle)
    );
    assert!(log.is_empty());
    assert_eq!(m, before);
    // Not cycles: the root, a sibling, the same definition twice.
    m.add_instance(BodyId::Root, inst(a), &mut log).unwrap();
    m.add_instance(body(a), inst(c), &mut log).unwrap();
    m.add_instance(body(a), inst(c), &mut log).unwrap();
    m.set_instance(body(b), bc, inst(c), &mut log).unwrap();
    assert_eq!(m.validate(), vec![]);
}

// -- face materials -----------------------------------------------------------------------

#[test]
fn face_materials_set_and_clear() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_material(mat("a"), &mut log);
    let b = m.add_material(mat("b"), &mut log);
    let f = triangle(&mut m, BodyId::Root, &mut log);
    assert_eq!(m.root().face_materials(f), FaceMaterials::default());
    let before = m.clone();
    let mut log = ModelEdit::new();
    let fm = FaceMaterials {
        front: Some(a),
        back: Some(b),
        ..Default::default()
    };
    m.set_face_materials(BodyId::Root, f, fm, &mut log).unwrap();
    assert_eq!(m.root().face_materials(f), fm);
    assert_eq!(
        m.root().face_material_entries().collect::<Vec<_>>(),
        vec![(f, fm)]
    );
    assert_eq!(
        log.records,
        vec![ModelRecord::SetFaceMaterials {
            body: BodyId::Root,
            face: f,
            from: FaceMaterials::default(),
            to: fm
        }]
    );
    assert_undo_redo(&before, &m, &log);
    // Back to the default removes the entry, so the body compares equal to before.
    let mut log = ModelEdit::new();
    m.set_face_materials(BodyId::Root, f, FaceMaterials::default(), &mut log)
        .unwrap();
    assert_eq!(m.root().face_material_entries().count(), 0);
    assert_eq!(m, before);
}

#[test]
fn face_material_errors() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_material(mat("a"), &mut log);
    let before = m.clone();
    let mut log = ModelEdit::new();
    let nowhere = body(DefinitionId(3));
    let fm = FaceMaterials {
        front: Some(a),
        back: None,
        ..Default::default()
    };
    assert_eq!(
        m.set_face_materials(nowhere, FaceId(0), fm, &mut log),
        Err(ModelError::MissingBody(nowhere))
    );
    let bad = FaceMaterials {
        front: Some(MaterialId(7)),
        back: Some(MaterialId(8)),
        ..Default::default()
    };
    assert_eq!(
        m.set_face_materials(BodyId::Root, FaceId(0), bad, &mut log),
        Err(ModelError::MissingMaterial(MaterialId(7)))
    );
    let bad_back = FaceMaterials {
        front: Some(a),
        back: Some(MaterialId(8)),
        ..Default::default()
    };
    assert_eq!(
        m.set_face_materials(BodyId::Root, FaceId(0), bad_back, &mut log),
        Err(ModelError::MissingMaterial(MaterialId(8)))
    );
    assert_eq!(
        m.apply(&ModelRecord::SetFaceMaterials {
            body: BodyId::Root,
            face: FaceId(0),
            from: fm,
            to: FaceMaterials::default()
        }),
        Err(ModelError::Mismatch)
    );
    assert!(log.is_empty());
    assert_eq!(m, before);
}

#[test]
fn paint_survives_removing_and_restoring_a_face() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_material(mat("a"), &mut log);
    let f = triangle(&mut m, BodyId::Root, &mut log);
    let fm = FaceMaterials {
        front: Some(a),
        back: None,
        ..Default::default()
    };
    m.set_face_materials(BodyId::Root, f, fm, &mut log).unwrap();
    let painted = m.clone();
    let mut erase = ModelEdit::new();
    m.edit_geometry(BodyId::Root, &mut erase, |ctx, edit| {
        ctx.delete_face(f, edit)
    })
    .unwrap()
    .unwrap();
    assert!(m.root().geometry().face(f).is_none());
    assert_eq!(m.root().face_materials(f), fm);
    m.apply_edit(&erase.inverse()).unwrap();
    assert_eq!(m, painted);
}

// -- geometry ---------------------------------------------------------------------------

#[test]
fn edit_geometry_wraps_records() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let d = m.add_definition(info("d", false), &mut log);
    let before = m.clone();
    let mut log = ModelEdit::new();
    let f = triangle(&mut m, body(d), &mut log);
    let geom = m.body(body(d)).unwrap().geometry();
    assert_eq!(geom.face_count(), 1);
    assert!(geom.face(f).is_some());
    assert!(m.root().geometry().face(f).is_none());
    assert_eq!(log.records.len(), 7);
    for r in &log.records {
        assert!(matches!(r, ModelRecord::Geometry(b, _) if *b == body(d)));
    }
    assert!(matches!(
        &log.records[0],
        ModelRecord::Geometry(_, Record::AddVertex(VertexId(0), _))
    ));
    assert_undo_redo(&before, &m, &log);
}

#[test]
fn edit_geometry_missing_body_does_not_call() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let mut called = false;
    let nowhere = body(DefinitionId(0));
    let r = m.edit_geometry(nowhere, &mut log, |_, _| called = true);
    assert_eq!(r, Err(ModelError::MissingBody(nowhere)));
    assert!(!called);
    assert!(log.is_empty());
}

#[test]
fn edit_geometry_appends_to_log() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_material(mat("a"), &mut log);
    triangle(&mut m, BodyId::Root, &mut log);
    assert_eq!(log.records.len(), 8);
    assert_eq!(log.records[0], ModelRecord::AddMaterial(a, mat("a")));
}

#[test]
fn geometry_record_errors() {
    let mut m = Model::new();
    let nowhere = body(DefinitionId(1));
    let r = Record::AddVertex(
        VertexId(0),
        Vertex {
            position: DVec3::ZERO,
        },
    );
    assert_eq!(
        m.apply(&ModelRecord::Geometry(nowhere, r.clone())),
        Err(ModelError::MissingBody(nowhere))
    );
    m.apply(&ModelRecord::Geometry(BodyId::Root, r.clone()))
        .unwrap();
    assert_eq!(
        m.apply(&ModelRecord::Geometry(BodyId::Root, r)),
        Err(ModelError::Topo(TopoError::IdInUse))
    );
}

// -- records and edits --------------------------------------------------------------------

#[test]
fn record_inverses() {
    let d = DefinitionId(1);
    let r = Record::AddVertex(VertexId(2), Vertex { position: DVec3::X });
    let fm = FaceMaterials {
        front: Some(MaterialId(1)),
        back: None,
        ..Default::default()
    };
    let pairs = [
        (
            ModelRecord::Geometry(body(d), r.clone()),
            ModelRecord::Geometry(body(d), r.inverse()),
        ),
        (
            ModelRecord::AddMaterial(MaterialId(1), mat("a")),
            ModelRecord::RemoveMaterial(MaterialId(1), mat("a")),
        ),
        (
            ModelRecord::SetMaterial {
                id: MaterialId(1),
                from: mat("a"),
                to: mat("b"),
            },
            ModelRecord::SetMaterial {
                id: MaterialId(1),
                from: mat("b"),
                to: mat("a"),
            },
        ),
        (
            ModelRecord::AddDefinition(d, info("a", true)),
            ModelRecord::RemoveDefinition(d, info("a", true)),
        ),
        (
            ModelRecord::SetDefinition {
                id: d,
                from: info("a", true),
                to: info("b", false),
            },
            ModelRecord::SetDefinition {
                id: d,
                from: info("b", false),
                to: info("a", true),
            },
        ),
        (
            ModelRecord::AddInstance(BodyId::Root, InstanceId(4), inst(d)),
            ModelRecord::RemoveInstance(BodyId::Root, InstanceId(4), inst(d)),
        ),
        (
            ModelRecord::SetInstance {
                body: BodyId::Root,
                id: InstanceId(4),
                from: inst(d),
                to: inst(DefinitionId(2)),
            },
            ModelRecord::SetInstance {
                body: BodyId::Root,
                id: InstanceId(4),
                from: inst(DefinitionId(2)),
                to: inst(d),
            },
        ),
        (
            ModelRecord::SetFaceMaterials {
                body: body(d),
                face: FaceId(3),
                from: FaceMaterials::default(),
                to: fm,
            },
            ModelRecord::SetFaceMaterials {
                body: body(d),
                face: FaceId(3),
                from: fm,
                to: FaceMaterials::default(),
            },
        ),
    ];
    for (a, b) in pairs {
        assert_eq!(a.inverse(), b);
        assert_eq!(b.inverse(), a);
    }
}

#[test]
fn edit_inverse_reverses_order() {
    let e = ModelEdit {
        records: vec![
            ModelRecord::AddMaterial(MaterialId(0), mat("a")),
            ModelRecord::AddDefinition(DefinitionId(0), info("d", false)),
        ],
    };
    assert_eq!(
        e.inverse().records,
        vec![
            ModelRecord::RemoveDefinition(DefinitionId(0), info("d", false)),
            ModelRecord::RemoveMaterial(MaterialId(0), mat("a")),
        ]
    );
    assert!(ModelEdit::new().inverse().is_empty());
}

#[test]
fn apply_edit_rolls_back_on_error() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    m.add_material(mat("keep"), &mut log);
    let before = m.clone();
    let edit = ModelEdit {
        records: vec![
            ModelRecord::AddMaterial(MaterialId(5), mat("a")),
            ModelRecord::AddDefinition(DefinitionId(0), info("d", false)),
            ModelRecord::AddInstance(BodyId::Root, InstanceId(0), inst(DefinitionId(0))),
            ModelRecord::Geometry(
                BodyId::Definition(DefinitionId(0)),
                Record::AddVertex(
                    VertexId(0),
                    Vertex {
                        position: DVec3::ZERO,
                    },
                ),
            ),
            // Fails: material 9 doesn't exist.
            ModelRecord::SetFaceMaterials {
                body: BodyId::Root,
                face: FaceId(0),
                from: FaceMaterials::default(),
                to: FaceMaterials {
                    front: Some(MaterialId(9)),
                    back: None,
                    ..Default::default()
                },
            },
        ],
    };
    assert_eq!(
        m.apply_edit(&edit),
        Err(ModelError::MissingMaterial(MaterialId(9)))
    );
    assert_eq!(m, before);
}

// -- validate -----------------------------------------------------------------------------

#[test]
fn validate_reports_in_order() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let g = m.add_definition(info("g", true), &mut log);
    let d = m.add_definition(info("d", false), &mut log);
    let h = m.add_definition(info("h", true), &mut log);
    // A stray vertex in d and in the root.
    m.edit_geometry(body(d), &mut log, |ctx, edit| {
        ctx.insert_vertex(DVec3::ZERO, edit)
    })
    .unwrap();
    m.edit_geometry(BodyId::Root, &mut log, |ctx, edit| {
        ctx.insert_vertex(DVec3::ONE, edit)
    })
    .unwrap();
    // Bad transforms: singular in the root, NaN in d.
    let mut flat = inst(g);
    flat.transform = DMat4::from_scale(DVec3::new(1.0, 1.0, 0.0));
    let i0 = m.add_instance(BodyId::Root, flat, &mut log).unwrap();
    let mut nan = inst(h);
    nan.transform.w_axis.x = f64::NAN;
    let i1 = m.add_instance(body(d), nan, &mut log).unwrap();
    m.add_instance(body(d), inst(h), &mut log).unwrap();
    // g has one instance (fine), h has two.
    assert_eq!(
        m.validate(),
        vec![
            ModelViolation::Geometry(BodyId::Root, Violation::StrayVertex(VertexId(0))),
            ModelViolation::BadTransform(BodyId::Root, i0),
            ModelViolation::Geometry(body(d), Violation::StrayVertex(VertexId(0))),
            ModelViolation::BadTransform(body(d), i1),
            ModelViolation::GroupInstances(h, 2),
        ]
    );
}

#[test]
fn validate_group_without_instance() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let g = m.add_definition(info("g", true), &mut log);
    m.add_definition(info("c", false), &mut log);
    assert_eq!(m.validate(), vec![ModelViolation::GroupInstances(g, 0)]);
    m.add_instance(BodyId::Root, inst(g), &mut log).unwrap();
    assert_eq!(m.validate(), vec![]);
}

#[test]
fn validate_accepts_mirrored_and_scaled_transforms() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let d = m.add_definition(info("d", false), &mut log);
    for t in [
        DMat4::from_scale(DVec3::new(-1.0, 1.0, 1.0)),
        DMat4::from_scale(DVec3::splat(1e-3)),
        DMat4::from_rotation_x(1.0) * DMat4::from_translation(DVec3::splat(5.0)),
    ] {
        let mut i = inst(d);
        i.transform = t;
        m.add_instance(BodyId::Root, i, &mut log).unwrap();
    }
    assert_eq!(m.validate(), vec![]);
}

// -- undo stack ---------------------------------------------------------------------------

#[test]
fn undo_stack_walks_back_and_forth() {
    let mut m = Model::new();
    let mut stack = UndoStack::new();
    assert_eq!(stack.undo_label(), None);
    assert_eq!(stack.undo(&mut m), Ok(None));
    assert_eq!(stack.redo(&mut m), Ok(None));

    let mut log = ModelEdit::new();
    let a = m.add_material(mat("a"), &mut log);
    stack.push("Add material", log);
    let s1 = m.clone();
    let mut log = ModelEdit::new();
    triangle(&mut m, BodyId::Root, &mut log);
    stack.push("Draw", log);
    let s2 = m.clone();
    let mut log = ModelEdit::new();
    m.set_face_materials(
        BodyId::Root,
        FaceId(0),
        FaceMaterials {
            front: Some(a),
            back: None,
            ..Default::default()
        },
        &mut log,
    )
    .unwrap();
    stack.push("Paint", log);
    let s3 = m.clone();

    assert_eq!(stack.undo_label(), Some("Paint"));
    assert_eq!(stack.redo_label(), None);
    assert_eq!(stack.undo(&mut m), Ok(Some("Paint".to_string())));
    assert_eq!(m, s2);
    assert_eq!(stack.undo(&mut m), Ok(Some("Draw".to_string())));
    assert_eq!(m, s1);
    assert_eq!(stack.undo_label(), Some("Add material"));
    assert_eq!(stack.redo_label(), Some("Draw"));
    assert_eq!(stack.redo(&mut m), Ok(Some("Draw".to_string())));
    assert_eq!(m, s2);
    assert_eq!(stack.redo(&mut m), Ok(Some("Paint".to_string())));
    assert_eq!(m, s3);
    assert_eq!(stack.redo(&mut m), Ok(None));
    for _ in 0..3 {
        stack.undo(&mut m).unwrap();
    }
    assert_eq!(m, Model::new());
    assert_eq!(stack.undo(&mut m), Ok(None));
}

#[test]
fn push_clears_redo_and_ignores_empty() {
    let mut m = Model::new();
    let mut stack = UndoStack::new();
    let mut log = ModelEdit::new();
    m.add_material(mat("a"), &mut log);
    stack.push("A", log);
    stack.undo(&mut m).unwrap();
    assert_eq!(stack.redo_label(), Some("A"));
    // An empty edit leaves both lists alone.
    stack.push("Nothing", ModelEdit::new());
    assert_eq!(stack.redo_label(), Some("A"));
    assert_eq!(stack.undo_label(), None);
    let mut log = ModelEdit::new();
    m.add_material(mat("b"), &mut log);
    stack.push("B", log);
    assert_eq!(stack.redo_label(), None);
    assert_eq!(stack.undo_label(), Some("B"));
}

#[test]
fn failed_undo_leaves_stack() {
    let mut m = Model::new();
    let mut stack = UndoStack::new();
    let mut log = ModelEdit::new();
    let a = m.add_material(mat("a"), &mut log);
    stack.push("A", log);
    // Change the material behind the stack's back; undoing the add now mismatches.
    let mut side = ModelEdit::new();
    m.set_material(a, mat("changed"), &mut side).unwrap();
    let before = m.clone();
    assert_eq!(stack.undo(&mut m), Err(ModelError::Mismatch));
    assert_eq!(m, before);
    assert_eq!(stack.undo_label(), Some("A"));
    assert_eq!(stack.redo_label(), None);
}
