// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! 3MF writer (`docs/specs/m7-export-3mf.md`).

use std::path::PathBuf;

use drydock_kernel::model::*;
use drydock_mesh::print_mesh::*;
use drydock_mesh::threemf;
use drydock_mesh::threemf_write::*;
use drydock_mesh::{import_3mf, to_kernel};
use glam::{DMat4, DVec3};

mod print_common;
use print_common::*;

const CORE: &str = "http://schemas.microsoft.com/3dmanufacturing/core/2015/02";

fn path(test: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("modeler-3mfw-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.join("out.3mf")
}

fn object(name: &str, color: Option<[f32; 4]>, triangles: Vec<Triangle>) -> PrintObject {
    PrintObject {
        name: name.into(),
        color,
        triangles,
    }
}

/// Triangles of a box group via the real export (mm).
fn box_triangles(lo: DVec3, hi: DVec3) -> Vec<Triangle> {
    let mut model = Model::new();
    let (def, _, _) = box_group(&mut model, "b", lo, hi);
    place(&mut model, BodyId::Root, def, DMat4::IDENTITY, "b");
    let mesh = print_mesh(&model, PrintScope::Model, &PrintOptions::default());
    assert_eq!(mesh.objects.len(), 1);
    mesh.objects[0].triangles.clone()
}

fn tri() -> Triangle {
    [v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0), v(0.0, 3.0, 0.0)]
}

fn le16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(b[at..at + 2].try_into().unwrap())
}

fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// (name, data, crc from the central directory, crc from the local header), walking the
/// archive with its own minimal reader and checking the stored-entry layout on the way.
fn entries(zip: &[u8]) -> Vec<(String, Vec<u8>, u32, u32)> {
    let eocd = zip.len() - 22;
    assert_eq!(
        &zip[eocd..eocd + 4],
        b"PK\x05\x06",
        "end record is last, no comment"
    );
    assert_eq!(le16(zip, eocd + 20), 0);
    let count = le16(zip, eocd + 10) as usize;
    assert_eq!(le16(zip, eocd + 8) as usize, count);
    let cdir = le32(zip, eocd + 16) as usize;
    assert_eq!(
        le32(zip, eocd + 12) as usize,
        eocd - cdir,
        "central directory size"
    );
    let mut pos = cdir;
    let mut out = Vec::new();
    for _ in 0..count {
        assert_eq!(&zip[pos..pos + 4], b"PK\x01\x02");
        assert_eq!(le16(zip, pos + 8) & 8, 0, "no data descriptor");
        assert_eq!(le16(zip, pos + 10), 0, "stored");
        assert_eq!(le16(zip, pos + 12), 0, "dos time");
        assert_eq!(le16(zip, pos + 14), 0x21, "dos date 1980-01-01");
        let crc = le32(zip, pos + 16);
        let csize = le32(zip, pos + 20) as usize;
        let usize_ = le32(zip, pos + 24) as usize;
        assert_eq!(csize, usize_);
        let name_len = le16(zip, pos + 28) as usize;
        assert_eq!(le16(zip, pos + 30), 0, "no extra field");
        assert_eq!(le16(zip, pos + 32), 0, "no comment");
        let local = le32(zip, pos + 42) as usize;
        let name = String::from_utf8(zip[pos + 46..pos + 46 + name_len].to_vec()).unwrap();
        assert_eq!(&zip[local..local + 4], b"PK\x03\x04");
        assert_eq!(le16(zip, local + 6) & 8, 0);
        assert_eq!(le16(zip, local + 8), 0);
        let local_crc = le32(zip, local + 14);
        assert_eq!(le32(zip, local + 18) as usize, csize);
        assert_eq!(le32(zip, local + 22) as usize, usize_);
        assert_eq!(le16(zip, local + 26) as usize, name_len);
        assert_eq!(le16(zip, local + 28), 0);
        assert_eq!(
            &zip[local + 30..local + 30 + name_len],
            name.as_bytes(),
            "local name"
        );
        let start = local + 30 + name_len;
        out.push((name, zip[start..start + csize].to_vec(), crc, local_crc));
        pos += 46 + name_len;
    }
    assert_eq!(pos, eocd, "central directory ends at the end record");
    out
}

