// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use std::collections::BTreeSet;

use drydock_kernel::model::*;
use drydock_kernel::topology::*;
use glam::{DMat4, DVec3};
use proptest::prelude::*;

#[derive(Clone, Debug)]
enum Op {
    AddMaterial,
    SetMaterial(usize),
    RemoveMaterial(usize),
    AddDefinition(bool),
    SetDefinition(usize),
    RemoveDefinition(usize),
    AddInstance {
        body: usize,
        def: usize,
        mat: Option<usize>,
    },
    SetInstance {
        pick: usize,
        def: usize,
        angle: f64,
    },
    RemoveInstance(usize),
    Paint {
        body: usize,
        face: u64,
        front: Option<usize>,
        back: Option<usize>,
    },
    Draw(usize),
    EraseFace {
        body: usize,
        pick: usize,
    },
}

fn op() -> impl Strategy<Value = Op> {
    let ix = 0usize..8;
    prop_oneof![
        2 => Just(Op::AddMaterial),
        1 => ix.clone().prop_map(Op::SetMaterial),
        1 => ix.clone().prop_map(Op::RemoveMaterial),
        2 => any::<bool>().prop_map(Op::AddDefinition),
        1 => ix.clone().prop_map(Op::SetDefinition),
        1 => ix.clone().prop_map(Op::RemoveDefinition),
        4 => (ix.clone(), ix.clone(), prop::option::of(ix.clone()))
            .prop_map(|(body, def, mat)| Op::AddInstance { body, def, mat }),
        2 => (ix.clone(), ix.clone(), -3.0f64..3.0)
            .prop_map(|(pick, def, angle)| Op::SetInstance { pick, def, angle }),
        1 => ix.clone().prop_map(Op::RemoveInstance),
        2 => (ix.clone(), 0u64..4, prop::option::of(ix.clone()), prop::option::of(ix.clone()))
            .prop_map(|(body, face, front, back)| Op::Paint { body, face, front, back }),
        2 => ix.clone().prop_map(Op::Draw),
        1 => (ix.clone(), ix).prop_map(|(body, pick)| Op::EraseFace { body, pick }),
    ]
}

fn bodies(m: &Model) -> Vec<BodyId> {
    std::iter::once(BodyId::Root)
        .chain(m.definitions().map(|(d, _)| BodyId::Definition(d)))
        .collect()
}

/// The i-th existing material (mod count), or one that doesn't exist.
fn pick_material(m: &Model, i: usize) -> MaterialId {
    let all: Vec<_> = m.materials().map(|(id, _)| id).collect();
    if all.is_empty() {
        MaterialId(1000)
    } else {
        all[i % all.len()]
    }
}

fn pick_definition(m: &Model, i: usize) -> DefinitionId {
    let all: Vec<_> = m.definitions().map(|(id, _)| id).collect();
    if all.is_empty() {
        DefinitionId(1000)
    } else {
        all[i % all.len()]
    }
}

