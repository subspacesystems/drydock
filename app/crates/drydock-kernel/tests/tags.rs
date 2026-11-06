// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Tags in the model (`docs/specs/m10-tags-kernel.md`).

use drydock_kernel::model::*;
use drydock_kernel::tags::*;
use glam::{DMat4, DVec3};

fn info(name: &str, group: bool) -> DefinitionInfo {
    DefinitionInfo {
        name: name.to_string(),
        group,
    }
}

fn inst(d: DefinitionId, x: f64) -> Instance {
    Instance {
        definition: d,
        transform: DMat4::from_translation(DVec3::new(x, 0.0, 0.0)),
        name: String::new(),
        material: None,
    }
}

/// A group `g` placed once in the root, a component `c` placed twice in the root and once
/// inside the group's body.
struct Scene {
    model: Model,
    group: InstanceId,
    comp_a: InstanceId,
    comp_b: InstanceId,
    /// The component instance inside the group's body.
    nested: InstanceId,
    group_def: DefinitionId,
}

fn scene() -> Scene {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let group_def = model.add_definition(info("Group", true), &mut log);
    let comp_def = model.add_definition(info("Chair", false), &mut log);
    let group = model
        .add_instance(BodyId::Root, inst(group_def, 0.0), &mut log)
        .unwrap();
    let comp_a = model
        .add_instance(BodyId::Root, inst(comp_def, 1.0), &mut log)
        .unwrap();
    let comp_b = model
        .add_instance(BodyId::Root, inst(comp_def, 2.0), &mut log)
        .unwrap();
    let nested = model
        .add_instance(BodyId::Definition(group_def), inst(comp_def, 3.0), &mut log)
        .unwrap();
    Scene {
        model,
        group,
        comp_a,
        comp_b,
        nested,
        group_def,
    }
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

fn tag_record(log: &ModelEdit, i: usize) -> &TagRecord {
    match &log.records[i] {
        ModelRecord::Tag(r) => r,
        other => panic!("record {i} is not a tag record: {other:?}"),
    }
}

// -- adding --------------------------------------------------------------------------------

#[test]
fn a_new_model_has_no_tags() {
    let m = Model::new();
    assert!(m.tags().is_empty());
    assert_eq!(m.tags().len(), 0);
    assert_eq!(m.tags().iter().count(), 0);
    assert_eq!(m.tags().assignments().count(), 0);
    assert_eq!(m.tag(TagId(0)), None);
}

#[test]
fn add_tag_makes_a_visible_tag_with_one_record() {
    let mut m = Model::new();
    let before = m.clone();
    let mut log = ModelEdit::new();
    let id = m.add_tag("Walls", &mut log).unwrap();
    assert_eq!(m.tag(id), Some(&Tag::new("Walls")));
    assert!(m.tag(id).unwrap().visible);
    assert_eq!(log.records.len(), 1);
    assert_eq!(
        tag_record(&log, 0),
        &TagRecord::AddTag(id, Tag::new("Walls"))
    );
    assert_undo_redo(&before, &m, &log);
}

#[test]
fn tag_ids_are_distinct_and_listed_in_ascending_order() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_tag("A", &mut log).unwrap();
    let b = m.add_tag("B", &mut log).unwrap();
    let c = m.add_tag("C", &mut log).unwrap();
    assert!(a < b && b < c);
    let listed: Vec<_> = m
        .tags()
        .iter()
        .map(|(id, t)| (id, t.name.clone()))
        .collect();
    assert_eq!(
        listed,
        vec![
            (a, "A".to_string()),
            (b, "B".to_string()),
            (c, "C".to_string())
        ]
    );
    assert_eq!(m.tags().len(), 3);
}

#[test]
fn two_tags_may_share_a_name() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_tag("Same", &mut log).unwrap();
    let b = m.add_tag("Same", &mut log).unwrap();
    assert_ne!(a, b);
    assert_eq!(m.tags().len(), 2);
}

#[test]
fn names_that_are_empty_or_blank_are_refused() {
    for name in ["", " ", "   ", "\t", "\n \r"] {
        let mut m = Model::new();
        let mut log = ModelEdit::new();
        assert_eq!(
            m.add_tag(name, &mut log),
            Err(ModelError::BadTagName),
            "{name:?}"
        );
        assert!(log.is_empty());
        assert!(m.tags().is_empty());
    }
    assert!(!tag_name_is_good(""));
    assert!(!tag_name_is_good(" \t"));
    assert!(tag_name_is_good(" x "));
    assert!(tag_name_is_good("Layer \"1\"\n"));
}

