// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! STL to kernel (`docs/specs/m7-import-stl.md`).

use std::path::PathBuf;

use glam::DVec3;
use modeler_kernel::model::{BodyId, Model};
use modeler_mesh::{MeshObject, import_stl, to_kernel};

type Tri = [[f32; 3]; 3];

/// The 12 triangles of the box `lo..hi`, corners counter-clockwise seen from outside.
fn box_triangles(lo: [f32; 3], hi: [f32; 3]) -> Vec<Tri> {
    let p = |i: usize| -> [f32; 3] {
        [
            if i & 1 != 0 { hi[0] } else { lo[0] },
            if i & 2 != 0 { hi[1] } else { lo[1] },
            if i & 4 != 0 { hi[2] } else { lo[2] },
        ]
    };
    // Quads (outward), split into two triangles each.
    let quads = [
        [0, 2, 3, 1], // z = lo
        [4, 5, 7, 6], // z = hi
        [0, 1, 5, 4], // y = lo
        [2, 6, 7, 3], // y = hi
        [0, 4, 6, 2], // x = lo
        [1, 3, 7, 5], // x = hi
    ];
    let mut out = Vec::new();
    for q in quads {
        out.push([p(q[0]), p(q[1]), p(q[2])]);
        out.push([p(q[0]), p(q[2]), p(q[3])]);
    }
    out
}

fn object(name: &str, triangles: Vec<Tri>) -> MeshObject {
    MeshObject {
        name: name.into(),
        triangles,
    }
}

/// The groups in the root, in instance order: (definition body, name).
fn groups(model: &Model) -> Vec<(BodyId, String)> {
    model
        .root()
        .instances()
        .map(|(_, inst)| {
            let def = model.definition(inst.definition).unwrap();
            assert!(def.group, "{} should be a group", def.name);
            (BodyId::Definition(inst.definition), def.name.clone())
        })
        .collect()
}

fn counts(model: &Model, body: BodyId) -> (usize, usize, usize) {
    let c = model.body(body).unwrap().geometry();
    (c.vertex_count(), c.edge_count(), c.face_count())
}

/// Every face of a convex body points away from the body's centre.
fn outward(model: &Model, body: BodyId) -> bool {
    let c = model.body(body).unwrap().geometry();
    let pts: Vec<DVec3> = c.vertices().map(|(_, v)| v.position).collect();
    let centre = pts.iter().copied().sum::<DVec3>() / pts.len() as f64;
    c.faces().all(|(f, face)| {
        let plane = c.face_plane(f).unwrap();
        let ps = c.loop_points(&face.outer);
        let mid = ps.iter().copied().sum::<DVec3>() / ps.len() as f64;
        plane.normal.dot(mid - centre) > 0.0
    })
}

fn extent(model: &Model, body: BodyId) -> (DVec3, DVec3) {
    let c = model.body(body).unwrap().geometry();
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    for (_, v) in c.vertices() {
        lo = lo.min(v.position);
        hi = hi.max(v.position);
    }
    (lo, hi)
}

fn near(a: DVec3, b: DVec3) -> bool {
    a.distance(b) < 1e-9
}

#[test]
fn cube_becomes_a_group_of_six_faces() {
    let model = to_kernel(&[object("cube", box_triangles([0.0; 3], [10.0; 3]))], 0.001).unwrap();
    assert!(model.validate().is_empty(), "{:?}", model.validate());
    let g = groups(&model);
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].1, "cube");
    assert_eq!(counts(&model, g[0].0), (8, 12, 6));
    assert!(outward(&model, g[0].0));
    let (lo, hi) = extent(&model, g[0].0);
    assert!(near(lo, DVec3::ZERO), "{lo:?}");
    assert!(near(hi, DVec3::splat(0.01)), "{hi:?}");
    // The root holds only the group.
    assert_eq!(counts(&model, BodyId::Root), (0, 0, 0));
}

#[test]
fn units_scale_the_file() {
    let model = to_kernel(
        &[object("b", box_triangles([1.0, 2.0, 3.0], [2.0, 4.0, 6.0]))],
        0.0254,
    )
    .unwrap();
    let g = groups(&model);
    let (lo, hi) = extent(&model, g[0].0);
    assert!(near(lo, DVec3::new(1.0, 2.0, 3.0) * 0.0254), "{lo:?}");
    assert!(near(hi, DVec3::new(2.0, 4.0, 6.0) * 0.0254), "{hi:?}");
}

#[test]
fn z_stays_up() {
    // A box tall in z stays tall in z (no Y-up turn).
    let model = to_kernel(
        &[object("tall", box_triangles([0.0; 3], [1.0, 1.0, 5.0]))],
        1.0,
    )
    .unwrap();
    let g = groups(&model);
    let (lo, hi) = extent(&model, g[0].0);
    assert!(near(hi - lo, DVec3::new(1.0, 1.0, 5.0)), "{:?}", hi - lo);
}

