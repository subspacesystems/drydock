// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use std::path::{Path, PathBuf};

use drydock_kernel::format::*;
use drydock_kernel::model::*;
use drydock_kernel::topology::*;
use glam::{DMat4, DVec3};

fn edit<R>(m: &mut Model, body: BodyId, f: impl FnOnce(&mut Context, &mut Edit) -> R) -> R {
    let mut log = ModelEdit::new();
    m.edit_geometry(body, &mut log, f).unwrap()
}

fn fwd(e: u64) -> EdgeUse {
    EdgeUse::forward(EdgeId(e))
}

fn back(e: u64) -> EdgeUse {
    EdgeUse::backward(EdgeId(e))
}

fn lp(uses: &[EdgeUse]) -> Loop {
    Loop {
        uses: uses.to_vec(),
    }
}

/// A model touching every record. With `stale`, face 2 of the root is painted and then
/// removed, leaving a face material entry for a missing face.
fn sample(stale: bool) -> Model {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let brick = m.add_material(
        Material {
            name: "Brick \"red\"".into(),
            color: [0.5, 0.25, 0.125, 1.0],
            texture: Some("tex\\brick.png".into()),
        },
        &mut log,
    );
    let glass = m.add_material(
        Material {
            name: "Glass".into(),
            color: [0.1, 0.2, 0.3, 0.4],
            texture: None,
        },
        &mut log,
    );
    let chair = m.add_definition(
        DefinitionInfo {
            name: "Chair".into(),
            group: false,
        },
        &mut log,
    );
    let group = m.add_definition(
        DefinitionInfo {
            name: "Group 1".into(),
            group: true,
        },
        &mut log,
    );
    assert_eq!((brick, glass), (MaterialId(0), MaterialId(1)));
    assert_eq!((chair, group), (DefinitionId(0), DefinitionId(1)));

    edit(&mut m, BodyId::Root, |c, e| {
        for p in [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.25, 0.25, 0.0],
            [0.75, 0.25, 0.0],
            [0.5, 0.75, 0.0],
        ] {
            c.insert_vertex(DVec3::from_array(p), e);
        }
        for (a, b) in [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 4)] {
            c.insert_edge(VertexId(a), VertexId(b), e).unwrap();
        }
        c.insert_face(
            Face {
                outer: lp(&[fwd(0), fwd(1), fwd(2), fwd(3)]),
                inner: vec![lp(&[back(6), back(5), back(4)])],
            },
            e,
        )
        .unwrap();
        c.insert_face(
            Face {
                outer: lp(&[fwd(4), fwd(5), fwd(6)]),
                inner: vec![],
            },
            e,
        )
        .unwrap();
        c.insert_face(
            Face {
                outer: lp(&[fwd(4), fwd(5), fwd(6)]),
                inner: vec![],
            },
            e,
        )
        .unwrap();
    });
    m.set_face_materials(
        BodyId::Root,
        FaceId(0),
        FaceMaterials {
            front: Some(brick),
            back: None,
            ..Default::default()
        },
        &mut log,
    )
    .unwrap();
    m.set_face_materials(
        BodyId::Root,
        FaceId(1),
        FaceMaterials {
            front: None,
            back: Some(glass),
            ..Default::default()
        },
        &mut log,
    )
    .unwrap();
    if stale {
        m.set_face_materials(
            BodyId::Root,
            FaceId(2),
            FaceMaterials {
                front: Some(glass),
                back: Some(glass),
                ..Default::default()
            },
            &mut log,
        )
        .unwrap();
    }
    edit(&mut m, BodyId::Root, |c, e| c.delete_face(FaceId(2), e)).unwrap();

    m.add_instance(
        BodyId::Root,
        Instance {
            definition: chair,
            transform: DMat4::from_translation(DVec3::new(2.0, 0.0, -0.5)),
            name: "Chair \u{1}\ttab".into(),
            material: Some(glass),
        },
        &mut log,
    )
    .unwrap();

    let chair_body = BodyId::Definition(chair);
    edit(&mut m, chair_body, |c, e| {
        let a = c.insert_vertex(DVec3::ZERO, e);
        let b = c.insert_vertex(DVec3::new(0.0, 0.0, 0.5), e);
        c.insert_edge(a, b, e).unwrap();
    });
    m.add_instance(
        chair_body,
        Instance {
            definition: group,
            transform: DMat4::IDENTITY,
            name: String::new(),
            material: None,
        },
        &mut log,
    )
    .unwrap();
    m
}