#[test]
fn tag_ids_are_never_handed_out_twice() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_tag("A", &mut log).unwrap();
    m.remove_tag(a, &mut log).unwrap();
    let b = m.add_tag("B", &mut log).unwrap();
    assert_ne!(a, b);

    // undoing an add does not give its id back either
    let mut add = ModelEdit::new();
    let c = m.add_tag("C", &mut add).unwrap();
    m.apply_edit(&add.inverse()).unwrap();
    assert_eq!(m.tag(c), None);
    let d = m.add_tag("D", &mut log).unwrap();
    assert!(d != a && d != b && d != c);
}

#[test]
fn an_added_record_moves_the_id_counter_past_it() {
    let mut m = Model::new();
    m.apply(&ModelRecord::Tag(TagRecord::AddTag(
        TagId(10),
        Tag::new("Ten"),
    )))
    .unwrap();
    let mut log = ModelEdit::new();
    let next = m.add_tag("Next", &mut log).unwrap();
    assert!(next.0 > 10, "{next:?}");
}

// -- renaming and showing ------------------------------------------------------------------

#[test]
fn rename_tag_keeps_visibility_and_is_one_record() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let id = m.add_tag("Old", &mut log).unwrap();
    m.set_tag_visible(id, false, &mut log).unwrap();
    let before = m.clone();
    let mut log = ModelEdit::new();
    m.rename_tag(id, "New", &mut log).unwrap();
    assert_eq!(
        m.tag(id),
        Some(&Tag {
            name: "New".into(),
            visible: false
        })
    );
    assert_eq!(log.records.len(), 1);
    assert_eq!(
        tag_record(&log, 0),
        &TagRecord::SetTag {
            id,
            from: Tag {
                name: "Old".into(),
                visible: false
            },
            to: Tag {
                name: "New".into(),
                visible: false
            },
        }
    );
    assert_undo_redo(&before, &m, &log);
}

#[test]
fn rename_to_the_same_name_records_nothing() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let id = m.add_tag("Same", &mut log).unwrap();
    let mut log = ModelEdit::new();
    m.rename_tag(id, "Same", &mut log).unwrap();
    assert!(log.is_empty());
}

#[test]
fn rename_errors_change_nothing() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let id = m.add_tag("Keep", &mut log).unwrap();
    let before = m.clone();
    let mut log = ModelEdit::new();
    assert_eq!(
        m.rename_tag(id, "  ", &mut log),
        Err(ModelError::BadTagName)
    );
    assert_eq!(
        m.rename_tag(TagId(99), "X", &mut log),
        Err(ModelError::MissingTag(TagId(99)))
    );
    assert!(log.is_empty());
    assert_eq!(m, before);
}

#[test]
fn set_tag_visible_toggles_with_one_record() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let id = m.add_tag("T", &mut log).unwrap();
    let before = m.clone();
    let mut log = ModelEdit::new();
    m.set_tag_visible(id, false, &mut log).unwrap();
    assert!(!m.tag(id).unwrap().visible);
    assert_eq!(m.tag(id).unwrap().name, "T");
    assert_eq!(log.records.len(), 1);
    assert!(matches!(
        tag_record(&log, 0),
        TagRecord::SetTag { id: i, from, to } if *i == id && from.visible && !to.visible
    ));
    assert_undo_redo(&before, &m, &log);

    let mut again = ModelEdit::new();
    m.set_tag_visible(id, false, &mut again).unwrap();
    assert!(again.is_empty(), "unchanged visibility records nothing");
    m.set_tag_visible(id, true, &mut again).unwrap();
    assert!(m.tag(id).unwrap().visible);
    assert_eq!(
        m.set_tag_visible(TagId(7), true, &mut again),
        Err(ModelError::MissingTag(TagId(7)))
    );
}

#[test]
fn every_tag_change_moves_the_revision() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let mut last = m.revision();
    let mut moved = |m: &Model| {
        let r = m.revision();
        assert_ne!(r, last);
        last = r;
    };
    let d = m.add_definition(info("G", true), &mut log);
    let i = m
        .add_instance(BodyId::Root, inst(d, 0.0), &mut log)
        .unwrap();
    moved(&m);
    let t = m.add_tag("T", &mut log).unwrap();
    moved(&m);
    m.set_tag_visible(t, false, &mut log).unwrap();
    moved(&m);
    m.rename_tag(t, "U", &mut log).unwrap();
    moved(&m);
    m.assign_tag(i, Some(t), &mut log).unwrap();
    moved(&m);
    m.remove_tag(t, &mut log).unwrap();
    moved(&m);
}

