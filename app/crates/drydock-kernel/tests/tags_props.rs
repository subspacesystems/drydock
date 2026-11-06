// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Tag invariants under random edits (`docs/specs/m10-tags-kernel.md`): the file round
//! trip, undo and redo, and no assignment ever names a missing tag or instance.

use drydock_kernel::format::{from_text, to_text};
use drydock_kernel::model::*;
use drydock_kernel::tags::*;
use glam::{DMat4, DVec3};
use proptest::prelude::*;

fn name() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => "[a-zA-Z0-9 _.#|+-]{0,10}",
        1 => any::<String>(),
        1 => prop::collection::vec(
            prop::sample::select(vec!['"', '\\', '\n', '\r', '\t', '\u{0}', '\u{7f}', ' ', 'é', '\u{1F600}', 'x']),
            0..8
        )
        .prop_map(|v| v.into_iter().collect()),
    ]
}

#[derive(Clone, Debug)]
enum Op {
    AddDefinition(bool),
    AddInstance { body: usize, def: usize, x: f64 },
    RemoveInstance(usize),
    AddTag(String),
    RenameTag(usize, String),
    ShowTag(usize, bool),
    RemoveTag(usize),
    Assign(usize, Option<usize>),
}

fn op() -> impl Strategy<Value = Op> {
    let ix = 0usize..9;
    prop_oneof![
        2 => any::<bool>().prop_map(Op::AddDefinition),
        4 => (ix.clone(), ix.clone(), -5.0f64..5.0)
            .prop_map(|(body, def, x)| Op::AddInstance { body, def, x }),
        1 => ix.clone().prop_map(Op::RemoveInstance),
        3 => name().prop_map(Op::AddTag),
        1 => (ix.clone(), name()).prop_map(|(t, n)| Op::RenameTag(t, n)),
        2 => (ix.clone(), any::<bool>()).prop_map(|(t, v)| Op::ShowTag(t, v)),
        1 => ix.clone().prop_map(Op::RemoveTag),
        5 => (ix.clone(), prop::option::of(ix)).prop_map(|(i, t)| Op::Assign(i, t)),
    ]
}

fn bodies(m: &Model) -> Vec<BodyId> {
    std::iter::once(BodyId::Root)
        .chain(m.definitions().map(|(d, _)| BodyId::Definition(d)))
        .collect()
}

