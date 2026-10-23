// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Native format version 3: texture sizes and face texture maps
//! (`docs/specs/m4-texture-model.md`, `docs/format.md`).

use glam::{DVec2, DVec4};
use modeler_kernel::format::*;
use modeler_kernel::model::*;
use modeler_kernel::texture::TextureMap;
use modeler_kernel::topology::*;
use proptest::prelude::*;

fn map(a: [f64; 8]) -> TextureMap {
    TextureMap {
        u: DVec4::new(a[0], a[1], a[2], a[3]),
        v: DVec4::new(a[4], a[5], a[6], a[7]),
    }
}

fn read_err(text: &str) -> ReadError {
    match from_text(text) {
        Ok(_) => panic!("read should fail: {text:?}"),
        Err(e) => e,
    }
}

fn kind_at(text: &str, line: usize, kind: ReadErrorKind) {
    assert_eq!(read_err(text), ReadError { line, kind }, "text: {text:?}");
}

/// A triangle with materials and maps on both sides, a second face with only a back map,
/// and a definition with a mapped face.
const V3: &str = r#"modeler-model 3
material 0 1 1 1 1 "Brick" "tex/brick.png" 2 0.5
material 1 0.5 0.5 0.5 1 "Plain" -
material 2 1 1 1 0.5 "Tile" "tile.png" 1 1
definition 0 component "Panel"
body root
vertex 0 0 0 0
vertex 1 1 0 0
vertex 2 1 1 0
vertex 3 0 1 0
edge 0 0 1
edge 1 1 2
edge 2 2 0
edge 3 2 3
edge 4 3 0
face 0 +0 +1 +2
face 1 -2 +3 +4
paint 0 0 2
texmap 0 front 0.5 -0.25 0.125 3 0 2 -0 -0.75
texmap 0 back -1 0 0 0.1 0 1 0 0.0000001
texmap 1 back 0 1 0 0 0 0 1 -2.5
instance 0 0 - 1 0 0 0 0 1 0 0 0 0 1 0 5 0 0 1 "P"
body 0
vertex 0 0 0 0
vertex 1 2 0 0
vertex 2 0 2 0
edge 0 0 1
edge 1 1 2
edge 2 2 0
face 0 +0 +1 +2
paint 0 - 1
texmap 0 front 1 0 0 0 0 1 0 0
end
"#;

#[test]
fn version_is_three() {
    assert_eq!(FORMAT_VERSION, 3);
}

#[test]
fn reads_version_three() {
    let m = from_text(V3).unwrap();
    assert_eq!(
        m.material(MaterialId(0)).unwrap().texture,
        Some(Texture::new("tex/brick.png", DVec2::new(2.0, 0.5)))
    );
    assert_eq!(m.material(MaterialId(1)).unwrap().texture, None);
    assert_eq!(
        m.material(MaterialId(2)).unwrap().texture,
        Some(Texture::new("tile.png", DVec2::ONE))
    );
    let f0 = m.root().face_materials(FaceId(0));
    assert_eq!(f0.front, Some(MaterialId(0)));
    assert_eq!(f0.back, Some(MaterialId(2)));
    assert_eq!(
        f0.front_map,
        Some(map([0.5, -0.25, 0.125, 3.0, 0.0, 2.0, -0.0, -0.75]))
    );
    assert_eq!(
        f0.back_map,
        Some(map([-1.0, 0.0, 0.0, 0.1, 0.0, 1.0, 0.0, 1e-7]))
    );
    assert_eq!(
        m.root().face_materials(FaceId(1)),
        FaceMaterials {
            back_map: Some(map([0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, -2.5])),
            ..Default::default()
        }
    );
    let inside = m.body(BodyId::Definition(DefinitionId(0))).unwrap();
    assert_eq!(
        inside.face_materials(FaceId(0)),
        FaceMaterials {
            back: Some(MaterialId(1)),
            front_map: Some(map([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0])),
            ..Default::default()
        }
    );
    assert!(m.validate().is_empty());
}

#[test]
fn writes_version_three_exactly() {
    let m = from_text(V3).unwrap();
    assert_eq!(to_text(&m).unwrap(), V3);
}

#[test]
fn negative_zero_survives() {
    let m = from_text(V3).unwrap();
    let v = m.root().face_materials(FaceId(0)).front_map.unwrap().v;
    assert!(v.z == 0.0 && v.z.is_sign_negative());
}

#[test]
fn unit_textures_without_maps_stay_version_one() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let mut brick = Material::new("Brick", [1.0, 1.0, 1.0, 1.0]);
    brick.texture = Some(Texture::from("brick.png"));
    m.add_material(brick, &mut log);
    assert_eq!(
        to_text(&m).unwrap(),
        "modeler-model 1\nmaterial 0 1 1 1 1 \"Brick\" \"brick.png\"\nbody root\nend\n"
    );
}