// -- assigning -----------------------------------------------------------------------------

#[test]
fn assign_tag_works_on_instances_in_any_body() {
    let mut s = scene();
    let mut log = ModelEdit::new();
    let t = s.model.add_tag("Furniture", &mut log).unwrap();
    for instance in [s.group, s.comp_a, s.nested] {
        let before = s.model.clone();
        let mut log = ModelEdit::new();
        s.model.assign_tag(instance, Some(t), &mut log).unwrap();
        assert_eq!(s.model.instance_tag(instance), Some(t));
        assert_eq!(s.model.tags().instance_tag(instance), Some(t));
        assert_eq!(log.records.len(), 1);
        assert_eq!(
            tag_record(&log, 0),
            &TagRecord::SetInstanceTag {
                instance,
                from: None,
                to: Some(t)
            }
        );
        assert_undo_redo(&before, &s.model, &log);
    }
    assert_eq!(s.model.instance_tag(s.comp_b), None);
    assert_eq!(
        s.model.tags().assignments().collect::<Vec<_>>(),
        vec![(s.group, t), (s.comp_a, t), (s.nested, t)]
    );
    assert_eq!(
        s.model.tags().instances_with(t),
        vec![s.group, s.comp_a, s.nested]
    );
}

#[test]
fn an_instance_carries_one_tag_and_can_be_moved_or_untagged() {
    let mut s = scene();
    let mut log = ModelEdit::new();
    let a = s.model.add_tag("A", &mut log).unwrap();
    let b = s.model.add_tag("B", &mut log).unwrap();
    s.model.assign_tag(s.comp_a, Some(a), &mut log).unwrap();

    let before = s.model.clone();
    let mut log = ModelEdit::new();
    s.model.assign_tag(s.comp_a, Some(b), &mut log).unwrap();
    assert_eq!(s.model.instance_tag(s.comp_a), Some(b));
    assert!(s.model.tags().instances_with(a).is_empty());
    assert_eq!(
        tag_record(&log, 0),
        &TagRecord::SetInstanceTag {
            instance: s.comp_a,
            from: Some(a),
            to: Some(b)
        }
    );
    assert_undo_redo(&before, &s.model, &log);

    let mut log = ModelEdit::new();
    s.model.assign_tag(s.comp_a, Some(b), &mut log).unwrap();
    assert!(log.is_empty(), "same tag again records nothing");
    s.model.assign_tag(s.comp_a, None, &mut log).unwrap();
    assert_eq!(s.model.instance_tag(s.comp_a), None);
    assert_eq!(log.records.len(), 1);
    let mut log = ModelEdit::new();
    s.model.assign_tag(s.comp_a, None, &mut log).unwrap();
    assert!(
        log.is_empty(),
        "untagging an untagged instance records nothing"
    );
}

#[test]
fn assign_errors_change_nothing() {
    let mut s = scene();
    let mut log = ModelEdit::new();
    let t = s.model.add_tag("T", &mut log).unwrap();
    let before = s.model.clone();
    let mut log = ModelEdit::new();
    assert_eq!(
        s.model.assign_tag(InstanceId(500), Some(t), &mut log),
        Err(ModelError::UnknownInstance(InstanceId(500)))
    );
    assert_eq!(
        s.model
            .assign_tag(InstanceId(500), Some(TagId(9)), &mut log),
        Err(ModelError::UnknownInstance(InstanceId(500))),
        "the instance is checked before the tag"
    );
    assert_eq!(
        s.model.assign_tag(s.group, Some(TagId(9)), &mut log),
        Err(ModelError::MissingTag(TagId(9)))
    );
    assert!(log.is_empty());
    assert_eq!(s.model, before);
}

#[test]
fn instance_visible_follows_the_tag() {
    let mut s = scene();
    let mut log = ModelEdit::new();
    let shown = s.model.add_tag("Shown", &mut log).unwrap();
    let hidden = s.model.add_tag("Hidden", &mut log).unwrap();
    s.model.set_tag_visible(hidden, false, &mut log).unwrap();
    s.model.assign_tag(s.comp_a, Some(shown), &mut log).unwrap();
    s.model
        .assign_tag(s.comp_b, Some(hidden), &mut log)
        .unwrap();
    s.model
        .assign_tag(s.nested, Some(hidden), &mut log)
        .unwrap();

    assert!(s.model.instance_visible(s.group), "untagged");
    assert!(s.model.instance_visible(s.comp_a), "visible tag");
    assert!(!s.model.instance_visible(s.comp_b), "hidden tag");
    assert!(!s.model.instance_visible(s.nested), "hidden tag, nested");
    assert!(
        s.model.instance_visible(InstanceId(999)),
        "unknown instance"
    );

    s.model.set_tag_visible(hidden, true, &mut log).unwrap();
    assert!(s.model.instance_visible(s.comp_b));
    s.model.set_tag_visible(shown, false, &mut log).unwrap();
    assert!(!s.model.instance_visible(s.comp_a));
    s.model.assign_tag(s.comp_a, None, &mut log).unwrap();
    assert!(s.model.instance_visible(s.comp_a));
}

