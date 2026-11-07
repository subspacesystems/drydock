// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Tags in the native file, version 7 (`docs/specs/m10-tags-kernel.md`, `docs/format.md`).

use drydock_kernel::format::*;
use drydock_kernel::model::*;
use drydock_kernel::tags::*;
use glam::DMat4;

const IDENTITY: &str = "1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1";

fn info(name: &str, group: bool) -> DefinitionInfo {
    DefinitionInfo {
        name: name.to_string(),
        group,
    }
}

fn inst(d: DefinitionId) -> Instance {
    Instance {
        definition: d,
        transform: DMat4::IDENTITY,
        name: String::new(),
        material: None,
    }
}

fn kind_at(text: &str, line: usize, kind: ReadErrorKind) {
    assert_eq!(
        from_text(text).map(|_| ()),
        Err(ReadError { line, kind }),
        "{text}"
    );
}

/// A group in the root tagged with a hidden tag, a second visible tag carried by nothing.
fn small() -> Model {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let d = m.add_definition(info("Box", true), &mut log);
    let i = m.add_instance(BodyId::Root, inst(d), &mut log).unwrap();
    let walls = m.add_tag("Walls", &mut log).unwrap();
    let hidden = m.add_tag("Hidden \"one\"", &mut log).unwrap();
    assert_eq!(
        (d, i, walls, hidden),
        (DefinitionId(0), InstanceId(0), TagId(0), TagId(1))
    );
    m.set_tag_visible(hidden, false, &mut log).unwrap();
    m.assign_tag(i, Some(hidden), &mut log).unwrap();
    m
}

#[test]
fn the_format_version_holds_tags() {
    // version 7 added tags; m10-background-kernel made it 8 (tests/background.rs)
    const { assert!(FORMAT_VERSION >= 7) };
}

#[test]
fn writes_tags_after_definitions_and_tagged_after_the_bodies() {
    let text = to_text(&small()).unwrap();
    let expected = format!(
        "drydock-model 7\n\
         definition 0 group \"Box\"\n\
         tag 0 on \"Walls\"\n\
         tag 1 off \"Hidden \\\"one\\\"\"\n\
         body root\n\
         instance 0 0 - {IDENTITY} \"\"\n\
         body 0\n\
         tagged 0 1\n\
         end\n"
    );
    assert_eq!(text, expected);
}

#[test]
fn round_trips_exactly() {
    let m = small();
    let text = to_text(&m).unwrap();
    let back = from_text(&text).unwrap();
    assert_eq!(back, m);
    assert_eq!(back.tag(TagId(1)).map(|t| t.visible), Some(false));
    assert_eq!(back.instance_tag(InstanceId(0)), Some(TagId(1)));
    assert!(!back.instance_visible(InstanceId(0)));
    assert_eq!(to_text(&back).unwrap(), text);
}

#[test]
fn a_tag_alone_needs_version_seven() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    m.add_tag("Lonely", &mut log).unwrap();
    assert_eq!(
        to_text(&m).unwrap(),
        "drydock-model 7\ntag 0 on \"Lonely\"\nbody root\nend\n"
    );
}

#[test]
fn a_model_without_tags_keeps_its_old_version() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let d = m.add_definition(info("Box", false), &mut log);
    m.add_instance(BodyId::Root, inst(d), &mut log).unwrap();
    assert!(to_text(&m).unwrap().starts_with("drydock-model 1\n"));

    // a tag added and removed again leaves nothing behind
    let t = m.add_tag("Gone", &mut log).unwrap();
    m.assign_tag(InstanceId(0), Some(t), &mut log).unwrap();
    m.remove_tag(t, &mut log).unwrap();
    let text = to_text(&m).unwrap();
    assert!(text.starts_with("drydock-model 1\n"), "{text}");
    assert!(!text.contains("tag"), "{text}");
}