const SAMPLE: &str = r#"modeler-model 1
material 0 0.5 0.25 0.125 1 "Brick \"red\"" "tex\\brick.png"
material 1 0.1 0.2 0.3 0.4 "Glass" -
definition 0 component "Chair"
definition 1 group "Group 1"
body root
vertex 0 0 0 0
vertex 1 1 0 0
vertex 2 1 1 0
vertex 3 0 1 0
vertex 4 0.25 0.25 0
vertex 5 0.75 0.25 0
vertex 6 0.5 0.75 0
edge 0 0 1
edge 1 1 2
edge 2 2 3
edge 3 3 0
edge 4 4 5
edge 5 5 6
edge 6 6 4
face 0 +0 +1 +2 +3 | -6 -5 -4
face 1 +4 +5 +6
paint 0 0 -
paint 1 - 1
instance 0 0 1 1 0 0 0 0 1 0 0 0 0 1 0 2 0 -0.5 1 "Chair \u{1}\ttab"
body 0
vertex 0 0 0 0
vertex 1 0 0 0.5
edge 0 0 1
instance 1 1 - 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 ""
body 1
end
"#;

const H: &str = "modeler-model 1\n";

fn read_err(text: &str) -> ReadError {
    match from_text(text) {
        Ok(_) => panic!("expected an error for {text:?}"),
        Err(e) => e,
    }
}

fn kind_at(text: &str, line: usize, kind: ReadErrorKind) {
    assert_eq!(read_err(text), ReadError { line, kind }, "text: {text:?}");
}

fn bad_field(body: &str) {
    let text = format!("{H}definition 0 component \"d\"\nbody root\n{body}\nend\n");
    kind_at(&text, 4, ReadErrorKind::BadField);
}

// -- writer ------------------------------------------------------------------------------

#[test]
fn version_is_at_least_two() {
    // Version 2 (m3-guides-model) adds guide records; files without guides still say 1.
    // Version 3 (m4-texture-model) has its own test.
    const { assert!(FORMAT_VERSION >= 2) };
}

#[test]
fn writes_sample_exactly() {
    assert_eq!(to_text(&sample(true)).unwrap(), SAMPLE);
    assert_eq!(to_text(&sample(false)).unwrap(), SAMPLE);
}

#[test]
fn writes_empty_model() {
    assert_eq!(
        to_text(&Model::new()).unwrap(),
        "modeler-model 1\nbody root\nend\n"
    );
}

#[test]
fn writes_empty_definition_bodies() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    for name in ["a", "b"] {
        m.add_definition(
            DefinitionInfo {
                name: name.into(),
                group: false,
            },
            &mut log,
        );
    }
    assert_eq!(
        to_text(&m).unwrap(),
        "modeler-model 1\ndefinition 0 component \"a\"\ndefinition 1 component \"b\"\nbody root\nbody 0\nbody 1\nend\n"
    );
}

#[test]
fn writes_shortest_numbers() {
    let mut m = Model::new();
    edit(&mut m, BodyId::Root, |c, e| {
        c.insert_vertex(DVec3::new(0.1, -0.0, 1e-7), e);
        c.insert_vertex(DVec3::new(123456.789, -2.5, 1e21), e);
    });
    let text = to_text(&m).unwrap();
    assert!(
        text.contains(&format!("\nvertex 0 0.1 -0 {}\n", 1e-7f64)),
        "{text}"
    );
    assert!(
        text.contains(&format!("\nvertex 1 123456.789 -2.5 {}\n", 1e21f64)),
        "{text}"
    );
}