#[test]
fn a_hidden_tag_does_not_hide_the_instances_inside() {
    // instance_visible looks only at the instance's own tag
    let mut s = scene();
    let mut log = ModelEdit::new();
    let t = s.model.add_tag("Hidden", &mut log).unwrap();
    s.model.set_tag_visible(t, false, &mut log).unwrap();
    s.model.assign_tag(s.group, Some(t), &mut log).unwrap();
    assert!(!s.model.instance_visible(s.group));
    assert!(s.model.instance_visible(s.nested));
}

// -- removing ------------------------------------------------------------------------------

#[test]
fn remove_tag_untags_its_instances_first() {
    let mut s = scene();
    let mut log = ModelEdit::new();
    let gone = s.model.add_tag("Gone", &mut log).unwrap();
    let kept = s.model.add_tag("Kept", &mut log).unwrap();
    s.model.set_tag_visible(gone, false, &mut log).unwrap();
    s.model.assign_tag(s.nested, Some(gone), &mut log).unwrap();
    s.model.assign_tag(s.group, Some(gone), &mut log).unwrap();
    s.model.assign_tag(s.comp_a, Some(kept), &mut log).unwrap();

    let before = s.model.clone();
    let mut log = ModelEdit::new();
    s.model.remove_tag(gone, &mut log).unwrap();
    assert_eq!(s.model.tag(gone), None);
    assert_eq!(s.model.instance_tag(s.group), None);
    assert_eq!(s.model.instance_tag(s.nested), None);
    assert_eq!(s.model.instance_tag(s.comp_a), Some(kept));
    assert!(s.model.instance_visible(s.group));

    // untag in ascending instance id, then remove the tag
    let mut carriers = vec![s.group, s.nested];
    carriers.sort();
    assert_eq!(log.records.len(), 3);
    for (i, instance) in carriers.into_iter().enumerate() {
        assert_eq!(
            tag_record(&log, i),
            &TagRecord::SetInstanceTag {
                instance,
                from: Some(gone),
                to: None
            }
        );
    }
    assert_eq!(
        tag_record(&log, 2),
        &TagRecord::RemoveTag(
            gone,
            Tag {
                name: "Gone".into(),
                visible: false
            }
        )
    );
    assert_undo_redo(&before, &s.model, &log);
}

#[test]
fn remove_a_missing_tag_is_an_error() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    assert_eq!(
        m.remove_tag(TagId(3), &mut log),
        Err(ModelError::MissingTag(TagId(3)))
    );
    assert!(log.is_empty());
}

#[test]
fn removing_a_tagged_instance_untags_it_first() {
    let mut s = scene();
    let mut log = ModelEdit::new();
    let t = s.model.add_tag("T", &mut log).unwrap();
    s.model.assign_tag(s.nested, Some(t), &mut log).unwrap();

    let before = s.model.clone();
    let mut log = ModelEdit::new();
    s.model
        .remove_instance(BodyId::Definition(s.group_def), s.nested, &mut log)
        .unwrap();
    assert_eq!(s.model.instance_tag(s.nested), None);
    assert!(s.model.tags().instances_with(t).is_empty());
    assert_eq!(log.records.len(), 2);
    assert_eq!(
        tag_record(&log, 0),
        &TagRecord::SetInstanceTag {
            instance: s.nested,
            from: Some(t),
            to: None
        }
    );
    assert!(matches!(
        &log.records[1],
        ModelRecord::RemoveInstance(_, id, _) if *id == s.nested
    ));
    assert_undo_redo(&before, &s.model, &log);
    // the tag itself stays
    assert!(s.model.tag(t).is_some());
}

// -- records -------------------------------------------------------------------------------