#[test]
fn nested_instances_round_trip_with_their_tags() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let outer = m.add_definition(info("Outer", true), &mut log);
    let inner = m.add_definition(info("Inner", false), &mut log);
    let a = m.add_instance(BodyId::Root, inst(outer), &mut log).unwrap();
    let b = m
        .add_instance(BodyId::Definition(outer), inst(inner), &mut log)
        .unwrap();
    let c = m
        .add_instance(BodyId::Definition(outer), inst(inner), &mut log)
        .unwrap();
    let t0 = m.add_tag("Zero", &mut log).unwrap();
    let t1 = m.add_tag("One", &mut log).unwrap();
    m.assign_tag(c, Some(t0), &mut log).unwrap();
    m.assign_tag(b, Some(t1), &mut log).unwrap();
    m.assign_tag(a, Some(t1), &mut log).unwrap();
    let text = to_text(&m).unwrap();
    // tagged lines come last, in ascending instance id
    let tail: Vec<String> = text.lines().rev().take(4).map(String::from).collect();
    assert_eq!(
        tail,
        vec![
            "end".to_string(),
            format!("tagged {} {}", c.0, t0.0),
            format!("tagged {} {}", b.0, t1.0),
            format!("tagged {} {}", a.0, t1.0),
        ]
    );
    assert_eq!(from_text(&text).unwrap(), m);
}

#[test]
fn older_versions_load_untagged() {
    for v in 1..=6 {
        let text = format!(
            "drydock-model {v}\ndefinition 0 group \"G\"\nbody root\ninstance 0 0 - {IDENTITY} \"\"\nend\n"
        );
        let m = from_text(&text).unwrap();
        assert!(m.tags().is_empty(), "version {v}");
        assert_eq!(m.instance_tag(InstanceId(0)), None);
        assert!(m.instance_visible(InstanceId(0)));
    }
}

#[test]
fn tag_records_are_unknown_before_version_seven() {
    for v in 1..=6 {
        kind_at(
            &format!("drydock-model {v}\ntag 0 on \"T\"\nend\n"),
            2,
            ReadErrorKind::UnknownRecord,
        );
        kind_at(
            &format!(
                "drydock-model {v}\ndefinition 0 group \"G\"\nbody root\ninstance 0 0 - {IDENTITY} \"\"\ntagged 0 0\nend\n"
            ),
            5,
            ReadErrorKind::UnknownRecord,
        );
    }
}

#[test]
fn tag_records_may_sit_anywhere_before_end() {
    // a tag inside a body does not end that body; tagged right after its instance
    let text = format!(
        "drydock-model 7\n\
         definition 0 group \"G\"\n\
         body root\n\
         tag 5 off \"Late\"\n\
         vertex 0 1 2 3\n\
         instance 3 0 - {IDENTITY} \"\"\n\
         tagged 3 5\n\
         vertex 1 4 5 6\n\
         body 0\n\
         end\n"
    );
    let m = from_text(&text).unwrap();
    assert_eq!(
        m.tag(TagId(5)),
        Some(&Tag {
            name: "Late".into(),
            visible: false
        })
    );
    assert_eq!(m.instance_tag(InstanceId(3)), Some(TagId(5)));
    assert_eq!(m.root().geometry().vertex_count(), 2);
    // and comes back in the writer's order
    let again = to_text(&m).unwrap();
    assert!(
        again.contains("definition 0 group \"G\"\ntag 5 off \"Late\"\nbody root\n"),
        "{again}"
    );
    assert!(again.ends_with("body 0\ntagged 3 5\nend\n"), "{again}");
    assert_eq!(from_text(&again).unwrap(), m);
}

#[test]
fn a_tag_record_before_any_body_is_fine() {
    let m = from_text("drydock-model 7\ntag 2 on \"Early\"\nend\n").unwrap();
    assert_eq!(m.tag(TagId(2)), Some(&Tag::new("Early")));
    // the id counter moved past the read id
    let mut m = m;
    let mut log = ModelEdit::new();
    assert!(m.add_tag("Next", &mut log).unwrap().0 > 2);
}

