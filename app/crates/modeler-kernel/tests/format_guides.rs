// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m3-guides-model: guides in the native file (`docs/specs/m3-guides-model.md`,
//! `docs/format.md` version 2).

use glam::DVec3;
use modeler_kernel::format::*;
use modeler_kernel::model::*;
use modeler_kernel::tape::{Guide, GuideId};

fn read_err(text: &str) -> ReadError {
    match from_text(text) {
        Ok(_) => panic!("expected an error for {text:?}"),
        Err(e) => e,
    }
}

fn kind_at(text: &str, line: usize, kind: ReadErrorKind) {
    assert_eq!(read_err(text), ReadError { line, kind }, "text: {text:?}");
}

fn add(m: &mut Model, body: BodyId, g: Guide) -> GuideId {
    let mut log = ModelEdit::new();
    m.add_guide(body, g, &mut log).unwrap()
}

/// Root: a vertex, guides 0 (line), 2 (point) after removing 1. Definition 0: one guide.
fn sample() -> Model {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let d = m.add_definition(
        DefinitionInfo {
            name: "d".into(),
            group: false,
        },
        &mut log,
    );
    m.edit_geometry(BodyId::Root, &mut log, |ctx, e| {
        ctx.insert_vertex(DVec3::new(1.0, 2.0, 3.0), e);
    })
    .unwrap();
    add(
        &mut m,
        BodyId::Root,
        Guide::Line {
            point: DVec3::new(0.5, -1.0, 0.0),
            dir: DVec3::new(0.0, 1.0, 0.0),
        },
    );
    let gone = add(&mut m, BodyId::Root, Guide::Point(DVec3::ZERO));
    m.remove_guide(BodyId::Root, gone, &mut log).unwrap();
    add(
        &mut m,
        BodyId::Root,
        Guide::Point(DVec3::new(0.1, -0.0, 2.5)),
    );
    add(
        &mut m,
        BodyId::Definition(d),
        Guide::Line {
            point: DVec3::new(1.0, 1.0, 1.0),
            dir: DVec3::new(-1.0, 0.0, 0.0),
        },
    );
    m
}

const SAMPLE: &str = "modeler-model 2
definition 0 component \"d\"
body root
vertex 0 1 2 3
guide 0 line 0.5 -1 0 0 1 0
guide 2 point 0.1 -0 2.5
body 0
guide 0 line 1 1 1 -1 0 0
end
";

#[test]
fn version_constant_is_two() {
    assert_eq!(FORMAT_VERSION, 2);
}

#[test]
fn writes_guides_exactly() {
    assert_eq!(to_text(&sample()).unwrap(), SAMPLE);
}

#[test]
fn reads_guides_with_ids() {
    let m = from_text(SAMPLE).unwrap();
    assert_eq!(m, sample());
    let ids: Vec<GuideId> = m.root().guides().iter().map(|(id, _)| id).collect();
    assert_eq!(ids, vec![GuideId(0), GuideId(2)]);
    assert_eq!(
        m.root().guides().get(GuideId(2)),
        Some(&Guide::Point(DVec3::new(0.1, -0.0, 2.5)))
    );
}

#[test]
fn round_trip_is_stable() {
    let m = sample();
    let text = to_text(&m).unwrap();
    let back = from_text(&text).unwrap();
    assert_eq!(back, m);
    assert_eq!(to_text(&back).unwrap(), text);
}