fn all_instances(m: &Model) -> Vec<(BodyId, InstanceId)> {
    bodies(m)
        .into_iter()
        .flat_map(|b| {
            m.body(b)
                .unwrap()
                .instances()
                .map(|(id, _)| (b, id))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The i-th existing item (mod count), or `missing` when there are none (so the operation's
/// error path runs too).
fn pick<T: Copy>(all: &[T], i: usize, missing: T) -> T {
    if all.is_empty() {
        missing
    } else {
        all[i % all.len()]
    }
}

fn run_op(m: &mut Model, op: &Op, log: &mut ModelEdit) -> Result<(), ModelError> {
    let tags: Vec<TagId> = m.tags().iter().map(|(id, _)| id).collect();
    let tag = |i: usize| pick(&tags, i, TagId(1000));
    let instances = all_instances(m);
    let defs: Vec<DefinitionId> = m.definitions().map(|(d, _)| d).collect();
    match op {
        Op::AddDefinition(group) => {
            let n = defs.len();
            m.add_definition(
                DefinitionInfo {
                    name: format!("d{n}"),
                    group: *group,
                },
                log,
            );
            Ok(())
        }
        Op::AddInstance { body, def, x } => {
            let bodies = bodies(m);
            let body = bodies[body % bodies.len()];
            let def = pick(&defs, *def, DefinitionId(1000));
            m.add_instance(
                body,
                Instance {
                    definition: def,
                    transform: DMat4::from_translation(DVec3::new(*x, 0.0, 0.0)),
                    name: String::new(),
                    material: None,
                },
                log,
            )
            .map(|_| ())
        }
        Op::RemoveInstance(i) => {
            let (body, id) = pick(&instances, *i, (BodyId::Root, InstanceId(1000)));
            m.remove_instance(body, id, log)
        }
        Op::AddTag(name) => m.add_tag(name, log).map(|_| ()),
        Op::RenameTag(t, name) => m.rename_tag(tag(*t), name, log),
        Op::ShowTag(t, visible) => m.set_tag_visible(tag(*t), *visible, log),
        Op::RemoveTag(t) => m.remove_tag(tag(*t), log),
        Op::Assign(i, t) => {
            let (_, id) = pick(&instances, *i, (BodyId::Root, InstanceId(1000)));
            m.assign_tag(id, t.map(tag), log)
        }
    }
}

/// Every assignment names an existing instance and an existing tag, and an instance is
/// hidden exactly when its tag is hidden.
fn consistent(m: &Model) -> Result<(), TestCaseError> {
    let instances: Vec<InstanceId> = all_instances(m).into_iter().map(|(_, id)| id).collect();
    let mut last = None;
    for (instance, tag) in m.tags().assignments() {
        prop_assert!(last < Some(instance), "assignments ascend");
        last = Some(instance);
        prop_assert!(
            instances.contains(&instance),
            "{instance:?} is gone but tagged"
        );
        prop_assert!(m.tag(tag).is_some(), "{instance:?} carries missing {tag:?}");
        prop_assert!(m.tags().instances_with(tag).contains(&instance));
    }
    for (id, tag) in m.tags().iter() {
        prop_assert!(tag_name_is_good(&tag.name), "{id:?} has a bad name");
    }
    for instance in instances {
        let hidden = m
            .instance_tag(instance)
            .is_some_and(|t| !m.tag(t).unwrap().visible);
        prop_assert_eq!(m.instance_visible(instance), !hidden);
    }
    Ok(())
}

/// Runs the ops, checking after each one that a failed op changed nothing and that the
/// model stays consistent. Returns the snapshots (start, then after each successful op) and
/// the undo stack.
fn play(ops: &[Op]) -> Result<(Model, Vec<Model>, UndoStack), TestCaseError> {
    let mut m = Model::new();
    let mut snapshots = vec![m.clone()];
    let mut stack = UndoStack::new();
    for op in ops {
        let before = m.clone();
        let mut log = ModelEdit::new();
        match run_op(&mut m, op, &mut log) {
            Ok(()) => {
                if !log.is_empty() {
                    snapshots.push(m.clone());
                    stack.push("op", log);
                } else {
                    prop_assert_eq!(&m, &before, "{:?} recorded nothing but changed", op);
                }
            }
            Err(error) => {
                prop_assert_eq!(&m, &before, "{:?} failed with {:?} but changed", op, error);
                prop_assert!(log.is_empty(), "{:?} failed but logged", op);
            }
        }
        consistent(&m)?;
    }
    Ok((m, snapshots, stack))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn tags_survive_save_and_load(ops in prop::collection::vec(op(), 0..40)) {
        let (m, _, _) = play(&ops)?;
        let text = to_text(&m).unwrap();
        let back = from_text(&text).unwrap();
        prop_assert_eq!(&back, &m);
        prop_assert_eq!(to_text(&back).unwrap(), text);
        consistent(&back)?;
    }

    #[test]
    fn undo_and_redo_restore_tags(ops in prop::collection::vec(op(), 0..40)) {
        let (mut m, snapshots, mut stack) = play(&ops)?;
        let end = m.clone();
        for expected in snapshots.iter().rev().skip(1) {
            prop_assert!(stack.undo(&mut m).unwrap().is_some());
            prop_assert_eq!(&m, expected);
            consistent(&m)?;
        }
        prop_assert!(stack.undo(&mut m).unwrap().is_none());
        for expected in snapshots.iter().skip(1) {
            prop_assert!(stack.redo(&mut m).unwrap().is_some());
            prop_assert_eq!(&m, expected);
        }
        prop_assert_eq!(&m, &end);
    }

    #[test]
    fn remove_tag_leaves_no_dangling_reference(
        ops in prop::collection::vec(op(), 0..40),
        which in 0usize..9,
    ) {
        let (mut m, _, _) = play(&ops)?;
        let tags: Vec<TagId> = m.tags().iter().map(|(id, _)| id).collect();
        prop_assume!(!tags.is_empty());
        let t = tags[which % tags.len()];
        let carriers = m.tags().instances_with(t);
        let others: Vec<_> = m.tags().assignments().filter(|(_, tag)| *tag != t).collect();
        let before = m.clone();
        let mut log = ModelEdit::new();
        m.remove_tag(t, &mut log).unwrap();
        prop_assert!(m.tag(t).is_none());
        prop_assert!(m.tags().instances_with(t).is_empty());
        prop_assert!(m.tags().assignments().all(|(_, tag)| tag != t));
        prop_assert_eq!(m.tags().assignments().collect::<Vec<_>>(), others);
        for instance in carriers {
            prop_assert!(m.instance_visible(instance));
        }
        consistent(&m)?;
        let mut undone = m.clone();
        undone.apply_edit(&log.inverse()).unwrap();
        prop_assert_eq!(&undone, &before);
        let text = to_text(&m).unwrap();
        prop_assert_eq!(&from_text(&text).unwrap(), &m);
    }
}
