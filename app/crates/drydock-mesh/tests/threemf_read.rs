// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! 3MF reader (`docs/specs/m7-import-3mf.md`).

use drydock_mesh::MeshObject;
use drydock_mesh::threemf::{ThreeMfError, read};

type Tri = [[f32; 3]; 3];

/// One archive entry: name, bytes, and how it is stored (0 = stored, 8 = deflated, other
/// numbers are written as the method but hold the bytes as they are).
struct Part<'a>(&'a str, Vec<u8>, u16);

fn stored<'a>(name: &'a str, data: &str) -> Part<'a> {
    Part(name, data.as_bytes().to_vec(), 0)
}

fn deflated<'a>(name: &'a str, data: &str) -> Part<'a> {
    Part(name, data.as_bytes().to_vec(), 8)
}

fn zip(parts: &[Part]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for Part(name, data, method) in parts {
        let body = if *method == 8 {
            miniz_oxide::deflate::compress_to_vec(data, 6)
        } else {
            data.clone()
        };
        let offset = out.len() as u32;
        let le16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
        let le32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
        // Local header. The CRC is left zero: the reader must not check it.
        out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
        le16(&mut out, 20);
        le16(&mut out, 0);
        le16(&mut out, *method);
        le32(&mut out, 0);
        le32(&mut out, 0);
        le32(&mut out, body.len() as u32);
        le32(&mut out, data.len() as u32);
        le16(&mut out, name.len() as u16);
        le16(&mut out, 4); // extra field, to be skipped
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(&[1, 2, 3, 4]);
        out.extend_from_slice(&body);
        // Central directory record.
        central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
        le16(&mut central, 20);
        le16(&mut central, 20);
        le16(&mut central, 0);
        le16(&mut central, *method);
        le32(&mut central, 0);
        le32(&mut central, 0);
        le32(&mut central, body.len() as u32);
        le32(&mut central, data.len() as u32);
        le16(&mut central, name.len() as u16);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le32(&mut central, 0);
        le32(&mut central, offset);
        central.extend_from_slice(name.as_bytes());
    }
    let start = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
    out.extend_from_slice(&(parts.len() as u16).to_le_bytes());
    out.extend_from_slice(&(parts.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&5u16.to_le_bytes()); // comment length
    out.extend_from_slice(b"hello");
    out
}

const NS: &str = "http://schemas.microsoft.com/3dmanufacturing/core/2015/02";

fn model_xml(unit: Option<&str>, body: &str) -> String {
    let unit = unit.map(|u| format!(" unit=\"{u}\"")).unwrap_or_default();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<model{unit} xmlns=\"{NS}\">{body}</model>"
    )
}

/// A tetrahedron-ish mesh of two triangles over four vertices.
const MESH: &str = "<mesh><vertices>\
    <vertex x=\"0\" y=\"0\" z=\"0\"/><vertex x=\"10\" y=\"0\" z=\"0\"/>\
    <vertex x=\"0\" y=\"10\" z=\"0\"/><vertex x=\"0\" y=\"0\" z=\"10\"/>\
    </vertices><triangles>\
    <triangle v1=\"0\" v2=\"2\" v3=\"1\"/><triangle v1=\"0\" v2=\"1\" v3=\"3\"/>\
    </triangles></mesh>";

const T_A: Tri = [[0.0, 0.0, 0.0], [0.0, 10.0, 0.0], [10.0, 0.0, 0.0]];
const T_B: Tri = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 0.0, 10.0]];

fn simple(unit: Option<&str>, build: &str) -> Vec<u8> {
    let xml = model_xml(
        unit,
        &format!(
            "<resources><object id=\"1\" type=\"model\" name=\"Bracket\">{MESH}</object></resources>\
             <build>{build}</build>"
        ),
    );
    zip(&[stored("3D/3dmodel.model", &xml)])
}

fn near(a: &Tri, b: &Tri) -> bool {
    a.iter()
        .flatten()
        .zip(b.iter().flatten())
        .all(|(x, y)| (x - y).abs() < 1e-4)
}

#[test]
fn reads_a_stored_model() {
    let file = read(&simple(None, "<item objectid=\"1\"/>")).unwrap();
    assert_eq!(file.unit_meters, 0.001);
    assert_eq!(
        file.objects,
        vec![MeshObject {
            name: "Bracket".into(),
            triangles: vec![T_A, T_B],
        }]
    );
}

#[test]
fn reads_a_deflated_model_past_a_zip_comment_and_extra_fields() {
    let xml = model_xml(
        Some("meter"),
        &format!(
            "<resources><object id=\"7\">{MESH}</object></resources>\
             <build><item objectid=\"7\"/></build>"
        ),
    );
    let file = read(&zip(&[
        stored("[Content_Types].xml", "<Types/>"),
        deflated("3D/3dmodel.model", &xml),
    ]))
    .unwrap();
    assert_eq!(file.unit_meters, 1.0);
    assert_eq!(file.objects.len(), 1);
    assert_eq!(file.objects[0].name, "Object 7");
    assert_eq!(file.objects[0].triangles, vec![T_A, T_B]);
}

