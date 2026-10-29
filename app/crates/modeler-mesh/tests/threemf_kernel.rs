// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! 3MF to kernel (`docs/specs/m7-import-3mf.md`).

use std::path::PathBuf;

use glam::DVec3;
use modeler_kernel::model::BodyId;
use modeler_mesh::import_3mf;

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("modeler-3mf-{}-{name}", std::process::id()))
}

/// A stored-only archive holding one model part: a 20 mm cube named "Cube", in millimeters.
fn cube_3mf() -> Vec<u8> {
    let verts = [
        (0, 0, 0),
        (20, 0, 0),
        (20, 20, 0),
        (0, 20, 0),
        (0, 0, 20),
        (20, 0, 20),
        (20, 20, 20),
        (0, 20, 20),
    ];
    let mut v = String::new();
    for (x, y, z) in verts {
        v.push_str(&format!("<vertex x=\"{x}\" y=\"{y}\" z=\"{z}\"/>"));
    }
    let quads = [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [3, 7, 6, 2],
        [0, 4, 7, 3],
        [1, 2, 6, 5],
    ];
    let mut t = String::new();
    for q in quads {
        t.push_str(&format!(
            "<triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/><triangle v1=\"{}\" v2=\"{}\" v3=\"{}\"/>",
            q[0], q[1], q[2], q[0], q[2], q[3]
        ));
    }
    let xml = format!(
        "<model unit=\"millimeter\"><resources><object id=\"1\" name=\"Cube\"><mesh>\
         <vertices>{v}</vertices><triangles>{t}</triangles></mesh></object></resources>\
         <build><item objectid=\"1\"/></build></model>"
    );
    let name = "3D/3dmodel.model";
    let mut out = Vec::new();
    let le16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
    let le32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    out.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
    le16(&mut out, 20);
    le16(&mut out, 0);
    le16(&mut out, 0);
    le32(&mut out, 0);
    le32(&mut out, 0);
    le32(&mut out, xml.len() as u32);
    le32(&mut out, xml.len() as u32);
    le16(&mut out, name.len() as u16);
    le16(&mut out, 0);
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(xml.as_bytes());
    let start = out.len() as u32;
    let mut c = Vec::new();
    c.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
    le16(&mut c, 20);
    le16(&mut c, 20);
    le16(&mut c, 0);
    le16(&mut c, 0);
    le32(&mut c, 0);
    le32(&mut c, 0);
    le32(&mut c, xml.len() as u32);
    le32(&mut c, xml.len() as u32);
    le16(&mut c, name.len() as u16);
    for _ in 0..4 {
        le16(&mut c, 0);
    }
    le32(&mut c, 0);
    le32(&mut c, 0);
    c.extend_from_slice(name.as_bytes());
    out.extend_from_slice(&c);
    out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 1, 0, 1, 0]);
    le32(&mut out, c.len() as u32);
    le32(&mut out, start);
    le16(&mut out, 0);
    out
}

#[test]
fn import_makes_one_named_group_in_the_files_unit() {
    let path = temp_path("cube.3mf");
    std::fs::write(&path, cube_3mf()).unwrap();
    let model = import_3mf(&path).unwrap();
    assert!(model.validate().is_empty(), "{:?}", model.validate());
    let groups: Vec<_> = model.root().instances().collect();
    assert_eq!(groups.len(), 1);
    let def = model.definition(groups[0].1.definition).unwrap();
    assert!(def.group);
    assert_eq!(def.name, "Cube");
    // A 20 mm cube: twelve triangles merged into six faces, facing outward.
    let body = BodyId::Definition(groups[0].1.definition);
    let c = model.body(body).unwrap().geometry();
    assert_eq!(
        (c.vertex_count(), c.edge_count(), c.face_count()),
        (8, 12, 6)
    );
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    for (_, v) in c.vertices() {
        hi = hi.max(v.position);
    }
    assert!(hi.distance(DVec3::splat(0.02)) < 1e-9, "{hi:?}");
    let centre = DVec3::splat(0.01);
    assert!(c.faces().all(|(f, face)| {
        let ps = c.loop_points(&face.outer);
        let mid = ps.iter().copied().sum::<DVec3>() / ps.len() as f64;
        c.face_plane(f).unwrap().normal.dot(mid - centre) > 0.0
    }));
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn import_errors_name_the_file() {
    let missing = temp_path("missing.3mf");
    let err = import_3mf(&missing).unwrap_err();
    assert!(
        err.starts_with(&format!("{}: ", missing.display())),
        "{err}"
    );

    let bad = temp_path("bad.3mf");
    std::fs::write(&bad, "not a zip at all").unwrap();
    let err = import_3mf(&bad).unwrap_err();
    assert_eq!(err, format!("{}: not a 3MF file", bad.display()));
    std::fs::remove_file(&bad).unwrap();
}