#[test]
fn escapes_strings() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let name = "a\\b\"c\nd\re\tf\u{0}g\u{1f}h\u{7f}i é 日本 \u{1F600} \u{80}";
    m.add_material(
        Material {
            name: name.into(),
            color: [1.0; 4],
            texture: Some(" spaced path/x.png ".into()),
        },
        &mut log,
    );
    let text = to_text(&m).unwrap();
    let expected = "material 0 1 1 1 1 \"a\\\\b\\\"c\\nd\\re\\tf\\u{0}g\\u{1f}h\\u{7f}i é 日本 \u{1F600} \u{80}\" \" spaced path/x.png \"\n";
    assert!(text.contains(expected), "{text}");
    let back = from_text(&text).unwrap();
    assert_eq!(back, m);
    assert_eq!(back.material(MaterialId(0)).unwrap().name, name);
}

#[test]
fn non_finite_numbers_are_not_written() {
    let mut m = Model::new();
    edit(&mut m, BodyId::Root, |c, e| {
        c.insert_vertex(DVec3::new(0.0, f64::NAN, 0.0), e);
    });
    assert_eq!(to_text(&m), Err(WriteError::NonFinite));

    let mut m = Model::new();
    let mut log = ModelEdit::new();
    m.add_material(
        Material {
            name: "x".into(),
            color: [0.0, 0.0, f32::INFINITY, 1.0],
            texture: None,
        },
        &mut log,
    );
    assert_eq!(to_text(&m), Err(WriteError::NonFinite));

    let mut m = Model::new();
    let d = m.add_definition(
        DefinitionInfo {
            name: "d".into(),
            group: false,
        },
        &mut log,
    );
    let mut t = DMat4::IDENTITY;
    t.w_axis.x = f64::NEG_INFINITY;
    m.add_instance(
        BodyId::Root,
        Instance {
            definition: d,
            transform: t,
            name: String::new(),
            material: None,
        },
        &mut log,
    )
    .unwrap();
    assert_eq!(to_text(&m), Err(WriteError::NonFinite));
}

// -- reader ------------------------------------------------------------------------------

#[test]
fn reads_sample() {
    let m = from_text(SAMPLE).unwrap();
    assert_eq!(m, sample(false));
    assert_ne!(m, sample(true));
    assert_eq!(to_text(&m).unwrap(), SAMPLE);
    assert!(m.validate().iter().all(|v| !matches!(
        v,
        ModelViolation::BadTransform(..) | ModelViolation::GroupInstances(..)
    )));
}