fn all_instances(m: &Model) -> Vec<(BodyId, InstanceId, Instance)> {
    bodies(m)
        .into_iter()
        .flat_map(|b| {
            m.body(b)
                .unwrap()
                .instances()
                .map(|(id, i)| (b, id, i.clone()))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn run_op(m: &mut Model, op: &Op, draws: &mut u32, log: &mut ModelEdit) -> Result<(), ModelError> {
    match op {
        Op::AddMaterial => {
            m.add_material(
                Material {
                    name: format!("m{}", m.materials().count()),
                    color: [0.2, 0.4, 0.6, 1.0],
                    texture: None,
                },
                log,
            );
            Ok(())
        }
        Op::SetMaterial(i) => {
            let id = pick_material(m, *i);
            let mut mat = m.material(id).cloned().unwrap_or(Material {
                name: String::new(),
                color: [0.0; 4],
                texture: None,
            });
            mat.color[0] += 0.125;
            mat.texture = Some(Texture::from(format!("t{i}.png").as_str()));
            m.set_material(id, mat, log)
        }
        Op::RemoveMaterial(i) => m.remove_material(pick_material(m, *i), log),
        Op::AddDefinition(group) => {
            m.add_definition(
                DefinitionInfo {
                    name: format!("d{}", m.definitions().count()),
                    group: *group,
                },
                log,
            );
            Ok(())
        }
        Op::SetDefinition(i) => {
            let id = pick_definition(m, *i);
            let mut info = m.definition(id).cloned().unwrap_or(DefinitionInfo {
                name: String::new(),
                group: false,
            });
            info.name.push('x');
            m.set_definition(id, info, log)
        }
        Op::RemoveDefinition(i) => m.remove_definition(pick_definition(m, *i), log),
        Op::AddInstance { body, def, mat } => {
            let bs = bodies(m);
            let b = bs[body % bs.len()];
            let inst = Instance {
                definition: pick_definition(m, *def),
                transform: DMat4::from_translation(DVec3::new(*def as f64, 0.0, 1.0)),
                name: String::new(),
                material: mat.map(|i| pick_material(m, i)),
            };
            m.add_instance(b, inst, log).map(|_| ())
        }
        Op::SetInstance { pick, def, angle } => {
            let all = all_instances(m);
            if all.is_empty() {
                return Ok(());
            }
            let (b, id, mut inst) = all[pick % all.len()].clone();
            inst.definition = pick_definition(m, *def);
            inst.transform = DMat4::from_rotation_z(*angle) * inst.transform;
            m.set_instance(b, id, inst, log)
        }
        Op::RemoveInstance(pick) => {
            let all = all_instances(m);
            if all.is_empty() {
                return Ok(());
            }
            let (b, id, _) = all[pick % all.len()];
            m.remove_instance(b, id, log)
        }
        Op::Paint {
            body,
            face,
            front,
            back,
        } => {
            let bs = bodies(m);
            let b = bs[body % bs.len()];
            let fm = FaceMaterials {
                front: front.map(|i| pick_material(m, i)),
                back: back.map(|i| pick_material(m, i)),
                ..Default::default()
            };
            m.set_face_materials(b, FaceId(*face), fm, log)
        }
        Op::Draw(body) => {
            let bs = bodies(m);
            let b = bs[body % bs.len()];
            *draws += 1;
            let x = *draws as f64 * 10.0;
            m.edit_geometry(b, log, |ctx, edit| {
                let v: Vec<_> = [
                    DVec3::new(x, 0.0, 0.0),
                    DVec3::new(x + 1.0, 0.0, 0.0),
                    DVec3::new(x, 1.0, 0.0),
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
                .unwrap();
            })
        }
        Op::EraseFace { body, pick } => {
            let bs = bodies(m);
            let b = bs[body % bs.len()];
            let faces: Vec<_> = m
                .body(b)
                .unwrap()
                .geometry()
                .faces()
                .map(|(f, _)| f)
                .collect();
            if faces.is_empty() {
                return Ok(());
            }
            let f = faces[pick % faces.len()];
            m.edit_geometry(b, log, |ctx, edit| ctx.delete_face(f, edit).unwrap())
        }
    }
}

/// References resolve, there are no cycles, and `instances_of` agrees with a scan.
fn consistent(m: &Model) -> Result<(), TestCaseError> {
    let materials: BTreeSet<_> = m.materials().map(|(id, _)| id).collect();
    let defs: BTreeSet<_> = m.definitions().map(|(id, _)| id).collect();
    let all = all_instances(m);
    let mut ids = BTreeSet::new();
    for (_, id, inst) in &all {
        prop_assert!(ids.insert(*id), "instance id {:?} used twice", id);
        prop_assert!(defs.contains(&inst.definition));
        if let Some(mat) = inst.material {
            prop_assert!(materials.contains(&mat));
        }
    }
    for b in bodies(m) {
        for (_, fm) in m.body(b).unwrap().face_material_entries() {
            prop_assert!(fm != FaceMaterials::default());
            for mat in [fm.front, fm.back].into_iter().flatten() {
                prop_assert!(materials.contains(&mat));
            }
        }
    }
    for d in &defs {
        // Walk everything reachable from d's body; d must not be reachable.
        let mut stack = vec![*d];
        let mut seen = BTreeSet::new();
        while let Some(x) = stack.pop() {
            for (_, inst) in m.body(BodyId::Definition(x)).unwrap().instances() {
                prop_assert!(inst.definition != *d, "cycle through {:?}", d);
                if seen.insert(inst.definition) {
                    stack.push(inst.definition);
                }
            }
        }
        let scan: Vec<_> = all
            .iter()
            .filter(|(_, _, i)| i.definition == *d)
            .map(|(b, id, _)| (*b, *id))
            .collect();
        prop_assert_eq!(m.instances_of(*d), scan);
    }
    for v in m.validate() {
        prop_assert!(matches!(v, ModelViolation::GroupInstances(..)), "{:?}", v);
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn random_operations_stay_consistent(ops in prop::collection::vec(op(), 1..40)) {
        let mut m = Model::new();
        let mut stack = UndoStack::new();
        let mut draws = 0;
        for (k, op) in ops.iter().enumerate() {
            let before = m.clone();
            let mut log = ModelEdit::new();
            match run_op(&mut m, op, &mut draws, &mut log) {
                Ok(()) => {
                    let mut undone = m.clone();
                    prop_assert_eq!(undone.apply_edit(&log.inverse()), Ok(()));
                    prop_assert_eq!(&undone, &before);
                    let mut redone = before.clone();
                    prop_assert_eq!(redone.apply_edit(&log), Ok(()));
                    prop_assert_eq!(&redone, &m);
                }
                Err(_) => {
                    prop_assert_eq!(&m, &before);
                    prop_assert!(log.is_empty());
                }
            }
            consistent(&m)?;
            stack.push(&format!("op {k}"), log);
        }
        let end = m.clone();
        while let Some(label) = stack.undo(&mut m).map_err(|e| TestCaseError::fail(format!("{e:?}")))? {
            prop_assert!(label.starts_with("op "));
        }
        prop_assert_eq!(&m, &Model::new());
        while stack.redo(&mut m).map_err(|e| TestCaseError::fail(format!("{e:?}")))?.is_some() {}
        prop_assert_eq!(&m, &end);
    }
}