#[test]
fn a_texture_size_makes_version_three() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let mut brick = Material::new("Brick", [1.0, 1.0, 1.0, 1.0]);
    brick.texture = Some(Texture::new("brick.png", DVec2::new(1.0, 0.25)));
    m.add_material(brick, &mut log);
    m.add_material(Material::new("Plain", [0.0, 0.0, 0.0, 1.0]), &mut log);
    assert_eq!(
        to_text(&m).unwrap(),
        "modeler-model 3\nmaterial 0 1 1 1 1 \"Brick\" \"brick.png\" 1 0.25\n\
         material 1 0 0 0 1 \"Plain\" -\nbody root\nend\n"
    );
}

#[test]
fn a_map_makes_version_three_and_stale_maps_are_dropped() {
    // the V3 text without maps but with one on a face that is then removed
    let mut m = from_text(V3).unwrap();
    let mut log = ModelEdit::new();
    for (body, face) in [
        (BodyId::Root, FaceId(0)),
        (BodyId::Root, FaceId(1)),
        (BodyId::Definition(DefinitionId(0)), FaceId(0)),
    ] {
        let mut fm = m.body(body).unwrap().face_materials(face);
        fm.front_map = None;
        fm.back_map = None;
        m.set_face_materials(body, face, fm, &mut log).unwrap();
    }
    let mut brick = m.material(MaterialId(0)).unwrap().clone();
    brick.texture = Some(Texture::from("tex/brick.png"));
    m.set_material(MaterialId(0), brick, &mut log).unwrap();
    let text = to_text(&m).unwrap();
    assert!(text.starts_with("modeler-model 1\n"), "{text}");
    assert!(!text.contains("texmap"));
    assert!(text.contains("material 0 1 1 1 1 \"Brick\" \"tex/brick.png\"\n"));
    // face 1 has no entry left at all, so no paint line either
    assert!(!text.contains("paint 1 "), "{text}");

    // a map on a face that goes away is not written
    let mut fm = m.root().face_materials(FaceId(1));
    fm.front_map = Some(map([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]));
    m.set_face_materials(BodyId::Root, FaceId(1), fm, &mut log)
        .unwrap();
    assert!(to_text(&m).unwrap().starts_with("modeler-model 3\n"));
    m.edit_geometry(BodyId::Root, &mut log, |ctx, log| {
        ctx.delete_face(FaceId(1), log)
    })
    .unwrap()
    .unwrap();
    let text = to_text(&m).unwrap();
    assert!(text.starts_with("modeler-model 1\n"), "{text}");
    assert!(!text.contains("texmap"));
}

#[test]
fn guides_and_unit_textures_stay_version_two() {
    let text = "modeler-model 2\nmaterial 0 1 1 1 1 \"B\" \"b.png\"\nbody root\n\
                guide 0 point 1 2 3\nend\n";
    let m = from_text(text).unwrap();
    assert_eq!(to_text(&m).unwrap(), text);
}

#[test]
fn old_versions_read_textures_as_one_meter() {
    for version in [1, 2] {
        let text = format!("modeler-model {version}\nmaterial 0 1 1 1 1 \"B\" \"b.png\"\nend\n");
        let m = from_text(&text).unwrap();
        assert_eq!(
            m.material(MaterialId(0)).unwrap().texture,
            Some(Texture::new("b.png", DVec2::ONE))
        );
    }
}

#[test]
fn old_versions_refuse_sizes_and_texmaps() {
    for version in [1, 2] {
        kind_at(
            &format!("modeler-model {version}\nmaterial 0 1 1 1 1 \"B\" \"b.png\" 2 2\nend\n"),
            2,
            ReadErrorKind::BadField,
        );
        kind_at(
            &format!(
                "modeler-model {version}\nbody root\nvertex 0 0 0 0\nvertex 1 1 0 0\n\
                 vertex 2 0 1 0\nedge 0 0 1\nedge 1 1 2\nedge 2 2 0\nface 0 +0 +1 +2\n\
                 texmap 0 front 1 0 0 0 0 1 0 0\nend\n"
            ),
            10,
            ReadErrorKind::UnknownRecord,
        );
    }
}

const TRI: &str = "body root\nvertex 0 0 0 0\nvertex 1 1 0 0\nvertex 2 0 1 0\nedge 0 0 1\n\
                   edge 1 1 2\nedge 2 2 0\nface 0 +0 +1 +2\n";

/// `record` as line 10, after a v3 header and a triangle face 0 in the root.
fn after_triangle(record: &str) -> String {
    format!("modeler-model 3\n{TRI}{record}\nend\n")
}