#[test]
fn reading_sets_id_counters() {
    let mut m = from_text(SAMPLE).unwrap();
    let mut log = ModelEdit::new();
    let mat = m.add_material(
        Material {
            name: "n".into(),
            color: [1.0; 4],
            texture: None,
        },
        &mut log,
    );
    assert_eq!(mat, MaterialId(2));
    let d = m.add_definition(
        DefinitionInfo {
            name: "n".into(),
            group: false,
        },
        &mut log,
    );
    assert_eq!(d, DefinitionId(2));
    let i = m
        .add_instance(
            BodyId::Root,
            Instance {
                definition: d,
                transform: DMat4::IDENTITY,
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    assert_eq!(i, InstanceId(2));
    let v = edit(&mut m, BodyId::Root, |c, e| c.insert_vertex(DVec3::ONE, e));
    assert_eq!(v, VertexId(7));
    let e7 = edit(&mut m, BodyId::Root, |c, e| {
        c.insert_edge(VertexId(7), VertexId(0), e).unwrap()
    });
    assert_eq!(e7, EdgeId(7));
}

#[test]
fn reads_liberal_layout() {
    let text = "modeler-model 1\r\n# a comment\r\n\r\n  \t \r\nmaterial 3 1 1 1 1 \"a\" -\r\n\tbody\troot  \r\n  # indented comment\r\nvertex 5 1e0 2.5 -3\r\nend\r\n\r\n   \r\n";
    let m = from_text(text).unwrap();
    assert_eq!(m.material(MaterialId(3)).unwrap().name, "a");
    assert_eq!(m.material(MaterialId(3)).unwrap().texture, None);
    assert_eq!(
        m.root().geometry().vertex(VertexId(5)).unwrap().position,
        DVec3::new(1.0, 2.5, -3.0)
    );
    assert_eq!(m.root().geometry().vertex_count(), 1);
}

#[test]
fn reads_bodies_in_any_order_and_definitions_late() {
    let text = format!(
        "{H}definition 4 group \"g\"\nbody 4\nvertex 0 1 2 3\nmaterial 0 0 0 0 1 \"m\" \"t.png\"\nbody root\ninstance 9 4 0 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 \"only\"\ndefinition 2 component \"late\"\nend\n"
    );
    let m = from_text(&text).unwrap();
    assert_eq!(
        m.definition(DefinitionId(4)),
        Some(&DefinitionInfo {
            name: "g".into(),
            group: true
        })
    );
    assert!(m.definition(DefinitionId(2)).is_some());
    assert_eq!(
        m.body(BodyId::Definition(DefinitionId(4)))
            .unwrap()
            .geometry()
            .vertex_count(),
        1
    );
    let inst = m.root().instance(InstanceId(9)).unwrap();
    assert_eq!(inst.definition, DefinitionId(4));
    assert_eq!(inst.material, Some(MaterialId(0)));
    assert_eq!(inst.transform, DMat4::IDENTITY);
    assert_eq!(inst.name, "only");
    assert_eq!(
        m.material(MaterialId(0))
            .unwrap()
            .texture
            .as_ref()
            .map(|t| t.path.as_str()),
        Some("t.png")
    );
    // The definition with no body section has an empty body.
    assert!(
        m.body(BodyId::Definition(DefinitionId(2)))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn reads_transform_column_major() {
    let text = format!(
        "{H}definition 0 component \"d\"\nbody root\ninstance 0 0 - 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 \"x\"\nend\n"
    );
    let m = from_text(&text).unwrap();
    let t = m.root().instance(InstanceId(0)).unwrap().transform;
    assert_eq!(
        t.to_cols_array(),
        [
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0
        ]
    );
    assert_eq!(t.x_axis.y, 2.0);
}

#[test]
fn reads_face_loops() {
    let text = format!(
        "{H}body root\nvertex 0 0 0 0\nvertex 1 1 0 0\nvertex 2 0 1 0\nedge 0 0 1\nedge 1 1 2\nedge 2 2 0\nface 0 +0 +1 +2\nface 1 -2 -1 -0 | +0 | \nface 2\nface 3 |\nend\n"
    );
    let m = from_text(&text).unwrap();
    let g = m.root().geometry();
    assert_eq!(
        g.face(FaceId(0)).unwrap(),
        &Face {
            outer: lp(&[fwd(0), fwd(1), fwd(2)]),
            inner: vec![]
        }
    );
    assert_eq!(
        g.face(FaceId(1)).unwrap(),
        &Face {
            outer: lp(&[back(2), back(1), back(0)]),
            inner: vec![lp(&[fwd(0)]), lp(&[])]
        }
    );
    assert_eq!(
        g.face(FaceId(2)).unwrap(),
        &Face {
            outer: lp(&[]),
            inner: vec![]
        }
    );
    assert_eq!(
        g.face(FaceId(3)).unwrap(),
        &Face {
            outer: lp(&[]),
            inner: vec![lp(&[])]
        }
    );
    // And it writes them back the same way.
    let out = to_text(&m).unwrap();
    assert!(
        out.contains("\nface 1 -2 -1 -0 | +0 |\nface 2\nface 3 |\n"),
        "{out}"
    );
}

#[test]
fn edge_keeps_end_order() {
    let text =
        format!("{H}body root\nvertex 3 0 0 0\nvertex 1 1 0 0\nedge 8 3 1\nedge 2 1 3\nend\n");
    let m = from_text(&text).unwrap();
    let g = m.root().geometry();
    assert_eq!(g.edge(EdgeId(8)).unwrap().ends, [VertexId(3), VertexId(1)]);
    assert_eq!(g.edge(EdgeId(2)).unwrap().ends, [VertexId(1), VertexId(3)]);
}

#[test]
fn paint_replaces_and_clears() {
    let tri = "vertex 0 0 0 0\nvertex 1 1 0 0\nvertex 2 0 1 0\nedge 0 0 1\nedge 1 1 2\nedge 2 2 0\nface 0 +0 +1 +2\n";
    let mats = "material 0 1 0 0 1 \"r\" -\nmaterial 1 0 1 0 1 \"g\" -\n";
    let text = format!("{H}{mats}body root\n{tri}paint 0 0 1\npaint 0 - 1\nend\n");
    let m = from_text(&text).unwrap();
    assert_eq!(
        m.root().face_materials(FaceId(0)),
        FaceMaterials {
            front: None,
            back: Some(MaterialId(1)),
            ..Default::default()
        }
    );
    let text = format!("{H}{mats}body root\n{tri}paint 0 0 1\npaint 0 - -\nend\n");
    let m = from_text(&text).unwrap();
    assert_eq!(m.root().face_materials(FaceId(0)), FaceMaterials::default());
    assert_eq!(m.root().face_material_entries().count(), 0);
}

#[test]
fn group_and_component_kinds() {
    let text = format!("{H}definition 0 group \"\"\ndefinition 1 component \"c d\"\nend\n");
    let m = from_text(&text).unwrap();
    assert!(m.definition(DefinitionId(0)).unwrap().group);
    assert!(!m.definition(DefinitionId(1)).unwrap().group);
    assert_eq!(m.definition(DefinitionId(1)).unwrap().name, "c d");
}

#[test]
fn reads_escapes() {
    let text = format!(
        "{H}material 0 1 1 1 1 \"\\u{{41}}\\u{{1F600}}\\u{{e9}}\\t\\\\\\\"\" \"\\n\"\nend\n"
    );
    let m = from_text(&text).unwrap();
    let mat = m.material(MaterialId(0)).unwrap();
    assert_eq!(mat.name, "A\u{1F600}é\t\\\"");
    assert_eq!(mat.texture.as_ref().map(|t| t.path.as_str()), Some("\n"));
}

#[test]
fn quoted_dash_is_a_texture_named_dash() {
    let text = format!("{H}material 0 1 1 1 1 \"m\" \"-\"\nend\n");
    let m = from_text(&text).unwrap();
    assert_eq!(
        m.material(MaterialId(0))
            .unwrap()
            .texture
            .as_ref()
            .map(|t| t.path.as_str()),
        Some("-")
    );
    assert!(
        to_text(&m)
            .unwrap()
            .contains("material 0 1 1 1 1 \"m\" \"-\"\n")
    );
}

// -- reader errors -----------------------------------------------------------------------

#[test]
fn not_a_model_file() {
    for text in [
        "",
        "\n",
        "hello\n",
        "modeler-model\n",
        "modeler-model 1 x\n",
        "modeler-model x\nend\n",
        "modeler-model -1\nend\n",
        "modeler-model 99999999999\nend\n",
        "\"modeler-model\" 1\nend\n",
        "# comment\nmodeler-model 1\nend\n",
        "Modeler-model 1\nend\n",
    ] {
        kind_at(text, 1, ReadErrorKind::NotAModelFile);
    }
}

#[test]
fn unsupported_version() {
    kind_at(
        "modeler-model 99\nend\n",
        1,
        ReadErrorKind::UnsupportedVersion(99),
    );
    kind_at(
        "modeler-model 0\nend\n",
        1,
        ReadErrorKind::UnsupportedVersion(0),
    );
}

#[test]
fn unknown_record() {
    kind_at(
        &format!("{H}body root\nwidget 1\nend\n"),
        3,
        ReadErrorKind::UnknownRecord,
    );
    kind_at(
        &format!("{H}\"body\" root\nend\n"),
        2,
        ReadErrorKind::UnknownRecord,
    );
    kind_at(&format!("{H}END\n"), 2, ReadErrorKind::UnknownRecord);
}

#[test]
fn bad_fields() {
    for line in [
        "vertex 0 0 0",
        "vertex 0 0 0 0 0",
        "vertex 0 0 0 x",
        "vertex 0 NaN 0 0",
        "vertex 0 inf 0 0",
        "vertex 0 -infinity 0 0",
        "vertex 0 1e400 0 0",
        "vertex -1 0 0 0",
        "vertex 1.5 0 0 0",
        "vertex \"0\" 0 0 0",
        "vertex 0 \"0\" 0 0",
        "edge 0 0",
        "edge 0 a 1",
        "face",
        "face x",
        "face 0 5",
        "face 0 +x",
        "face 0 +",
        "face 0 *1",
        "face 0 \"+1\"",
        "face 0 +1 || +2",
        "paint 0 x -",
        "paint 0 -",
        "paint 0 - - -",
        "instance 0 0 - 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1",
        "instance 0 0 - 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 name",
        "instance 0 0 - 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 nan \"n\"",
        "instance 0 0 \"-\" 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 \"n\"",
        "material 0 1 1 1 1 \"m\"",
        "material 0 1 1 1 1 m -",
        "material 0 1 1 1 1 \"m\" tex.png",
        "material 0 1 1 1e40 1 \"m\" -",
        "material 0 1 1 1 \"m\" -",
        "definition 1 solid \"d\"",
        "definition 1 component",
        "definition 1 \"component\" \"d\"",
        "definition 1 component d",
        "body",
        "body abc",
        "body \"root\"",
        "body root root",
        "end now",
        "material 0 1 1 1 1 \"m\"x -",
        "material 0 1 1 1 1 \"m -",
        "material 0 1 1 1 1 \"m\\q\" -",
        "material 0 1 1 1 1 \"m\\u{}\" -",
        "material 0 1 1 1 1 \"m\\u{1234567}\" -",
        "material 0 1 1 1 1 \"m\\u{0000041}\" -",
        "material 0 1 1 1 1 \"m\\u{d800}\" -",
        "material 0 1 1 1 1 \"m\\u{110000}\" -",
        "material 0 1 1 1 1 \"m\\u{zz}\" -",
        "material 0 1 1 1 1 \"m\\u41\" -",
        "material 0 1 1 1 1 \"m\\",
        "material 0 1 1 1 1 a\"b\" -",
    ] {
        bad_field(line);
    }
}

#[test]
fn no_body() {
    for line in [
        "vertex 0 0 0 0",
        "edge 0 0 1",
        "face 0",
        "paint 0 - -",
        "instance 0 0 - 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 \"n\"",
    ] {
        kind_at(
            &format!("{H}material 0 1 1 1 1 \"m\" -\n{line}\nbody root\nend\n"),
            3,
            ReadErrorKind::NoBody,
        );
    }
}

#[test]
fn duplicate_body() {
    kind_at(
        &format!("{H}body root\nbody root\nend\n"),
        3,
        ReadErrorKind::DuplicateBody,
    );
    kind_at(
        &format!("{H}definition 0 component \"d\"\nbody 0\nbody root\nbody 0\nend\n"),
        5,
        ReadErrorKind::DuplicateBody,
    );
}

#[test]
fn rejected_records() {
    use ReadErrorKind::Rejected;
    let cases: Vec<(String, usize, ModelError)> = vec![
        (
            format!("{H}material 0 1 1 1 1 \"a\" -\nmaterial 0 1 1 1 1 \"b\" -\nend\n"),
            3,
            ModelError::IdInUse,
        ),
        (
            format!("{H}definition 1 group \"a\"\ndefinition 1 group \"a\"\nend\n"),
            3,
            ModelError::IdInUse,
        ),
        (
            format!("{H}body 4\nend\n"),
            2,
            ModelError::MissingBody(BodyId::Definition(DefinitionId(4))),
        ),
        (
            format!("{H}body root\nvertex 0 0 0 0\nvertex 0 1 0 0\nend\n"),
            4,
            ModelError::Topo(TopoError::IdInUse),
        ),
        (
            format!("{H}body root\nvertex 0 0 0 0\nedge 0 0 9\nend\n"),
            4,
            ModelError::Topo(TopoError::MissingVertex(VertexId(9))),
        ),
        (
            format!("{H}body root\nvertex 0 0 0 0\nedge 0 0 0\nend\n"),
            4,
            ModelError::Topo(TopoError::SameVertex(VertexId(0))),
        ),
        (
            format!("{H}body root\nface 0 +3\nend\n"),
            3,
            ModelError::Topo(TopoError::MissingEdge(EdgeId(3))),
        ),
        (
            format!("{H}body root\npaint 0 - -\nend\n"),
            3,
            ModelError::Topo(TopoError::MissingFace(FaceId(0))),
        ),
        (
            format!("{H}body root\nface 0\npaint 0 7 -\nend\n"),
            4,
            ModelError::MissingMaterial(MaterialId(7)),
        ),
        (
            format!("{H}body root\nface 0\npaint 0 - 8\nend\n"),
            4,
            ModelError::MissingMaterial(MaterialId(8)),
        ),
        (
            format!("{H}body root\ninstance 0 3 - 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 \"n\"\nend\n"),
            3,
            ModelError::MissingDefinition(DefinitionId(3)),
        ),
        (
            format!(
                "{H}definition 0 group \"g\"\nbody root\ninstance 0 0 5 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 \"n\"\nend\n"
            ),
            4,
            ModelError::MissingMaterial(MaterialId(5)),
        ),
        (
            format!(
                "{H}definition 0 group \"g\"\nbody 0\ninstance 0 0 - 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 \"n\"\nend\n"
            ),
            4,
            ModelError::Cycle,
        ),
        (
            format!(
                "{H}definition 0 group \"g\"\nbody root\ninstance 0 0 - 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 \"n\"\nbody 0\ninstance 0 0 - 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 \"n\"\nend\n"
            ),
            6,
            ModelError::IdInUse,
        ),
    ];
    for (text, line, e) in cases {
        kind_at(&text, line, Rejected(e));
    }
}

#[test]
fn paint_checks_the_current_body() {
    // Face 0 exists in the root but not in definition 0's body.
    let text =
        format!("{H}definition 0 component \"d\"\nbody root\nface 0\nbody 0\npaint 0 - -\nend\n");
    kind_at(
        &text,
        6,
        ReadErrorKind::Rejected(ModelError::Topo(TopoError::MissingFace(FaceId(0)))),
    );
}

#[test]
fn missing_end() {
    kind_at(&format!("{H}body root\n"), 2, ReadErrorKind::MissingEnd);
    kind_at("modeler-model 1", 1, ReadErrorKind::MissingEnd);
    kind_at(
        &format!("{H}body root\n\n# done?\n"),
        4,
        ReadErrorKind::MissingEnd,
    );
}

#[test]
fn trailing_data() {
    kind_at(
        &format!("{H}end\n\nbody root\n"),
        4,
        ReadErrorKind::TrailingData,
    );
    kind_at(&format!("{H}end\nend\n"), 3, ReadErrorKind::TrailingData);
    kind_at(
        &format!("{H}end\n # late comment\n"),
        3,
        ReadErrorKind::TrailingData,
    );
    assert!(from_text(&format!("{H}end\n \t\n\n")).is_ok());
}

#[test]
fn first_problem_wins() {
    kind_at(
        &format!("{H}widget\nvertex x\n"),
        2,
        ReadErrorKind::UnknownRecord,
    );
    kind_at(
        &format!("{H}vertex x\nwidget\n"),
        2,
        ReadErrorKind::BadField,
    );
    // Field errors come before the body check.
    kind_at(
        &format!("{H}vertex 0 0 0\nend\n"),
        2,
        ReadErrorKind::BadField,
    );
    // A string error comes before the record name check.
    kind_at(
        &format!("{H}widget \"open\nend\n"),
        2,
        ReadErrorKind::BadField,
    );
}

// -- files -------------------------------------------------------------------------------

/// A fresh empty directory for one test.
fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("modeler-format-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn save_and_load() {
    let dir = scratch("save-load");
    let path = dir.join("house.mdl");
    save(&sample(true), &path).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), SAMPLE);
    assert_eq!(entries(&dir), ["house.mdl"]);
    assert_eq!(load(&path).unwrap(), sample(false));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn save_replaces_existing_file_and_stale_temp() {
    let dir = scratch("replace");
    let path = dir.join("m.mdl");
    std::fs::write(&path, "old").unwrap();
    std::fs::write(dir.join("m.mdl.tmp"), "leftover from a crash").unwrap();
    save(&Model::new(), &path).unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "modeler-model 1\nbody root\nend\n"
    );
    assert_eq!(entries(&dir), ["m.mdl"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn failed_write_leaves_target_alone() {
    let dir = scratch("nonfinite");
    let path = dir.join("m.mdl");
    std::fs::write(&path, "old").unwrap();
    let mut m = Model::new();
    edit(&mut m, BodyId::Root, |c, e| {
        c.insert_vertex(DVec3::new(f64::INFINITY, 0.0, 0.0), e);
    });
    assert_eq!(
        save(&m, &path),
        Err(FileError::Write(WriteError::NonFinite))
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "old");
    assert_eq!(entries(&dir), ["m.mdl"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn save_into_missing_directory() {
    let dir = scratch("missing-dir");
    let path = dir.join("nope").join("m.mdl");
    match save(&Model::new(), &path) {
        Err(FileError::Io { kind, .. }) => assert_eq!(kind, std::io::ErrorKind::NotFound),
        other => panic!("{other:?}"),
    }
    assert!(entries(&dir).is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn failed_rename_removes_temp() {
    let dir = scratch("onto-dir");
    let path = dir.join("taken");
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("inside"), "keep").unwrap();
    assert!(matches!(
        save(&Model::new(), &path),
        Err(FileError::Io { .. })
    ));
    assert!(path.is_dir());
    assert_eq!(
        std::fs::read_to_string(path.join("inside")).unwrap(),
        "keep"
    );
    assert_eq!(entries(&dir), ["taken"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn save_without_file_name() {
    match save(&Model::new(), Path::new("/")) {
        Err(FileError::Io { kind, .. }) => assert_eq!(kind, std::io::ErrorKind::InvalidInput),
        other => panic!("{other:?}"),
    }
}

#[test]
fn load_errors() {
    let dir = scratch("load-errors");
    match load(&dir.join("absent.mdl")) {
        Err(FileError::Io { kind, message }) => {
            assert_eq!(kind, std::io::ErrorKind::NotFound);
            assert!(!message.is_empty());
        }
        other => panic!("{other:?}"),
    }
    let bin = dir.join("bin.mdl");
    std::fs::write(&bin, b"modeler-model 1\n\xff\xfe\nend\n").unwrap();
    assert_eq!(load(&bin), Err(FileError::NotUtf8));
    let bad = dir.join("bad.mdl");
    std::fs::write(&bad, "modeler-model 1\nbody root\n").unwrap();
    assert_eq!(
        load(&bad),
        Err(FileError::Read(ReadError {
            line: 2,
            kind: ReadErrorKind::MissingEnd
        }))
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