fn part(zip: &[u8], name: &str) -> String {
    let e = entries(zip);
    let (_, data, _, _) = e
        .iter()
        .find(|(n, ..)| n == name)
        .unwrap_or_else(|| panic!("{name}"));
    String::from_utf8(data.clone()).unwrap()
}

fn encoded(objects: &[PrintObject]) -> Vec<u8> {
    encode(objects).unwrap()
}

fn model_xml(zip: &[u8]) -> String {
    part(zip, "3D/3dmodel.model")
}

// ---------------------------------------------------------------------------
// CRC-32 and ZIP
// ---------------------------------------------------------------------------

#[test]
fn crc32_known_values() {
    assert_eq!(crc32(b""), 0);
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(crc32(b"a"), 0xE8B7_BE43);
    assert_eq!(
        crc32(b"The quick brown fox jumps over the lazy dog"),
        0x414F_A339
    );
    assert_eq!(crc32(&[0u8; 32]), 0x190A_55AD);
}

#[test]
fn zip_stored_layout_and_crc() {
    let a = b"hello".to_vec();
    let b: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
    let zip = zip_stored(&[("a.txt", &a), ("dir/b.bin", &b), ("empty", b"")]);
    let e = entries(&zip);
    assert_eq!(e.len(), 3);
    assert_eq!(e[0].0, "a.txt");
    assert_eq!(e[1].0, "dir/b.bin");
    assert_eq!(e[2].0, "empty");
    assert_eq!(e[0].1, a);
    assert_eq!(e[1].1, b);
    assert!(e[2].1.is_empty());
    for (_, data, crc, local_crc) in &e {
        assert_eq!(*crc, crc32(data));
        assert_eq!(*local_crc, crc32(data));
    }
    assert_eq!(e[0].2, 0x3610_A686);
}

#[test]
fn zip_stored_is_deterministic_and_empty_is_valid() {
    let one = zip_stored(&[("x", b"abc")]);
    assert_eq!(one, zip_stored(&[("x", b"abc")]));
    let none = zip_stored(&[]);
    assert_eq!(none.len(), 22);
    assert_eq!(&none[..4], b"PK\x05\x06");
}

#[test]
fn zip_stored_is_read_by_the_importer_zip_reader() {
    // The reader rejects an archive without the model part, but only after reading every
    // entry, so a clean MissingModel proves the archive itself parses.
    let zip = zip_stored(&[("readme.txt", b"hi")]);
    assert_eq!(
        threemf::read(&zip),
        Err(threemf::ThreeMfError::MissingModel)
    );
}

// ---------------------------------------------------------------------------
// Package parts
// ---------------------------------------------------------------------------

#[test]
fn package_has_the_three_parts_in_order() {
    let zip = encoded(&[object("A", None, vec![tri()])]);
    let names: Vec<String> = entries(&zip).into_iter().map(|e| e.0).collect();
    assert_eq!(
        names,
        ["[Content_Types].xml", "_rels/.rels", "3D/3dmodel.model"]
    );
    for (_, data, crc, local_crc) in entries(&zip) {
        assert_eq!(crc, crc32(&data));
        assert_eq!(local_crc, crc);
    }
}

#[test]
fn content_types_and_relationships() {
    let zip = encoded(&[object("A", None, vec![tri()])]);
    let ct = part(&zip, "[Content_Types].xml");
    let doc = roxmltree::Document::parse(&ct).unwrap();
    let defaults: Vec<(String, String)> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "Default")
        .map(|n| {
            (
                n.attribute("Extension").unwrap().to_string(),
                n.attribute("ContentType").unwrap().to_string(),
            )
        })
        .collect();
    assert!(defaults.contains(&(
        "rels".into(),
        "application/vnd.openxmlformats-package.relationships+xml".into()
    )));
    assert!(defaults.contains(&(
        "model".into(),
        "application/vnd.ms-package.3dmanufacturing-3dmodel+xml".into()
    )));
    assert_eq!(
        doc.root_element().tag_name().namespace(),
        Some("http://schemas.openxmlformats.org/package/2006/content-types")
    );

    let rels = part(&zip, "_rels/.rels");
    let doc = roxmltree::Document::parse(&rels).unwrap();
    let rel: Vec<_> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "Relationship")
        .collect();
    assert_eq!(rel.len(), 1);
    assert_eq!(rel[0].attribute("Target"), Some("/3D/3dmodel.model"));
    assert_eq!(
        rel[0].attribute("Type"),
        Some("http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel")
    );
    assert!(!rel[0].attribute("Id").unwrap().is_empty());
}