#[test]
fn units_in_meters() {
    for (name, meters) in [
        ("micron", 1e-6),
        ("millimeter", 1e-3),
        ("centimeter", 1e-2),
        ("inch", 0.0254),
        ("foot", 0.3048),
        ("meter", 1.0),
    ] {
        let file = read(&simple(Some(name), "<item objectid=\"1\"/>")).unwrap();
        assert_eq!(file.unit_meters, meters, "{name}");
    }
}

#[test]
fn unknown_unit_is_a_model_error() {
    let err = read(&simple(Some("furlong"), "<item objectid=\"1\"/>")).unwrap_err();
    assert!(matches!(err, ThreeMfError::Model(m) if m.contains("furlong")));
}

#[test]
fn model_part_found_through_the_relationships() {
    let rels = "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
        <Relationship Target=\"/Objects/part.model\" Id=\"rel0\" \
        Type=\"http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel\"/></Relationships>";
    let xml = model_xml(
        None,
        &format!(
            "<resources><object id=\"1\">{MESH}</object></resources><build><item objectid=\"1\"/></build>"
        ),
    );
    let file = read(&zip(&[
        stored("_rels/.rels", rels),
        stored("Objects/part.model", &xml),
    ]))
    .unwrap();
    assert_eq!(file.objects.len(), 1);
}

#[test]
fn item_transform_is_a_row_vector_matrix() {
    // Rotate 90 degrees about Z (x -> y, y -> -x), then move by (100, 200, 300).
    let t = "0 1 0 -1 0 0 0 0 1 100 200 300";
    let file = read(&simple(
        None,
        &format!("<item objectid=\"1\" transform=\"{t}\"/>"),
    ))
    .unwrap();
    let want_a: Tri = [
        [100.0, 200.0, 300.0],
        [90.0, 200.0, 300.0],
        [100.0, 210.0, 300.0],
    ];
    let want_b: Tri = [
        [100.0, 200.0, 300.0],
        [100.0, 210.0, 300.0],
        [100.0, 200.0, 310.0],
    ];
    assert!(
        near(&file.objects[0].triangles[0], &want_a),
        "{:?}",
        file.objects[0]
    );
    assert!(
        near(&file.objects[0].triangles[1], &want_b),
        "{:?}",
        file.objects[0]
    );
}

#[test]
fn a_mirroring_transform_keeps_triangles_facing_outward() {
    // x -> -x turns the corner order inside out; the reader swaps corners 2 and 3.
    let t = "-1 0 0 0 1 0 0 0 1 0 0 0";
    let file = read(&simple(
        None,
        &format!("<item objectid=\"1\" transform=\"{t}\"/>"),
    ))
    .unwrap();
    let want_a: Tri = [[0.0, 0.0, 0.0], [-10.0, 0.0, 0.0], [0.0, 10.0, 0.0]];
    assert!(
        near(&file.objects[0].triangles[0], &want_a),
        "{:?}",
        file.objects[0]
    );
}

#[test]
fn each_build_item_is_an_object_even_when_they_share_a_mesh() {
    let file = read(&simple(
        None,
        "<item objectid=\"1\"/><item objectid=\"1\" transform=\"1 0 0 0 1 0 0 0 1 50 0 0\"/>",
    ))
    .unwrap();
    assert_eq!(file.objects.len(), 2);
    assert_eq!(file.objects[1].name, "Bracket");
    assert!(near(
        &file.objects[1].triangles[0],
        &[[50.0, 0.0, 0.0], [50.0, 10.0, 0.0], [60.0, 0.0, 0.0]]
    ));
}

#[test]
fn components_are_flattened_with_their_transforms() {
    let xml = model_xml(
        None,
        &format!(
            "<resources>\
             <object id=\"1\">{MESH}</object>\
             <object id=\"2\" name=\"Pair\"><components>\
               <component objectid=\"1\"/>\
               <component objectid=\"1\" transform=\"1 0 0 0 1 0 0 0 1 0 0 20\"/>\
             </components></object>\
             </resources>\
             <build><item objectid=\"2\" transform=\"1 0 0 0 1 0 0 0 1 5 0 0\"/></build>"
        ),
    );
    let file = read(&zip(&[stored("3D/3dmodel.model", &xml)])).unwrap();
    assert_eq!(file.objects.len(), 1);
    let o = &file.objects[0];
    assert_eq!(o.name, "Pair");
    assert_eq!(o.triangles.len(), 4);
    assert!(near(
        &o.triangles[0],
        &[[5.0, 0.0, 0.0], [5.0, 10.0, 0.0], [15.0, 0.0, 0.0]]
    ));
    assert!(near(
        &o.triangles[2],
        &[[5.0, 0.0, 20.0], [5.0, 10.0, 20.0], [15.0, 0.0, 20.0]]
    ));
}