#[test]
fn texmap_errors() {
    let bad = [
        "texmap 0 front 1 0 0 0 0 1 0",
        "texmap 0 front 1 0 0 0 0 1 0 0 0",
        "texmap 0 side 1 0 0 0 0 1 0 0",
        "texmap 0 \"front\" 1 0 0 0 0 1 0 0",
        "texmap 0 front 1 0 0 x 0 1 0 0",
        "texmap 0 front 1 0 0 inf 0 1 0 0",
        "texmap 0 back 1 0 0 0 0 1 0 NaN",
        "texmap x front 1 0 0 0 0 1 0 0",
    ];
    for record in bad {
        kind_at(&after_triangle(record), 10, ReadErrorKind::BadField);
    }
    kind_at(
        &after_triangle("texmap 4 front 1 0 0 0 0 1 0 0"),
        10,
        ReadErrorKind::Rejected(ModelError::Topo(TopoError::MissingFace(FaceId(4)))),
    );
    kind_at(
        "modeler-model 3\ntexmap 0 front 1 0 0 0 0 1 0 0\nend\n",
        2,
        ReadErrorKind::NoBody,
    );
}

#[test]
fn texmap_keeps_paint_and_the_other_side() {
    let text = format!(
        "modeler-model 3\nmaterial 0 1 0 0 1 \"R\" -\n{TRI}paint 0 0 -\n\
         texmap 0 back 0 0 1 0 1 0 0 0\nend\n"
    );
    let m = from_text(&text).unwrap();
    assert_eq!(
        m.root().face_materials(FaceId(0)),
        FaceMaterials {
            front: Some(MaterialId(0)),
            back_map: Some(map([0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0])),
            ..Default::default()
        }
    );
    assert_eq!(to_text(&m).unwrap(), text);
}

#[test]
fn v3_material_record_shapes() {
    let bad = [
        "material 0 1 1 1 1 \"B\" \"b.png\"",
        "material 0 1 1 1 1 \"B\" \"b.png\" 2",
        "material 0 1 1 1 1 \"B\" - 2 2",
        "material 0 1 1 1 1 \"B\" \"b.png\" 2 2 2",
        "material 0 1 1 1 1 \"B\" \"b.png\" x 2",
        "material 0 1 1 1 1 \"B\" \"b.png\" 2 inf",
    ];
    for record in bad {
        kind_at(
            &format!("modeler-model 3\n{record}\nend\n"),
            2,
            ReadErrorKind::BadField,
        );
    }
    for size in ["0 1", "1 -2", "-0 1"] {
        kind_at(
            &format!("modeler-model 3\nmaterial 0 1 1 1 1 \"B\" \"b.png\" {size}\nend\n"),
            2,
            ReadErrorKind::Rejected(ModelError::BadTexture),
        );
    }
}

fn finite() -> impl Strategy<Value = f64> {
    prop_oneof![
        -1e6f64..1e6,
        Just(0.0),
        Just(-0.0),
        Just(1e-300),
        Just(0.1),
        Just(f64::MAX),
    ]
}

fn any_map() -> impl Strategy<Value = Option<TextureMap>> {
    proptest::option::of(proptest::array::uniform8(finite()).prop_map(map))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn round_trip_is_exact(
        sizes in proptest::collection::vec(
            proptest::option::of((1e-6f64..1e4, 1e-6f64..1e4)), 1..4),
        maps in proptest::collection::vec((any_map(), any_map(), 0usize..5, 0usize..5), 2),
    ) {
        let mut m = from_text(&format!("modeler-model 3\n{TRI}end\n")).unwrap();
        let mut log = ModelEdit::new();
        let mut ids = Vec::new();
        for (i, size) in sizes.iter().enumerate() {
            let mut mat = Material::new(&format!("M{i}"), [0.5, 0.25, 1.0, 1.0]);
            mat.texture = size.map(|(w, h)| Texture::new(&format!("t{i}.png"), DVec2::new(w, h)));
            ids.push(m.add_material(mat, &mut log));
        }
        let pick = |k: usize| if k < ids.len() { Some(ids[k]) } else { None };
        let (front_map, back_map, front, back) = maps[0];
        m.set_face_materials(
            BodyId::Root,
            FaceId(0),
            FaceMaterials { front: pick(front), back: pick(back), front_map, back_map },
            &mut log,
        ).unwrap();
        let text = to_text(&m).unwrap();
        let back_in = from_text(&text).unwrap();
        prop_assert_eq!(&back_in, &m);
        prop_assert_eq!(to_text(&back_in).unwrap(), text.clone());
        let needs_three = sizes.iter().flatten().any(|&(w, h)| (w, h) != (1.0, 1.0))
            || front_map.is_some()
            || back_map.is_some();
        prop_assert_eq!(text.starts_with("modeler-model 3\n"), needs_three, "{}", text);
        // bit-exact numbers, including -0
        let got = back_in.root().face_materials(FaceId(0));
        for (a, b) in [(got.front_map, front_map), (got.back_map, back_map)] {
            if let (Some(a), Some(b)) = (a, b) {
                for k in 0..4 {
                    prop_assert_eq!(a.u[k].to_bits(), b.u[k].to_bits());
                    prop_assert_eq!(a.v[k].to_bits(), b.v[k].to_bits());
                }
            }
        }
    }
}