#[test]
fn model_part_is_well_formed_core_xml_in_millimeters() {
    let zip = encoded(&[object("A", None, vec![tri()])]);
    let xml = model_xml(&zip);
    assert!(xml.starts_with("<?xml"));
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let root = doc.root_element();
    assert_eq!(root.tag_name().name(), "model");
    assert_eq!(root.tag_name().namespace(), Some(CORE));
    assert_eq!(root.attribute("unit"), Some("millimeter"));
    let objects: Vec<_> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "object")
        .collect();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].attribute("type"), Some("model"));
    assert_eq!(objects[0].attribute("name"), Some("A"));
    let items: Vec<_> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "item")
        .collect();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].attribute("objectid"), objects[0].attribute("id"));
    assert!(items[0].attribute("transform").is_none());
}

#[test]
fn output_is_deterministic() {
    let objs = [object("A", Some([1.0, 0.0, 0.0, 1.0]), vec![tri()])];
    assert_eq!(encoded(&objs), encoded(&objs));
}

// ---------------------------------------------------------------------------
// Mesh content
// ---------------------------------------------------------------------------

#[test]
fn vertices_are_welded_and_triangles_keep_winding() {
    let tris = box_triangles(v(0.0, 0.0, 0.0), v(10.0, 20.0, 30.0));
    assert_eq!(tris.len(), 12);
    let zip = encoded(&[object("Box", None, tris.clone())]);
    let xml = model_xml(&zip);
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let verts: Vec<[f32; 3]> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "vertex")
        .map(|n| {
            [
                n.attribute("x").unwrap().parse().unwrap(),
                n.attribute("y").unwrap().parse().unwrap(),
                n.attribute("z").unwrap().parse().unwrap(),
            ]
        })
        .collect();
    assert_eq!(verts.len(), 8, "a box has eight corners");
    let mut tri_idx = Vec::new();
    for n in doc
        .descendants()
        .filter(|n| n.tag_name().name() == "triangle")
    {
        let i: Vec<usize> = ["v1", "v2", "v3"]
            .iter()
            .map(|a| n.attribute(*a).unwrap().parse().unwrap())
            .collect();
        tri_idx.push(i);
    }
    assert_eq!(tri_idx.len(), 12);
    for (t, i) in tris.iter().zip(&tri_idx) {
        for k in 0..3 {
            let p = verts[i[k]];
            assert_eq!(
                [p[0] as f64, p[1] as f64, p[2] as f64],
                [
                    t[k].x as f32 as f64,
                    t[k].y as f32 as f64,
                    t[k].z as f32 as f64
                ],
                "triangles keep their order and corner order"
            );
        }
    }
}

#[test]
fn degenerate_and_non_finite_triangles_are_dropped() {
    let nan = v(f64::NAN, 0.0, 0.0);
    let good = tri();
    let collapsed = [v(0.0, 0.0, 0.0), v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0)];
    let bad = [nan, v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0)];
    let zip = encoded(&[object("A", None, vec![collapsed, good, bad])]);
    let doc_xml = model_xml(&zip);
    let doc = roxmltree::Document::parse(&doc_xml).unwrap();
    assert_eq!(
        doc.descendants()
            .filter(|n| n.tag_name().name() == "triangle")
            .count(),
        1
    );
    let parsed = threemf::read(&zip).unwrap();
    assert_eq!(parsed.objects[0].triangles.len(), 1);
}

#[test]
fn nothing_to_write() {
    assert_eq!(encode(&[]), Err(ThreeMfWriteError::NothingToWrite));
    assert_eq!(
        encode(&[object("E", None, vec![])]),
        Err(ThreeMfWriteError::NothingToWrite)
    );
    let collapsed = [v(0.0, 0.0, 0.0), v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0)];
    assert_eq!(
        encode(&[object("E", None, vec![collapsed])]),
        Err(ThreeMfWriteError::NothingToWrite)
    );
    let p = path("nothing");
    assert_eq!(write(&[], &p), Err(ThreeMfWriteError::NothingToWrite));
    assert!(!p.exists());
}

