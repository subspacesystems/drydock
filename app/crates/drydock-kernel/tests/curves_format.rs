// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Curves in the native file, version 6 (`docs/specs/m7-curves.md`, `docs/format.md`).

use drydock_kernel::curve::Curve;
use drydock_kernel::format::*;
use drydock_kernel::model::*;
use drydock_kernel::tools::arc::ArcTool;
use drydock_kernel::tools::circle::CircleTool;
use drydock_kernel::tools::plane::DrawPlane;
use drydock_kernel::tools::*;
use drydock_kernel::topology::*;
use drydock_kernel::units::{LengthFormat, Unit};
use glam::DVec3;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn xy() -> DrawPlane {
    DrawPlane {
        u: DVec3::X,
        v: DVec3::Y,
    }
}

fn target<'a>(model: &'a mut Model, undo: &'a mut UndoStack, body: BodyId) -> Target<'a> {
    Target {
        model,
        body,
        undo,
        unit: Unit::Meter,
    }
}

fn circle(model: &mut Model, body: BodyId, center: DVec3, r: f64, segments: usize) {
    let mut undo = UndoStack::new();
    let mut t = CircleTool::new(xy());
    let mut tg = target(model, &mut undo, body);
    t.handle(Input::Typed(format!("{segments}s")), &mut tg)
        .unwrap();
    t.handle(Input::Click(center), &mut tg).unwrap();
    t.handle(Input::Click(center + v(r, 0.0, 0.0)), &mut tg)
        .unwrap();
}

fn arc(model: &mut Model, center: DVec3, start: DVec3, degrees: f64) {
    let mut undo = UndoStack::new();
    let mut t = ArcTool::new(xy());
    let mut tg = target(model, &mut undo, BodyId::Root);
    t.handle(Input::Click(center), &mut tg).unwrap();
    t.handle(Input::Click(start), &mut tg).unwrap();
    t.handle(Input::Typed(format!("{degrees}")), &mut tg)
        .unwrap();
}

fn sample() -> Model {
    let mut model = Model::new();
    circle(&mut model, BodyId::Root, v(0.1, 0.2, 0.0), 0.75, 24);
    arc(&mut model, v(5.0, 0.0, 0.0), v(5.3, 0.4, 0.0), 117.0);
    model
}

fn curve_lines(text: &str) -> Vec<(usize, &str)> {
    text.lines()
        .enumerate()
        .filter(|(_, l)| l.starts_with("curve "))
        .collect()
}

fn replace_line(text: &str, index: usize, line: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    lines[index] = line.to_string();
    lines.join("\n") + "\n"
}

#[test]
fn the_version_is_six() {
    assert_eq!(FORMAT_VERSION, 6);
}

#[test]
fn curves_round_trip_exactly() {
    let model = sample();
    assert_eq!(model.root().geometry().curve_count(), 2);
    let text = to_text(&model).unwrap();
    assert!(text.starts_with("modeler-model 6\n"));
    assert_eq!(curve_lines(&text).len(), 2);
    let back = from_text(&text).unwrap();
    assert_eq!(back, model);
    assert_eq!(to_text(&back).unwrap(), text);
}

#[test]
fn a_curve_line_holds_every_number_and_the_chain() {
    let model = sample();
    let text = to_text(&model).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let curves: Vec<(CurveId, Curve)> = model
        .root()
        .geometry()
        .curves()
        .map(|(id, c)| (id, c.clone()))
        .collect();
    let found = curve_lines(&text);
    for ((index, line), (id, c)) in found.iter().zip(&curves) {
        let tokens: Vec<&str> = line.split(' ').collect();
        assert_eq!(tokens[0], "curve");
        assert_eq!(tokens[1], id.0.to_string());
        let numbers = [
            c.center.x, c.center.y, c.center.z, c.normal.x, c.normal.y, c.normal.z, c.x_axis.x,
            c.x_axis.y, c.x_axis.z, c.radius, c.start, c.end,
        ];
        for (k, value) in numbers.iter().enumerate() {
            assert_eq!(tokens[2 + k], value.to_string(), "number {k}");
        }
        let chain: Vec<String> = c.vertices.iter().map(|x| x.0.to_string()).collect();
        assert_eq!(tokens[14..].to_vec(), chain);
        // after the body's faces, before its paint, instances and guides
        let last_face = lines.iter().rposition(|l| l.starts_with("face ")).unwrap();
        assert!(*index > last_face);
        assert!(
            lines[*index + 1..]
                .iter()
                .take_while(|l| !l.starts_with("body ") && **l != "end")
                .all(|l| l.starts_with("curve ")
                    || l.starts_with("paint ")
                    || l.starts_with("texmap ")
                    || l.starts_with("instance ")
                    || l.starts_with("guide "))
        );
    }
    // ascending id
    let ids: Vec<u64> = found
        .iter()
        .map(|(_, l)| l.split(' ').nth(1).unwrap().parse().unwrap())
        .collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
}

#[test]
fn a_model_without_curves_keeps_its_older_version() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    draw_segments(
        &mut target(&mut model, &mut undo, BodyId::Root),
        &[(v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0))],
        "Line",
    )
    .unwrap();
    let text = to_text(&model).unwrap();
    assert!(text.starts_with("modeler-model 1\n"), "{text}");

    let mut log = ModelEdit::new();
    model
        .set_length_format(
            LengthFormat::Decimal {
                unit: Unit::Millimeter,
                decimals: 1,
            },
            &mut log,
        )
        .unwrap();
    let text = to_text(&model).unwrap();
    assert!(text.starts_with("modeler-model 5\n"));
    assert_eq!(from_text(&text).unwrap(), model);

    // with units and a curve: 6
    circle(&mut model, BodyId::Root, v(3.0, 0.0, 0.0), 1.0, 8);
    let text = to_text(&model).unwrap();
    assert!(text.starts_with("modeler-model 6\n"));
    assert!(text.contains("\nunits decimal mm 1\n"));
    assert_eq!(from_text(&text).unwrap(), model);
}

