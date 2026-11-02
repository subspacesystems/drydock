// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m0-dae-import: reading documents into a `Model`. See docs/specs/m0-dae-import.md.

use std::path::{Path, PathBuf};

use drydock_dae::{ImportError, PrimitiveKind, Projection, UpAxis, import_file, import_str};
use glam::DVec3;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn tables() -> drydock_dae::Model {
    import_file(&fixture("tables.dae")).expect("tables.dae imports")
}

/// A minimal document around `body` (library elements and a scene).
fn doc(asset: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1">
<asset>{asset}</asset>
{body}
</COLLADA>"#
    )
}

/// One geometry `g` holding `prim`, positions from `pos`, placed by a scene node.
fn one_geometry(pos: &str, prim: &str, node_transform: &str) -> String {
    let n = pos.split_whitespace().count();
    doc(
        "",
        &format!(
            r##"<library_geometries><geometry id="g"><mesh>
<source id="p"><float_array id="pa" count="{n}">{pos}</float_array>
<technique_common><accessor source="#pa" count="{c}" stride="3"><param name="X" type="float"/><param name="Y" type="float"/><param name="Z" type="float"/></accessor></technique_common></source>
<vertices id="v"><input semantic="POSITION" source="#p"/></vertices>
{prim}
</mesh></geometry></library_geometries>
<library_visual_scenes><visual_scene id="s"><node name="n">{node_transform}<instance_geometry url="#g"/></node></visual_scene></library_visual_scenes>
<scene><instance_visual_scene url="#s"/></scene>"##,
            c = n / 3
        ),
    )
}

fn approx(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() <= eps
}

#[test]
fn reads_units_and_up_axis() {
    let m = tables();
    assert!(approx(m.unit_meters, 0.0254, 1e-12));
    assert_eq!(m.up_axis, UpAxis::Z);

    let m = import_str(&doc("", ""), Path::new(".")).unwrap();
    assert_eq!(m.unit_meters, 1.0, "unit defaults to meters");
    assert_eq!(m.up_axis, UpAxis::Y, "up axis defaults to Y");
    assert!(m.scene.children.is_empty(), "no visual scene -> empty root");
}

#[test]
fn reads_meshes_in_document_order() {
    let m = tables();
    let ids: Vec<&str> = m.meshes.iter().map(|g| g.id.as_str()).collect();
    assert_eq!(ids, ["g-box", "g-top", "g-edges", "g-empty", "g-raw"]);
    assert_eq!(m.meshes[0].name, "box");
}

#[test]
fn reindexes_shared_vertex_streams() {
    let m = tables();
    // Box: one VERTEX input carrying positions and normals, 24 vertices, 12 triangles.
    let bx = &m.meshes[0].primitives;
    assert_eq!(bx.len(), 1);
    assert_eq!(bx[0].kind, PrimitiveKind::Triangles);
    assert_eq!(bx[0].material_symbol.as_deref(), Some("Mat0"));
    assert_eq!(bx[0].positions.len(), 24);
    assert_eq!(bx[0].normals.len(), 24);
    assert!(bx[0].texcoords.is_empty());
    assert_eq!(bx[0].indices.len(), 36);

    // Top: separate NORMAL and TEXCOORD offsets; six distinct index combinations.
    let top = &m.meshes[1].primitives[0];
    assert_eq!(top.positions.len(), 6);
    assert_eq!(top.normals.len(), 6);
    assert_eq!(top.texcoords.len(), 6);
    assert_eq!(top.indices.len(), 6);
    let tri: Vec<[f32; 3]> = top
        .indices
        .iter()
        .map(|&i| top.positions[i as usize])
        .collect();
    assert_eq!(tri[0], [0.0, 0.0, 0.0]);
    assert_eq!(tri[1], [50.0, 0.0, 0.0]);
    assert_eq!(tri[5], [0.0, 10.0, 0.0]);
    let uv: Vec<[f32; 2]> = top
        .indices
        .iter()
        .map(|&i| top.texcoords[i as usize])
        .collect();
    assert_eq!(uv[2], [1.0, 1.0]);
    assert_eq!(uv[5], [0.0, 1.0]);
    for &i in &top.indices {
        assert_eq!(top.normals[i as usize], [0.0, 0.0, 1.0]);
    }
}