#[test]
fn objects_without_triangles_are_skipped() {
    let zip = encoded(&[
        object("Empty", None, vec![]),
        object("Real", None, vec![tri()]),
    ]);
    let parsed = threemf::read(&zip).unwrap();
    assert_eq!(parsed.objects.len(), 1);
    assert_eq!(parsed.objects[0].name, "Real");
}

#[test]
fn names_are_escaped_and_empty_name_becomes_object() {
    let zip = encoded(&[
        object("A & <B> \"q\"/x", None, vec![tri()]),
        object("", None, vec![tri()]),
    ]);
    let parsed = threemf::read(&zip).unwrap();
    assert_eq!(parsed.objects[0].name, "A & <B> \"q\"/x");
    assert_eq!(parsed.objects[1].name, "Object");
}

// ---------------------------------------------------------------------------
// Colors
// ---------------------------------------------------------------------------

type ColorInfo = (
    Vec<String>,
    Vec<(Option<String>, Option<String>)>,
    Option<String>,
);

fn color_info(xml: &str) -> ColorInfo {
    let doc = roxmltree::Document::parse(xml).unwrap();
    let mats: Vec<_> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "basematerials")
        .collect();
    let colors = mats
        .iter()
        .flat_map(|m| m.children().filter(|c| c.tag_name().name() == "base"))
        .map(|b| b.attribute("displaycolor").unwrap().to_string())
        .collect();
    let objs = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "object")
        .map(|o| {
            (
                o.attribute("pid").map(String::from),
                o.attribute("pindex").map(String::from),
            )
        })
        .collect();
    let mat_id = mats
        .first()
        .and_then(|m| m.attribute("id"))
        .map(String::from);
    (colors, objs, mat_id)
}

#[test]
fn colors_become_one_base_per_distinct_color() {
    let red = [1.0, 0.0, 0.0, 1.0];
    let blue = [0.0, 0.0, 1.0, 0.5];
    let zip = encoded(&[
        object("R1", Some(red), vec![tri()]),
        object("None", None, vec![tri()]),
        object("B", Some(blue), vec![tri()]),
        object("R2", Some(red), vec![tri()]),
    ]);
    let xml = model_xml(&zip);
    let (colors, objs, mat_id) = color_info(&xml);
    assert_eq!(colors, ["#FF0000FF", "#0000FF80"]);
    let mat_id = mat_id.unwrap();
    assert_eq!(objs[0], (Some(mat_id.clone()), Some("0".into())));
    assert_eq!(objs[1], (None, None));
    assert_eq!(objs[2], (Some(mat_id.clone()), Some("1".into())));
    assert_eq!(objs[3], (Some(mat_id.clone()), Some("0".into())));

    // Ids are unique across the materials resource and the objects.
    let doc = roxmltree::Document::parse(&xml).unwrap();
    let mut ids: Vec<String> = doc
        .descendants()
        .filter(|n| matches!(n.tag_name().name(), "object" | "basematerials"))
        .map(|n| n.attribute("id").unwrap().to_string())
        .collect();
    let n = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n);
    assert_eq!(n, 5);
}

#[test]
fn color_components_round_to_bytes() {
    let zip = encoded(&[object("C", Some([0.2, 0.5, 1.0, 0.0]), vec![tri()])]);
    let (colors, ..) = color_info(&model_xml(&zip));
    // 0.2*255 = 51, 0.5*255 = 127.5 -> 128, 1.0 -> 255, 0.0 -> 0.
    assert_eq!(colors, ["#3380FF00"]);
    let zip = encoded(&[object("C", Some([-1.0, 2.0, f32::NAN, 1.0]), vec![tri()])]);
    let (colors, ..) = color_info(&model_xml(&zip));
    assert_eq!(colors, ["#00FF00FF"], "clamped; NaN is 0");
}

#[test]
fn colors_that_round_alike_share_a_base() {
    let zip = encoded(&[
        object("A", Some([1.0, 0.0, 0.0, 1.0]), vec![tri()]),
        object("B", Some([0.999, 0.0, 0.0, 1.0]), vec![tri()]),
    ]);
    let (colors, objs, _) = color_info(&model_xml(&zip));
    assert_eq!(colors.len(), 1);
    assert_eq!(objs[0], objs[1]);
}

#[test]
fn no_colors_means_no_materials_resource() {
    let zip = encoded(&[object("A", None, vec![tri()])]);
    let (colors, objs, mat_id) = color_info(&model_xml(&zip));
    assert!(colors.is_empty());
    assert!(mat_id.is_none());
    assert_eq!(objs, [(None, None)]);
}