#[test]
fn malformed_tag_records_are_bad_fields() {
    for line in [
        "tag 0 on",
        "tag 0 on \"A\" extra",
        "tag",
        "tag 0 yes \"A\"",
        "tag 0 \"on\" \"A\"",
        "tag 0 on A",
        "tag -1 on \"A\"",
        "tag x on \"A\"",
    ] {
        kind_at(
            &format!("drydock-model 7\n{line}\nend\n"),
            2,
            ReadErrorKind::BadField,
        );
    }
}

#[test]
fn refused_tags_are_rejected() {
    kind_at(
        "drydock-model 7\ntag 0 on \"\"\nend\n",
        2,
        ReadErrorKind::Rejected(ModelError::BadTagName),
    );
    kind_at(
        "drydock-model 7\ntag 0 on \" \\t\"\nend\n",
        2,
        ReadErrorKind::Rejected(ModelError::BadTagName),
    );
    kind_at(
        "drydock-model 7\ntag 0 on \"A\"\ntag 0 off \"B\"\nend\n",
        3,
        ReadErrorKind::Rejected(ModelError::IdInUse),
    );
}

#[test]
fn tagged_needs_the_instance_and_the_tag_first() {
    let def = "definition 0 group \"G\"\n";
    let instance = format!("body root\ninstance 4 0 - {IDENTITY} \"\"\n");
    // instance missing
    kind_at(
        &format!("drydock-model 7\n{def}tag 1 on \"T\"\ntagged 4 1\n{instance}end\n"),
        4,
        ReadErrorKind::Rejected(ModelError::UnknownInstance(InstanceId(4))),
    );
    // tag missing
    kind_at(
        &format!("drydock-model 7\n{def}{instance}tagged 4 1\ntag 1 on \"T\"\nend\n"),
        5,
        ReadErrorKind::Rejected(ModelError::MissingTag(TagId(1))),
    );
}

#[test]
fn malformed_tagged_records_are_bad_fields() {
    let head = format!(
        "drydock-model 7\ndefinition 0 group \"G\"\ntag 1 on \"T\"\ntag 2 on \"U\"\nbody root\ninstance 4 0 - {IDENTITY} \"\"\n"
    );
    for line in [
        "tagged 4",
        "tagged 4 1 1",
        "tagged",
        "tagged four 1",
        "tagged 4 -",
        "tagged \"4\" 1",
    ] {
        kind_at(&format!("{head}{line}\nend\n"), 7, ReadErrorKind::BadField);
    }
    // a second tagged record for the same instance
    kind_at(
        &format!("{head}tagged 4 1\ntagged 4 2\nend\n"),
        8,
        ReadErrorKind::BadField,
    );
    kind_at(
        &format!("{head}tagged 4 1\ntagged 4 1\nend\n"),
        8,
        ReadErrorKind::BadField,
    );
}

#[test]
fn odd_names_survive_the_file() {
    let names = [
        "Tab\there",
        "Line\nbreak",
        "Quote \" and \\ backslash",
        "\u{1b}esc",
        "Ünïcödé 😀",
        " padded ",
        "# not a comment",
    ];
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    for name in names {
        m.add_tag(name, &mut log).unwrap();
    }
    let back = from_text(&to_text(&m).unwrap()).unwrap();
    assert_eq!(back, m);
    let read: Vec<String> = back.tags().iter().map(|(_, t)| t.name.clone()).collect();
    assert_eq!(read, names);
}

#[test]
fn save_and_load_keep_tags() {
    let dir = std::env::temp_dir().join(format!("drydock-tags-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tags.drydock");
    let m = small();
    save(&m, &path).unwrap();
    assert_eq!(load(&path).unwrap(), m);
    std::fs::remove_dir_all(&dir).unwrap();
}