#[test]
fn records_that_would_break_the_tags_are_refused() {
    let mut s = scene();
    let mut log = ModelEdit::new();
    let t = s.model.add_tag("T", &mut log).unwrap();
    s.model.assign_tag(s.comp_a, Some(t), &mut log).unwrap();
    let before = s.model.clone();
    let comp = s
        .model
        .root()
        .instance(s.comp_a)
        .cloned()
        .expect("comp_a is in the root");
    let cases: Vec<(ModelRecord, ModelError)> = vec![
        (
            ModelRecord::RemoveInstance(BodyId::Root, s.comp_a, comp),
            ModelError::InstanceTagged(s.comp_a),
        ),
        (
            ModelRecord::Tag(TagRecord::RemoveTag(t, Tag::new("T"))),
            ModelError::TagInUse(t),
        ),
        (
            ModelRecord::Tag(TagRecord::RemoveTag(t, Tag::new("Other"))),
            ModelError::Mismatch,
        ),
        (
            ModelRecord::Tag(TagRecord::RemoveTag(TagId(40), Tag::new("T"))),
            ModelError::MissingTag(TagId(40)),
        ),
        (
            ModelRecord::Tag(TagRecord::AddTag(t, Tag::new("Again"))),
            ModelError::IdInUse,
        ),
        (
            ModelRecord::Tag(TagRecord::AddTag(TagId(41), Tag::new(" "))),
            ModelError::BadTagName,
        ),
        (
            ModelRecord::Tag(TagRecord::SetTag {
                id: t,
                from: Tag::new("Wrong"),
                to: Tag::new("X"),
            }),
            ModelError::Mismatch,
        ),
        (
            ModelRecord::Tag(TagRecord::SetTag {
                id: t,
                from: Tag::new("T"),
                to: Tag::new(""),
            }),
            ModelError::BadTagName,
        ),
        (
            ModelRecord::Tag(TagRecord::SetTag {
                id: TagId(42),
                from: Tag::new("T"),
                to: Tag::new("X"),
            }),
            ModelError::MissingTag(TagId(42)),
        ),
        (
            ModelRecord::Tag(TagRecord::SetInstanceTag {
                instance: s.comp_a,
                from: None,
                to: None,
            }),
            ModelError::Mismatch,
        ),
        (
            ModelRecord::Tag(TagRecord::SetInstanceTag {
                instance: s.comp_b,
                from: None,
                to: Some(TagId(43)),
            }),
            ModelError::MissingTag(TagId(43)),
        ),
        (
            ModelRecord::Tag(TagRecord::SetInstanceTag {
                instance: InstanceId(700),
                from: None,
                to: Some(t),
            }),
            ModelError::UnknownInstance(InstanceId(700)),
        ),
    ];
    for (record, error) in cases {
        assert_eq!(s.model.apply(&record), Err(error), "{record:?}");
        assert_eq!(s.model, before, "{record:?} changed the model");
    }
}

#[test]
fn equality_and_clone_see_tags() {
    let s = scene();
    let mut m = s.model.clone();
    let mut log = ModelEdit::new();
    let t = m.add_tag("T", &mut log).unwrap();
    assert_ne!(m, s.model, "a tag makes models differ");
    let copy = m.clone();
    assert_eq!(copy, m);
    assert_eq!(copy.tag(t), Some(&Tag::new("T")));

    let mut a = m.clone();
    a.assign_tag(s.group, Some(t), &mut log).unwrap();
    assert_ne!(a, m, "an assignment makes models differ");
    let mut h = m.clone();
    h.set_tag_visible(t, false, &mut log).unwrap();
    assert_ne!(h, m, "visibility makes models differ");
}

#[test]
fn undo_stack_restores_tags() {
    let mut s = scene();
    let mut stack = UndoStack::new();
    let start = s.model.clone();

    let mut log = ModelEdit::new();
    let t = s.model.add_tag("T", &mut log).unwrap();
    stack.push("Add Tag", log);
    let mut log = ModelEdit::new();
    s.model.assign_tag(s.comp_b, Some(t), &mut log).unwrap();
    stack.push("Assign Tag", log);
    let mut log = ModelEdit::new();
    s.model.set_tag_visible(t, false, &mut log).unwrap();
    stack.push("Hide Tag", log);
    let mut log = ModelEdit::new();
    s.model.remove_tag(t, &mut log).unwrap();
    stack.push("Delete Tag", log);
    let end = s.model.clone();

    stack.undo(&mut s.model).unwrap();
    assert_eq!(s.model.instance_tag(s.comp_b), Some(t));
    assert!(!s.model.instance_visible(s.comp_b));
    while stack.undo(&mut s.model).unwrap().is_some() {}
    assert_eq!(s.model, start);
    while stack.redo(&mut s.model).unwrap().is_some() {}
    assert_eq!(s.model, end);
}