#[test]
fn colored_package_still_imports() {
    let zip = encoded(&[object("A", Some([0.3, 0.6, 0.9, 1.0]), vec![tri()])]);
    assert_eq!(threemf::read(&zip).unwrap().objects.len(), 1);
}

// ---------------------------------------------------------------------------
// Round trips
// ---------------------------------------------------------------------------

#[test]
fn read_back_gives_mm_names_and_triangles() {
    let tris = box_triangles(v(0.0, 0.0, 0.0), v(10.0, 20.0, 30.0));
    let zip = encoded(&[
        object("First", None, tris.clone()),
        object("Second", Some([1.0, 1.0, 0.0, 1.0]), vec![tri()]),
    ]);
    let parsed = threemf::read(&zip).unwrap();
    assert_eq!(parsed.unit_meters, 0.001);
    assert_eq!(parsed.objects.len(), 2);
    assert_eq!(parsed.objects[0].name, "First");
    assert_eq!(parsed.objects[1].name, "Second");
    assert_eq!(parsed.objects[0].triangles.len(), 12);
    assert_eq!(parsed.objects[1].triangles.len(), 1);
    let first = &parsed.objects[0].triangles[0];
    for k in 0..3 {
        assert_eq!(first[k][0] as f64, tris[0][k].x as f32 as f64);
        assert_eq!(first[k][1] as f64, tris[0][k].y as f32 as f64);
        assert_eq!(first[k][2] as f64, tris[0][k].z as f32 as f64);
    }
}

#[test]
fn file_round_trip_keeps_volume_bbox_and_names() {
    let mut model = Model::new();
    let (a, _, _) = box_group(&mut model, "Block", v(0.0, 0.0, 0.0), v(0.02, 0.03, 0.04));
    place(&mut model, BodyId::Root, a, DMat4::IDENTITY, "Block");
    let cyl = cylinder_group(&mut model, "Peg", 0.005, 0.02, 24, false);
    place(
        &mut model,
        BodyId::Root,
        cyl,
        DMat4::from_translation(v(0.05, 0.0, 0.0)),
        "Peg",
    );
    let mesh = print_mesh(&model, PrintScope::Model, &PrintOptions::default());
    assert_eq!(mesh.objects.len(), 2);

    let p = path("roundtrip");
    write(&mesh.objects, &p).unwrap();
    let back = import_3mf(&p).unwrap();

    // Re-export the re-imported model: same volume and bounding box per named object.
    let again = print_mesh(&back, PrintScope::Model, &PrintOptions::default());
    assert_eq!(again.objects.len(), 2);
    for orig in &mesh.objects {
        let found = again
            .objects
            .iter()
            .find(|o| o.name == orig.name)
            .unwrap_or_else(|| panic!("{} kept", orig.name));
        assert_rel(volume(&orig.triangles), volume(&found.triangles), 1e-6);
        let (lo, hi) = bounds(&orig.triangles);
        let (lo2, hi2) = bounds(&found.triangles);
        assert_close(lo, lo2, 1e-4);
        assert_close(hi, hi2, 1e-4);
    }
    // The block is 20 x 30 x 40 mm.
    let block = again.objects.iter().find(|o| o.name == "Block").unwrap();
    assert_rel(volume(&block.triangles), 20.0 * 30.0 * 40.0, 1e-6);
}

#[test]
fn write_to_a_missing_folder_is_an_io_error() {
    let p = path("missing").parent().unwrap().join("nope").join("x.3mf");
    match write(&[object("A", None, vec![tri()])], &p) {
        Err(ThreeMfWriteError::Io(m)) => assert!(!m.is_empty()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn written_file_equals_encode() {
    let objs = [object("A", None, vec![tri()])];
    let p = path("same");
    write(&objs, &p).unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), encoded(&objs));
}

#[test]
fn to_kernel_accepts_what_the_reader_returns() {
    let tris = box_triangles(v(0.0, 0.0, 0.0), v(1.0, 2.0, 3.0));
    let parsed = threemf::read(&encoded(&[object("Box", None, tris)])).unwrap();
    assert!(to_kernel(&parsed.objects, parsed.unit_meters).is_ok());
}