#[test]
fn round_trip_odd_numbers() {
    let mut m = Model::new();
    let dir = DVec3::new(1.0, 2.0, 3.0).normalize();
    add(
        &mut m,
        BodyId::Root,
        Guide::Line {
            point: DVec3::new(1e-300, -123456.789, 0.1 + 0.2),
            dir,
        },
    );
    add(
        &mut m,
        BodyId::Root,
        Guide::Point(DVec3::new(f64::MAX, f64::MIN_POSITIVE, -1e20)),
    );
    let back = from_text(&to_text(&m).unwrap()).unwrap();
    assert_eq!(back, m);
    match back.root().guides().get(GuideId(0)) {
        Some(Guide::Line { dir: d, .. }) => assert_eq!(d.to_array(), dir.to_array()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn ids_continue_after_load() {
    let mut m = from_text(SAMPLE).unwrap();
    let id = add(&mut m, BodyId::Root, Guide::Point(DVec3::ONE));
    assert!(id > GuideId(2));
}

#[test]
fn no_guides_writes_version_one() {
    let mut m = Model::new();
    assert_eq!(to_text(&m).unwrap(), "modeler-model 1\nbody root\nend\n");
    let id = add(&mut m, BodyId::Root, Guide::Point(DVec3::ZERO));
    assert!(to_text(&m).unwrap().starts_with("modeler-model 2\n"));
    let mut log = ModelEdit::new();
    m.remove_guide(BodyId::Root, id, &mut log).unwrap();
    assert_eq!(to_text(&m).unwrap(), "modeler-model 1\nbody root\nend\n");
}

#[test]
fn guide_in_definition_only_writes_version_two() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let d = m.add_definition(
        DefinitionInfo {
            name: "d".into(),
            group: true,
        },
        &mut log,
    );
    add(&mut m, BodyId::Definition(d), Guide::Point(DVec3::X));
    assert!(to_text(&m).unwrap().starts_with("modeler-model 2\n"));
}

#[test]
fn reads_both_versions() {
    let v1 = "modeler-model 1\nbody root\nvertex 0 1 2 3\nend\n";
    let v2 = "modeler-model 2\nbody root\nvertex 0 1 2 3\nend\n";
    assert_eq!(from_text(v1).unwrap(), from_text(v2).unwrap());
    for v in [0, 3, 99] {
        kind_at(
            &format!("modeler-model {v}\nend\n"),
            1,
            ReadErrorKind::UnsupportedVersion(v),
        );
    }
}

#[test]
fn guide_in_version_one_is_unknown() {
    kind_at(
        "modeler-model 1\nbody root\nguide 0 point 0 0 0\nend\n",
        3,
        ReadErrorKind::UnknownRecord,
    );
}

#[test]
fn guide_before_body() {
    kind_at(
        "modeler-model 2\nguide 0 point 0 0 0\nend\n",
        2,
        ReadErrorKind::NoBody,
    );
}

#[test]
fn bad_guide_fields() {
    for line in [
        "guide",
        "guide 0",
        "guide 0 point",
        "guide 0 point 0 0",
        "guide 0 point 0 0 0 0",
        "guide 0 line 0 0 0 1 0",
        "guide 0 line 0 0 0 1 0 0 0",
        "guide 0 circle 0 0 0",
        "guide 0 \"point\" 0 0 0",
        "guide 0 POINT 0 0 0",
        "guide x point 0 0 0",
        "guide -1 point 0 0 0",
        "guide 0 point a 0 0",
        "guide 0 point NaN 0 0",
        "guide 0 point inf 0 0",
        "guide 0 line 0 0 0 1 0 1e999",
    ] {
        kind_at(
            &format!("modeler-model 2\nbody root\n{line}\nend\n"),
            3,
            ReadErrorKind::BadField,
        );
    }
}

#[test]
fn rejected_guides() {
    kind_at(
        "modeler-model 2\nbody root\nguide 0 line 0 0 0 2 0 0\nend\n",
        3,
        ReadErrorKind::Rejected(ModelError::BadGuide),
    );
    kind_at(
        "modeler-model 2\nbody root\nguide 0 line 0 0 0 0 0 0\nend\n",
        3,
        ReadErrorKind::Rejected(ModelError::BadGuide),
    );
    kind_at(
        "modeler-model 2\nbody root\nguide 4 point 0 0 0\nguide 4 point 1 0 0\nend\n",
        4,
        ReadErrorKind::Rejected(ModelError::IdInUse),
    );
}

#[test]
fn same_id_in_two_bodies_is_fine() {
    let text = "modeler-model 2\ndefinition 0 component \"d\"\nbody root\nguide 0 point 0 0 0\nbody 0\nguide 0 point 1 0 0\nend\n";
    let m = from_text(text).unwrap();
    assert_eq!(to_text(&m).unwrap(), text);
}

#[test]
fn save_and_load_guides() {
    let path = std::env::temp_dir().join(format!(
        "modeler-format-guides-{}.modeler",
        std::process::id()
    ));
    let m = sample();
    save(&m, &path).unwrap();
    let back = load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(back, m);
}