#[test]
fn version_five_files_still_read() {
    let text = "modeler-model 5\n\
                units decimal cm 2\n\
                body root\n\
                vertex 0 0 0 0\n\
                vertex 1 1 0 0\n\
                vertex 2 0 1 0\n\
                edge 0 0 1\n\
                edge 1 1 2\n\
                edge 2 2 0\n\
                face 0 +0 +1 +2\n\
                end\n";
    let model = from_text(text).unwrap();
    assert_eq!(model.root().geometry().face_count(), 1);
    assert_eq!(model.root().geometry().curve_count(), 0);
    assert_eq!(
        model.length_format(),
        LengthFormat::Decimal {
            unit: Unit::Centimeter,
            decimals: 2
        }
    );
}

#[test]
fn a_curve_record_needs_version_six() {
    let text = to_text(&sample()).unwrap();
    let older = text.replacen("modeler-model 6", "modeler-model 5", 1);
    let (index, _) = curve_lines(&older)[0];
    assert_eq!(
        from_text(&older),
        Err(ReadError {
            line: index + 1,
            kind: ReadErrorKind::UnknownRecord,
        })
    );
}

#[test]
fn a_curve_naming_a_missing_vertex_is_rejected() {
    let text = to_text(&sample()).unwrap();
    let (index, line) = curve_lines(&text)[0];
    let mut tokens: Vec<String> = line.split(' ').map(str::to_string).collect();
    tokens[15] = "9999".to_string();
    let bad = replace_line(&text, index, &tokens.join(" "));
    assert_eq!(
        from_text(&bad),
        Err(ReadError {
            line: index + 1,
            kind: ReadErrorKind::Rejected(ModelError::Topo(TopoError::MissingVertex(VertexId(
                9999
            )))),
        })
    );
}

#[test]
fn malformed_curve_records_are_bad_fields() {
    let text = to_text(&sample()).unwrap();
    let (index, line) = curve_lines(&text)[1];
    let tokens: Vec<String> = line.split(' ').map(str::to_string).collect();
    let bad_field = |tokens: &[String]| {
        assert_eq!(
            from_text(&replace_line(&text, index, &tokens.join(" "))),
            Err(ReadError {
                line: index + 1,
                kind: ReadErrorKind::BadField,
            }),
            "{}",
            tokens.join(" ")
        );
    };

    // only two vertices
    bad_field(&tokens[..16]);
    // not a number, not finite, not an id
    for (k, word) in [(5, "x"), (11, "inf"), (12, "NaN"), (14, "-1"), (15, "1.5")] {
        let mut t = tokens.clone();
        t[k] = word.to_string();
        bad_field(&t);
    }
    // numbers that don't fit the vertices: radius, end angle, a reversed chain
    let mut t = tokens.clone();
    t[11] = "2".to_string();
    bad_field(&t);
    let mut t = tokens.clone();
    t[13] = "3".to_string();
    bad_field(&t);
    let mut t = tokens.clone();
    t[14..].reverse();
    bad_field(&t);
}

#[test]
fn a_repeated_curve_id_is_rejected() {
    let text = to_text(&sample()).unwrap();
    let found = curve_lines(&text);
    let (index, second) = found[1];
    let first_id = found[0].1.split(' ').nth(1).unwrap();
    let mut tokens: Vec<&str> = second.split(' ').collect();
    tokens[1] = first_id;
    let bad = replace_line(&text, index, &tokens.join(" "));
    assert_eq!(
        from_text(&bad),
        Err(ReadError {
            line: index + 1,
            kind: ReadErrorKind::Rejected(ModelError::Topo(TopoError::IdInUse)),
        })
    );
}

#[test]
fn a_curve_outside_a_body_is_refused() {
    let text = to_text(&sample()).unwrap();
    let (_, line) = curve_lines(&text)[0];
    let bad = format!("modeler-model 6\n{line}\nend\n");
    assert_eq!(
        from_text(&bad),
        Err(ReadError {
            line: 2,
            kind: ReadErrorKind::NoBody,
        })
    );
}

#[test]
fn curves_in_definitions_round_trip() {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let def = model.add_definition(
        DefinitionInfo {
            name: "Washer".to_string(),
            group: false,
        },
        &mut log,
    );
    circle(&mut model, BodyId::Definition(def), DVec3::ZERO, 0.004, 32);
    circle(&mut model, BodyId::Definition(def), DVec3::ZERO, 0.0021, 16);
    assert_eq!(
        model
            .body(BodyId::Definition(def))
            .unwrap()
            .geometry()
            .curve_count(),
        2
    );
    let text = to_text(&model).unwrap();
    assert!(text.starts_with("modeler-model 6\n"));
    let back = from_text(&text).unwrap();
    assert_eq!(back, model);
    assert!(back.validate().is_empty());
}

#[test]
fn save_and_load_keep_curves() {
    let dir = std::env::temp_dir().join(format!("modeler-curves-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("curves.mdl");
    let model = sample();
    save(&model, &path).unwrap();
    let back = load(&path).unwrap();
    assert_eq!(back, model);
    std::fs::remove_dir_all(&dir).unwrap();
}