#[test]
fn reads_lines_empty_meshes_and_missing_normals() {
    let m = tables();
    let edges = &m.meshes[2].primitives[0];
    assert_eq!(edges.kind, PrimitiveKind::Lines);
    assert_eq!(edges.indices.len(), 4);
    assert_eq!(edges.material_symbol.as_deref(), Some("Edge"));
    assert!(
        m.meshes[3].primitives.is_empty(),
        "<mesh/> has no primitives"
    );
    let raw = &m.meshes[4].primitives[0];
    assert!(raw.normals.is_empty());
    assert_eq!(raw.material_symbol, None);
}

#[test]
fn reads_materials() {
    let m = tables();
    let names: Vec<&str> = m.materials.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, ["red", "wood", "glass", "edge"]);

    let red = &m.materials[0];
    assert_eq!(red.id, "m-red");
    assert_eq!(red.color, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(red.texture, None);
    assert_eq!(red.opacity, 1.0);
    assert!(!red.unlit);

    let wood = &m.materials[1];
    assert_eq!(wood.color, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(
        wood.texture.as_deref(),
        Some(fixture("box tex/wood grain.png").as_path()),
        "texture path is percent-decoded and resolved against the file's directory"
    );

    let glass = &m.materials[2];
    assert!(
        (glass.opacity - 0.25).abs() < 1e-4,
        "RGB_ZERO 0.75 grey -> 0.25, got {}",
        glass.opacity
    );

    let edge = &m.materials[3];
    assert!(edge.unlit, "constant shading is unlit");
    assert_eq!(
        edge.color,
        [0.1, 0.2, 0.3, 1.0],
        "constant uses its emission color"
    );
}

fn material_doc(effect: &str) -> String {
    doc(
        "",
        &format!(
            r##"<library_images><image id="img"><init_from>file:///abs/dir/pic.png</init_from></image></library_images>
<library_effects><effect id="fx"><profile_COMMON><technique sid="t">{effect}</technique></profile_COMMON></effect></library_effects>
<library_materials><material id="m" name="m"><instance_effect url="#fx"/></material></library_materials>"##
        ),
    )
}

fn only_material(effect: &str) -> drydock_dae::Material {
    let m = import_str(&material_doc(effect), Path::new("/base")).unwrap();
    assert_eq!(m.materials.len(), 1);
    m.materials[0].clone()
}

#[test]
fn opacity_follows_the_common_profile_rules() {
    let a_one = only_material(
        r#"<lambert><diffuse><color>1 1 1 1</color></diffuse><transparent opaque="A_ONE"><color>1 1 1 0.25</color></transparent><transparency><float>1</float></transparency></lambert>"#,
    );
    assert!((a_one.opacity - 0.25).abs() < 1e-4);

    let default_mode = only_material(
        r#"<lambert><diffuse><color>1 1 1 1</color></diffuse><transparent><color>0 0 0 0.5</color></transparent><transparency><float>0.5</float></transparency></lambert>"#,
    );
    assert!(
        (default_mode.opacity - 0.25).abs() < 1e-4,
        "A_ONE is the default mode"
    );

    let rgb_zero = only_material(
        r#"<phong><diffuse><color>1 1 1 1</color></diffuse><transparent opaque="RGB_ZERO"><color>0.2 0.2 0.2 1</color></transparent><transparency><float>0.5</float></transparency></phong>"#,
    );
    assert!(
        (rgb_zero.opacity - 0.9).abs() < 1e-4,
        "1 - 0.2*0.5, got {}",
        rgb_zero.opacity
    );

    let textured = only_material(
        r#"<lambert><diffuse><color>1 1 1 1</color></diffuse><transparent opaque="A_ONE"><texture texture="img" texcoord="UV"/></transparent></lambert>"#,
    );
    assert_eq!(
        textured.opacity, 1.0,
        "texture alpha is applied at render time"
    );

    let none = only_material(r#"<blinn><diffuse><color>0.5 0.5 0.5 1</color></diffuse></blinn>"#);
    assert_eq!(none.opacity, 1.0);
    assert_eq!(none.color, [0.5, 0.5, 0.5, 1.0]);

    let empty = only_material(r#"<lambert/>"#);
    assert_eq!(empty.color, [0.8, 0.8, 0.8, 1.0], "default color");
}

#[test]
fn texture_can_name_an_image_directly_and_file_urls_are_stripped() {
    let m = only_material(
        r#"<lambert><diffuse><texture texture="img" texcoord="UV"/></diffuse></lambert>"#,
    );
    assert_eq!(m.texture.as_deref(), Some(Path::new("/abs/dir/pic.png")));
}

#[test]
fn builds_the_scene_graph() {
    let m = tables();
    assert_eq!(m.scene.name, "fixture");
    let names: Vec<&str> = m.scene.children.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "camera", "table-1", "table-2", "glass", "edges", "nothing", "n-raw"
        ],
        "node name falls back to id"
    );

    let defs: Vec<(&str, &str)> = m
        .definitions
        .iter()
        .map(|d| (d.id.as_str(), d.name.as_str()))
        .collect();
    assert_eq!(defs, [("d-leg", "leg"), ("d-table", "table")]);
    let table = &m.definitions[1].node;
    assert_eq!(table.children.len(), 3);
    assert_eq!(
        table.children[1].instances,
        vec![0],
        "leg-a instances d-leg"
    );

    let t2 = &m.scene.children[2];
    assert_eq!(t2.instances, vec![1]);
    assert_eq!(
        t2.transform.transform_point3(DVec3::new(1.0, 0.0, 0.0)),
        DVec3::new(99.0, 0.0, 0.0)
    );

    // <translate> then <scale>: T * S.
    let glass = &m.scene.children[3];
    let p = glass.transform.transform_point3(DVec3::new(1.0, 1.0, 1.0));
    assert!((p - DVec3::new(1.0, 51.0, 2.0)).length() < 1e-12, "got {p}");
    assert_eq!(glass.geometry.len(), 1);
    assert_eq!(glass.geometry[0].mesh, 0);
    assert_eq!(glass.geometry[0].material_for("Mat0"), Some(2));
    assert_eq!(glass.geometry[0].material_for("Other"), None);
}

#[test]
fn rotate_is_degrees_about_an_axis_in_document_order() {
    let xml = one_geometry(
        "0 0 0 1 0 0 0 1 0",
        r##"<triangles count="1"><input semantic="VERTEX" source="#v" offset="0"/><p>0 1 2</p></triangles>"##,
        "<translate>10 0 0</translate><rotate>0 0 1 90</rotate>",
    );
    let m = import_str(&xml, Path::new(".")).unwrap();
    let p = m.scene.children[0]
        .transform
        .transform_point3(DVec3::new(1.0, 0.0, 0.0));
    assert!((p - DVec3::new(10.0, 1.0, 0.0)).length() < 1e-9, "got {p}");
}

#[test]
fn reads_cameras_with_world_transforms() {
    let m = tables();
    assert_eq!(m.cameras.len(), 2);
    assert_eq!(m.cameras[0].name, "camera");
    assert_eq!(
        m.cameras[0].projection,
        Projection::Perspective { yfov_degrees: 35.0 }
    );
    let eye = m.cameras[0].transform.transform_point3(DVec3::ZERO);
    assert_eq!(eye, DVec3::new(200.0, -200.0, 200.0));
    assert_eq!(m.cameras[1].name, "top-camera");
    assert_eq!(
        m.cameras[1].projection,
        Projection::Orthographic { xmag: 50.0 },
        "ymag stands in for a missing xmag"
    );
}

#[test]
fn polylist_is_fan_triangulated() {
    let xml = one_geometry(
        "0 0 0 1 0 0 1 1 0 0 1 0 5 5 5 6 6 6",
        r##"<polylist count="2"><input semantic="VERTEX" source="#v" offset="0"/><vcount>4 2</vcount><p>0 1 2 3 4 5</p></polylist>"##,
        "",
    );
    let m = import_str(&xml, Path::new(".")).unwrap();
    let prim = &m.meshes[0].primitives[0];
    assert_eq!(prim.kind, PrimitiveKind::Triangles);
    assert_eq!(
        prim.indices.len(),
        6,
        "quad -> 2 triangles; the 2-vertex polygon is skipped"
    );
    let corners: Vec<[f32; 3]> = prim
        .indices
        .iter()
        .map(|&i| prim.positions[i as usize])
        .collect();
    assert_eq!(corners[0], [0.0, 0.0, 0.0]);
    assert_eq!(corners[3], [0.0, 0.0, 0.0], "fan from the first vertex");
    assert_eq!(corners[5], [0.0, 1.0, 0.0]);
}

#[test]
fn uses_the_lowest_texcoord_set_and_accessor_stride() {
    let xml = doc(
        "",
        r##"<library_geometries><geometry id="g"><mesh>
<source id="p"><float_array id="pa" count="12">0 0 0 9 1 0 0 9 0 1 0 9</float_array>
<technique_common><accessor source="#pa" count="3" stride="4"><param name="X" type="float"/><param name="Y" type="float"/><param name="Z" type="float"/><param name="W" type="float"/></accessor></technique_common></source>
<source id="t1"><float_array id="t1a" count="6">9 9 9 9 9 9</float_array><technique_common><accessor source="#t1a" count="3" stride="2"><param name="S" type="float"/><param name="T" type="float"/></accessor></technique_common></source>
<source id="t0"><float_array id="t0a" count="6">0 0 1 0 0 1</float_array><technique_common><accessor source="#t0a" count="3" stride="2"><param name="S" type="float"/><param name="T" type="float"/></accessor></technique_common></source>
<vertices id="v"><input semantic="POSITION" source="#p"/></vertices>
<triangles count="1"><input semantic="VERTEX" source="#v" offset="0"/><input semantic="TEXCOORD" source="#t1" offset="0" set="1"/><input semantic="TEXCOORD" source="#t0" offset="0" set="0"/><p>0 1 2</p></triangles>
</mesh></geometry></library_geometries>"##,
    );
    let m = import_str(&xml, Path::new(".")).unwrap();
    let prim = &m.meshes[0].primitives[0];
    assert_eq!(
        prim.positions,
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
    );
    assert_eq!(prim.texcoords, vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
}

#[test]
fn accepts_documents_without_a_namespace() {
    let xml = r#"<COLLADA version="1.4.1"><asset><unit meter="0.01"/><up_axis>Z_UP</up_axis></asset></COLLADA>"#;
    let m = import_str(xml, Path::new(".")).unwrap();
    assert!(approx(m.unit_meters, 0.01, 1e-12));
    assert_eq!(m.up_axis, UpAxis::Z);
}

#[test]
fn rejects_bad_input() {
    assert!(matches!(
        import_str("<COLLADA><asset>", Path::new(".")),
        Err(ImportError::Xml(_))
    ));
    assert!(matches!(
        import_str("<svg/>", Path::new(".")),
        Err(ImportError::Invalid(_))
    ));

    let out_of_range = one_geometry(
        "0 0 0 1 0 0 0 1 0",
        r##"<triangles count="1"><input semantic="VERTEX" source="#v" offset="0"/><p>0 1 7</p></triangles>"##,
        "",
    );
    match import_str(&out_of_range, Path::new(".")) {
        Err(ImportError::Invalid(msg)) => {
            assert!(msg.contains("\"g\"") || msg.contains(" g"), "{msg}")
        }
        other => panic!("expected Invalid, got {other:?}"),
    }

    let ragged = one_geometry(
        "0 0 0 1 0 0 0 1 0",
        r##"<triangles count="1"><input semantic="VERTEX" source="#v" offset="0"/><p>0 1</p></triangles>"##,
        "",
    );
    assert!(matches!(
        import_str(&ragged, Path::new(".")),
        Err(ImportError::Invalid(_))
    ));

    let dangling = doc(
        "",
        r##"<library_visual_scenes><visual_scene id="s"><node><instance_geometry url="#nope"/></node></visual_scene></library_visual_scenes>"##,
    );
    match import_str(&dangling, Path::new(".")) {
        Err(ImportError::Invalid(msg)) => assert!(msg.contains("nope"), "{msg}"),
        other => panic!("expected Invalid, got {other:?}"),
    }

    let cycle = doc(
        "",
        r##"<library_nodes><node id="a"><instance_node url="#b"/></node><node id="b"><node><instance_node url="#a"/></node></node></library_nodes>
<library_visual_scenes><visual_scene id="s"><node><instance_node url="#a"/></node></visual_scene></library_visual_scenes>"##,
    );
    assert!(matches!(
        import_str(&cycle, Path::new(".")),
        Err(ImportError::Invalid(_))
    ));

    let missing = import_file(Path::new("/definitely/not/here.dae"));
    assert!(matches!(missing, Err(ImportError::Io { .. })));
}
