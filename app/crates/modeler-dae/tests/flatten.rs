// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m0-dae-import: flattening a `Model` into world-space batches. See
//! docs/specs/m0-dae-import.md.

use std::path::Path;

use modeler_dae::{FlatScene, flatten, import_file, import_str};

const INCH: f32 = 0.0254;

fn tables() -> FlatScene {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tables.dae");
    flatten(&import_file(&path).expect("tables.dae imports"))
}

fn close3(a: [f32; 3], b: [f32; 3], eps: f32) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() <= eps)
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[test]
fn expands_every_instance() {
    let s = tables();
    // Two tables x (top 2 + two legs x 12) + glass box 12 + raw triangle 1.
    assert_eq!(s.triangle_count(), 65);
    assert_eq!(s.line_count(), 2);
    assert_eq!(s.materials.len(), 4);
}

#[test]
fn one_batch_per_material_none_last() {
    let s = tables();
    let mats: Vec<Option<usize>> = s.batches.iter().map(|b| b.material).collect();
    assert_eq!(mats, [Some(0), Some(1), Some(2), None]);
    let tris: Vec<usize> = s.batches.iter().map(|b| b.indices.len() / 3).collect();
    assert_eq!(tris, [48, 4, 12, 1]);
    let lines: Vec<Option<usize>> = s.lines.iter().map(|l| l.material).collect();
    assert_eq!(lines, [Some(3)]);
}

#[test]
fn texcoords_only_for_textured_materials() {
    let s = tables();
    for b in &s.batches {
        assert_eq!(b.normals.len(), b.positions.len());
        if b.material == Some(1) {
            assert_eq!(b.texcoords.len(), b.positions.len());
        } else {
            assert!(
                b.texcoords.is_empty(),
                "material {:?} has no texture",
                b.material
            );
        }
    }
}

#[test]
fn bounds_are_in_meters() {
    let b = tables().bounds.expect("non-empty");
    assert!(close3(b.min, [0.0, -20.0 * INCH, 0.0], 1e-5), "{b:?}");
    assert!(
        close3(b.max, [100.0 * INCH, 60.0 * INCH, 40.0 * INCH], 1e-5),
        "{b:?}"
    );
}

#[test]
fn transforms_compose_through_definitions() {
    let s = tables();
    // Table tops (z = 30 in): table 1 spans x 0..50 in, mirrored table 2 spans x 50..100 in.
    let tops = &s.batches[1];
    let xs: Vec<f32> = tops.positions.iter().map(|p| p[0] / INCH).collect();
    let min = xs.iter().cloned().fold(f32::MAX, f32::min);
    let max = xs.iter().cloned().fold(f32::MIN, f32::max);
    assert!(
        (min - 0.0).abs() < 1e-3 && (max - 100.0).abs() < 1e-3,
        "{min}..{max}"
    );
    assert!(
        tops.positions
            .iter()
            .all(|p| (p[2] / INCH - 30.0).abs() < 1e-3)
    );
    // Glass box: <translate> then <scale> doubles its height.
    let glass = &s.batches[2];
    let zmax = glass
        .positions
        .iter()
        .map(|p| p[2])
        .fold(f32::MIN, f32::max);
    assert!((zmax / INCH - 20.0).abs() < 1e-3);
}

#[test]
fn normals_are_unit_and_match_the_winding_even_when_mirrored() {
    let s = tables();
    for b in &s.batches {
        for n in &b.normals {
            assert!(
                (dot(*n, *n).sqrt() - 1.0).abs() < 1e-4,
                "normal {n:?} is not unit length"
            );
        }
        for t in b.indices.chunks(3) {
            let [a, bb, c] = [t[0], t[1], t[2]].map(|i| b.positions[i as usize]);
            let face = cross(sub(bb, a), sub(c, a));
            if dot(face, face) < 1e-12 {
                continue;
            }
            let n = b.normals[t[0] as usize];
            assert!(
                dot(face, n) > 0.0,
                "material {:?}: winding disagrees with normal at {a:?}",
                b.material
            );
        }
    }
}

#[test]
fn generates_face_normals_when_missing() {
    let s = tables();
    let raw = s.batches.last().unwrap();
    assert_eq!(raw.material, None);
    assert_eq!(raw.positions.len(), 3);
    for n in &raw.normals {
        assert!(close3(*n, [0.0, -1.0, 0.0], 1e-5), "{n:?}");
    }
}

fn single_triangle(asset: &str) -> FlatScene {
    let xml = format!(
        r##"<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1"><asset>{asset}</asset>
<library_geometries><geometry id="g"><mesh>
<source id="p"><float_array id="pa" count="9">0 1 0 1 1 0 0 1 2</float_array>
<technique_common><accessor source="#pa" count="3" stride="3"><param name="X" type="float"/><param name="Y" type="float"/><param name="Z" type="float"/></accessor></technique_common></source>
<vertices id="v"><input semantic="POSITION" source="#p"/></vertices>
<triangles count="1"><input semantic="VERTEX" source="#v" offset="0"/><p>0 1 2</p></triangles>
</mesh></geometry></library_geometries>
<library_visual_scenes><visual_scene id="s"><node><instance_geometry url="#g"/></node></visual_scene></library_visual_scenes>
<scene><instance_visual_scene url="#s"/></scene></COLLADA>"##
    );
    flatten(&import_str(&xml, Path::new(".")).unwrap())
}

#[test]
fn converts_up_axis_and_units() {
    let y_up = single_triangle(r#"<unit meter="0.5"/><up_axis>Y_UP</up_axis>"#);
    // (0,1,0) -> (0,0,1) * 0.5 ; (0,1,2) -> (0,-2,1) * 0.5
    let p = &y_up.batches[0].positions;
    assert!(p.iter().any(|q| close3(*q, [0.0, 0.0, 0.5], 1e-6)), "{p:?}");
    assert!(
        p.iter().any(|q| close3(*q, [0.0, -1.0, 0.5], 1e-6)),
        "{p:?}"
    );

    let x_up = single_triangle(r#"<up_axis>X_UP</up_axis>"#);
    // (x,y,z) -> (-y,-z,x): (1,1,0) -> (-1,0,1)
    let p = &x_up.batches[0].positions;
    assert!(
        p.iter().any(|q| close3(*q, [-1.0, 0.0, 1.0], 1e-6)),
        "{p:?}"
    );

    let z_up = single_triangle(r#"<up_axis>Z_UP</up_axis>"#);
    let b = z_up.bounds.unwrap();
    assert!(
        close3(b.min, [0.0, 1.0, 0.0], 1e-6) && close3(b.max, [1.0, 1.0, 2.0], 1e-6),
        "{b:?}"
    );
}

#[test]
fn empty_scene_has_no_bounds() {
    let xml = r#"<COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1"><asset/></COLLADA>"#;
    let s = flatten(&import_str(xml, Path::new(".")).unwrap());
    assert_eq!(s.triangle_count(), 0);
    assert!(s.batches.is_empty() && s.lines.is_empty());
    assert_eq!(s.bounds, None);
}