#[test]
fn objects_become_groups_in_order_and_empty_ones_are_skipped() {
    let model = to_kernel(
        &[
            object("first", box_triangles([0.0; 3], [1.0; 3])),
            object("nothing", vec![]),
            object("second", box_triangles([3.0, 0.0, 0.0], [4.0, 2.0, 1.0])),
            object("first", box_triangles([0.0, 5.0, 0.0], [1.0, 6.0, 1.0])),
        ],
        1.0,
    )
    .unwrap();
    assert!(model.validate().is_empty());
    let names: Vec<String> = groups(&model).into_iter().map(|(_, n)| n).collect();
    assert_eq!(names, ["first", "second", "first"]);
    for (body, _) in groups(&model) {
        assert_eq!(counts(&model, body), (8, 12, 6));
    }
}

#[test]
fn no_objects_is_an_empty_model() {
    let model = to_kernel(&[], 0.001).unwrap();
    assert!(model.root().is_empty());
    assert_eq!(model.definitions().count(), 0);
}

/// An L-shaped prism from a fan-triangulated outline: the concave end faces merge back.
#[test]
fn concave_faces_merge_back() {
    let l: [[f32; 2]; 6] = [
        [0.0, 0.0],
        [3.0, 0.0],
        [3.0, 1.0],
        [1.0, 1.0],
        [1.0, 2.0],
        [0.0, 2.0],
    ];
    let at = |i: usize, z: f32| [l[i][0], l[i][1], z];
    let mut tris = Vec::new();
    // Bottom (outward = down) and top, as ear-free triangles that stay inside the L.
    for [a, b, c] in [[0, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 5]] {
        tris.push([at(a, 0.0), at(c, 0.0), at(b, 0.0)]);
        tris.push([at(a, 2.0), at(b, 2.0), at(c, 2.0)]);
    }
    for i in 0..6 {
        let j = (i + 1) % 6;
        tris.push([at(i, 0.0), at(j, 0.0), at(j, 2.0)]);
        tris.push([at(i, 0.0), at(j, 2.0), at(i, 2.0)]);
    }
    let model = to_kernel(&[object("ell", tris)], 1.0).unwrap();
    assert!(model.validate().is_empty());
    let g = groups(&model);
    assert_eq!(counts(&model, g[0].0), (12, 18, 8));
}

fn temp_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("modeler-mesh-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn ascii(name: &str, tris: &[Tri]) -> String {
    let mut s = format!("solid {name}\n");
    for t in tris {
        s.push_str("facet normal 0 0 0\nouter loop\n");
        for c in t {
            s.push_str(&format!("vertex {} {} {}\n", c[0], c[1], c[2]));
        }
        s.push_str("endloop\nendfacet\n");
    }
    s.push_str(&format!("endsolid {name}\n"));
    s
}

#[test]
fn import_names_unnamed_solids_after_the_file() {
    let path = temp_path("bracket.stl");
    std::fs::write(&path, ascii("", &box_triangles([0.0; 3], [20.0; 3]))).unwrap();
    let model = import_stl(&path, 0.001).unwrap();
    let g = groups(&model);
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].1, "bracket");
    let (_, hi) = extent(&model, g[0].0);
    assert!(near(hi, DVec3::splat(0.02)), "{hi:?}");
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn import_keeps_solid_names() {
    let path = temp_path("two.stl");
    let mut text = ascii("left", &box_triangles([0.0; 3], [1.0; 3]));
    text.push_str(&ascii(
        "right",
        &box_triangles([2.0, 0.0, 0.0], [3.0, 1.0, 1.0]),
    ));
    std::fs::write(&path, text).unwrap();
    let model = import_stl(&path, 1.0).unwrap();
    let names: Vec<String> = groups(&model).into_iter().map(|(_, n)| n).collect();
    assert_eq!(names, ["left", "right"]);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn import_errors_name_the_file() {
    let missing = temp_path("missing.stl");
    let err = import_stl(&missing, 0.001).unwrap_err();
    assert!(
        err.starts_with(&format!("{}: ", missing.display())),
        "{err}"
    );
    assert!(!err.contains('\n'));

    let bad = temp_path("bad.stl");
    std::fs::write(&bad, "solid s\nfacet normal 0 0 1\nvertex 0 0 0\n").unwrap();
    let err = import_stl(&bad, 0.001).unwrap_err();
    assert!(
        err.starts_with(&format!("{}: line 3: ", bad.display())),
        "{err}"
    );

    let junk = temp_path("junk.stl");
    std::fs::write(&junk, "hello").unwrap();
    let err = import_stl(&junk, 0.001).unwrap_err();
    assert_eq!(err, format!("{}: not an STL file", junk.display()));
    for p in [bad, junk] {
        std::fs::remove_file(p).unwrap();
    }
}