#[test]
fn without_a_build_section_every_unreferenced_object_is_one() {
    let xml = model_xml(
        None,
        &format!(
            "<resources>\
             <object id=\"1\">{MESH}</object>\
             <object id=\"2\" name=\"Other\">{MESH}</object>\
             <object id=\"3\" name=\"Wrapper\"><components><component objectid=\"1\"/></components></object>\
             </resources>"
        ),
    );
    let file = read(&zip(&[stored("3D/3dmodel.model", &xml)])).unwrap();
    let names: Vec<&str> = file.objects.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, ["Other", "Wrapper"]);
    assert_eq!(file.objects[1].triangles.len(), 2);
}

#[test]
fn not_a_zip() {
    assert_eq!(read(b"solid nope").unwrap_err(), ThreeMfError::NotZip);
    assert_eq!(read(&[]).unwrap_err(), ThreeMfError::NotZip);
}

#[test]
fn missing_model_part() {
    let bytes = zip(&[stored("[Content_Types].xml", "<Types/>")]);
    assert_eq!(read(&bytes).unwrap_err(), ThreeMfError::MissingModel);
}

#[test]
fn unsupported_compression_method() {
    let bytes = zip(&[Part("3D/3dmodel.model", b"<model/>".to_vec(), 12)]);
    assert!(matches!(read(&bytes).unwrap_err(), ThreeMfError::Zip(m) if m.contains("12")));
}

#[test]
fn damaged_deflate_data() {
    let mut bytes = zip(&[deflated(
        "3D/3dmodel.model",
        &model_xml(None, "<resources/>"),
    )]);
    // Local header is 30 bytes + 16-byte name + 4 extra = 50; wreck the deflate stream.
    for b in &mut bytes[50..58] {
        *b = 0xff;
    }
    assert!(matches!(read(&bytes).unwrap_err(), ThreeMfError::Zip(_)));
}

#[test]
fn xml_that_is_not_well_formed() {
    let bytes = zip(&[stored("3D/3dmodel.model", "<model><resources></model>")]);
    assert!(matches!(read(&bytes).unwrap_err(), ThreeMfError::Xml(_)));
}

fn model_error(body: &str) -> String {
    let bytes = zip(&[stored("3D/3dmodel.model", &model_xml(None, body))]);
    match read(&bytes).unwrap_err() {
        ThreeMfError::Model(m) => m,
        other => panic!("expected a model error, got {other:?}"),
    }
}

#[test]
fn triangle_with_a_missing_vertex() {
    let m = model_error(
        "<resources><object id=\"1\"><mesh><vertices><vertex x=\"0\" y=\"0\" z=\"0\"/></vertices>\
         <triangles><triangle v1=\"0\" v2=\"1\" v3=\"0\"/></triangles></mesh></object></resources>\
         <build><item objectid=\"1\"/></build>",
    );
    assert_eq!(m, "triangle references vertex 1, mesh has 1");
}

#[test]
fn item_for_an_unknown_object() {
    let m = model_error("<resources/><build><item objectid=\"9\"/></build>");
    assert!(m.contains('9'), "{m}");
}

#[test]
fn components_that_contain_themselves() {
    let m = model_error(
        "<resources>\
         <object id=\"1\"><components><component objectid=\"2\"/></components></object>\
         <object id=\"2\"><components><component objectid=\"1\"/></components></object>\
         </resources><build><item objectid=\"1\"/></build>",
    );
    assert_eq!(m, "component cycle");
}

#[test]
fn bad_numbers_and_transforms() {
    let bad_vertex = format!(
        "<resources><object id=\"1\">{}</object></resources><build><item objectid=\"1\"/></build>",
        MESH.replace("x=\"10\"", "x=\"nan\"")
    );
    assert!(model_error(&bad_vertex).contains("nan"));
    let short = model_error(&format!(
        "<resources><object id=\"1\">{MESH}</object></resources>\
         <build><item objectid=\"1\" transform=\"1 0 0\"/></build>"
    ));
    assert_eq!(short, "transform needs 12 numbers");
}

#[test]
fn display_messages() {
    assert_eq!(ThreeMfError::NotZip.to_string(), "not a 3MF file");
    assert_eq!(
        ThreeMfError::MissingModel.to_string(),
        "3MF has no model part"
    );
    assert_eq!(
        ThreeMfError::Model("x".into()).to_string(),
        "bad 3MF model: x"
    );
}
