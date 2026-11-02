// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Binary STL writer (`docs/specs/m7-export-stl.md`).

use std::path::PathBuf;

use glam::DVec3;
use modeler_kernel::model::*;
use modeler_mesh::print_mesh::*;
use modeler_mesh::stl;
use modeler_mesh::stl_write::*;
use modeler_mesh::{import_stl, write_combined as combined_reexport};

mod print_common;
use print_common::*;

fn dir(test: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("modeler-stlw-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn object(name: &str, triangles: Vec<Triangle>) -> PrintObject {
    PrintObject {
        name: name.into(),
        color: None,
        triangles,
    }
}

/// The triangles of a box group of the given size at the origin, via the real export.
fn box_triangles(lo: DVec3, hi: DVec3) -> Vec<Triangle> {
    let mut model = Model::new();
    let (def, _, _) = box_group(&mut model, "b", lo, hi);
    place(&mut model, BodyId::Root, def, glam::DMat4::IDENTITY, "b");
    let mesh = print_mesh(&model, PrintScope::Model, &PrintOptions::default());
    assert_eq!(mesh.objects.len(), 1);
    mesh.objects[0].triangles.clone()
}

fn tri() -> Triangle {
    [v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0), v(0.0, 3.0, 0.0)]
}

fn f32_at(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

#[test]
fn encode_layout_is_exact() {
    let other: Triangle = [v(1.5, -2.25, 3.0), v(1.5, -2.25, 4.0), v(1.5, 5.0, 3.0)];
    let bytes = encode("Bracket", &[tri(), other]);
    assert_eq!(bytes.len(), 84 + 2 * 50);
    assert!(bytes[..13].starts_with(b"Drydock STL: "));
    assert_eq!(&bytes[13..20], b"Bracket");
    assert!(bytes[20..80].iter().all(|b| *b == 0));
    assert_eq!(u32::from_le_bytes(bytes[80..84].try_into().unwrap()), 2);
    // First record: normal +z, then the three corners in order, attribute 0.
    let r = &bytes[84..134];
    assert_eq!([f32_at(r, 0), f32_at(r, 4), f32_at(r, 8)], [0.0, 0.0, 1.0]);
    assert_eq!(
        [f32_at(r, 12), f32_at(r, 16), f32_at(r, 20)],
        [0.0, 0.0, 0.0]
    );
    assert_eq!(
        [f32_at(r, 24), f32_at(r, 28), f32_at(r, 32)],
        [2.0, 0.0, 0.0]
    );
    assert_eq!(
        [f32_at(r, 36), f32_at(r, 40), f32_at(r, 44)],
        [0.0, 3.0, 0.0]
    );
    assert_eq!(&r[48..50], &[0, 0]);
    // Second record: normal from the winding, unit length, values as given.
    let r = &bytes[134..184];
    let n = [f32_at(r, 0), f32_at(r, 4), f32_at(r, 8)];
    assert_eq!(n[0], -1.0);
    assert_eq!(n[1].abs() + n[2].abs(), 0.0);
    assert_eq!(f32_at(r, 12), 1.5);
    assert_eq!(f32_at(r, 16), -2.25);
    assert_eq!(&r[48..50], &[0, 0]);
}

#[test]
fn normal_follows_winding_and_is_unit_length() {
    let t = tri();
    let flipped = [t[0], t[2], t[1]];
    let skew: Triangle = [v(0.0, 0.0, 0.0), v(4.0, 0.0, 0.0), v(0.0, 4.0, 4.0)];
    let bytes = encode("n", &[flipped, skew]);
    assert_eq!(f32_at(&bytes[84..], 8), -1.0);
    let r = &bytes[134..];
    let n = DVec3::new(
        f32_at(r, 0) as f64,
        f32_at(r, 4) as f64,
        f32_at(r, 8) as f64,
    );
    assert!((n.length() - 1.0).abs() < 1e-6);
    assert!(n.x.abs() < 1e-6 && (n.y + n.z).abs() < 1e-6 && n.y < 0.0);
}

#[test]
fn degenerate_triangle_gets_zero_normal() {
    let line: Triangle = [v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(2.0, 0.0, 0.0)];
    let bytes = encode("d", &[line]);
    assert_eq!(bytes.len(), 134);
    for k in 0..3 {
        assert_eq!(f32_at(&bytes[84..], 4 * k), 0.0);
    }
}

#[test]
fn encode_header_is_cut_on_a_character_boundary() {
    let long = "é".repeat(100);
    let bytes = encode(&long, &[tri()]);
    assert_eq!(bytes.len(), 134);
    let header = &bytes[..80];
    let end = header.iter().rposition(|b| *b != 0).unwrap() + 1;
    assert!(std::str::from_utf8(&header[..end]).is_ok());
    assert!(end > 70);
}

#[test]
fn encode_header_never_starts_with_solid() {
    assert!(!encode("solid", &[tri()]).starts_with(b"solid"));
}

#[test]
fn encode_of_nothing_is_a_valid_empty_file() {
    let bytes = encode("e", &[]);
    assert_eq!(bytes.len(), 84);
    assert_eq!(u32::from_le_bytes(bytes[80..84].try_into().unwrap()), 0);
}

#[test]
fn sanitize_rules() {
    assert_eq!(sanitize_name("Bracket 2"), "Bracket 2");
    assert_eq!(
        sanitize_name("a/b\\c:d*e?f\"g<h>i|j"),
        "a_b_c_d_e_f_g_h_i_j"
    );
    assert_eq!(sanitize_name("  padded  "), "padded");
    assert_eq!(sanitize_name("../../etc"), "______etc");
    assert_eq!(sanitize_name("Möbius-1_x"), "Möbius-1_x");
    assert_eq!(sanitize_name(""), "part");
    assert_eq!(sanitize_name("   "), "part");
    assert_eq!(sanitize_name(&"x".repeat(250)).chars().count(), 100);
    assert_eq!(sanitize_name("a.b"), "a_b");
}

#[test]
fn separate_writes_one_file_per_object_in_order() {
    let d = dir("separate");
    let a = box_triangles(v(0.0, 0.0, 0.0), v(10.0, 20.0, 30.0));
    let b = box_triangles(v(0.0, 0.0, 0.0), v(5.0, 5.0, 5.0));
    let objects = vec![object("Base", a.clone()), object("Lid", b.clone())];
    let paths = write_separate(&objects, &d).unwrap();
    assert_eq!(paths, vec![d.join("Base.stl"), d.join("Lid.stl")]);
    for (path, tris) in paths.iter().zip([&a, &b]) {
        let read = stl::read(&std::fs::read(path).unwrap(), "x").unwrap();
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].triangles.len(), tris.len());
    }
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 2);
}

#[test]
fn separate_numbers_duplicates_and_sanitises() {
    let d = dir("dupes");
    let objects = vec![
        object("Part", vec![tri()]),
        object("part", vec![tri()]),
        object("Part", vec![tri()]),
        object("Part-2", vec![tri()]),
        object("a/b", vec![tri()]),
        object("", vec![tri()]),
    ];
    let paths = write_separate(&objects, &d).unwrap();
    let names: Vec<String> = paths
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names[0], "Part.stl");
    assert_eq!(names[1], "part-2.stl");
    assert_eq!(names[2], "Part-3.stl");
    assert_eq!(names[4], "a_b.stl");
    assert_eq!(names[3], "Part-2-2.stl");
    assert_eq!(names[5], "part-4.stl");
    // "Part-2" collides with the number given to the second "part": it gets another one.
    let mut lower: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
    lower.sort();
    lower.dedup();
    assert_eq!(lower.len(), 6);
    assert!(paths.iter().all(|p| p.starts_with(&d) && p.exists()));
}

#[test]
fn separate_creates_the_folder_and_skips_empty_objects() {
    let d = dir("nested").join("a").join("b");
    let objects = vec![object("Empty", vec![]), object("Full", vec![tri()])];
    let paths = write_separate(&objects, &d).unwrap();
    assert_eq!(paths, vec![d.join("Full.stl")]);
}

#[test]
fn separate_overwrites_existing_files() {
    let d = dir("overwrite");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("Full.stl"), b"old").unwrap();
    write_separate(&[object("Full", vec![tri()])], &d).unwrap();
    assert_eq!(std::fs::read(d.join("Full.stl")).unwrap().len(), 134);
}

#[test]
fn empty_input_is_refused_and_writes_nothing() {
    let d = dir("empty");
    assert_eq!(write_separate(&[], &d), Err(StlWriteError::NothingToWrite));
    assert_eq!(
        write_separate(&[object("E", vec![])], &d),
        Err(StlWriteError::NothingToWrite)
    );
    assert!(!d.exists());
    let file = dir("empty-file").join("all.stl");
    assert_eq!(
        write_combined(&[], &file),
        Err(StlWriteError::NothingToWrite)
    );
    assert_eq!(
        write_combined(&[object("E", vec![])], &file),
        Err(StlWriteError::NothingToWrite)
    );
    assert!(!file.exists());
}

#[test]
fn combined_holds_every_triangle_in_object_order() {
    let d = dir("combined");
    std::fs::create_dir_all(&d).unwrap();
    let path = d.join("all.stl");
    let objects = vec![
        object("Empty", vec![]),
        object("One", vec![tri()]),
        object("Two", vec![tri(), tri()]),
    ];
    combined_reexport(&objects, &path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(bytes.len(), 84 + 3 * 50);
    assert_eq!(&bytes[13..16], b"One");
    assert_eq!(u32::from_le_bytes(bytes[80..84].try_into().unwrap()), 3);
}

#[test]
fn io_failure_is_reported() {
    let d = dir("io");
    std::fs::create_dir_all(&d).unwrap();
    // A file where the folder should be.
    let blocker = d.join("blocker");
    std::fs::write(&blocker, b"x").unwrap();
    let err = write_separate(&[object("A", vec![tri()])], &blocker.join("sub")).unwrap_err();
    assert!(matches!(err, StlWriteError::Io(_)));
    let err = write_combined(&[object("A", vec![tri()])], &blocker.join("all.stl")).unwrap_err();
    assert!(matches!(err, StlWriteError::Io(_)));
}

#[test]
fn roundtrip_keeps_volume_and_bounding_box() {
    let d = dir("roundtrip");
    let lo = v(-12.5, 3.0, 0.25);
    let hi = v(37.5, 23.0, 40.25);
    let tris = box_triangles(lo * 0.001, hi * 0.001);
    // box_group is in meters; the print mesh is in mm.
    assert_rel(volume(&tris), 50.0 * 20.0 * 40.0, 1e-6);
    let paths = write_separate(&[object("Block", tris.clone())], &d).unwrap();
    let model = import_stl(&paths[0], 0.001).unwrap();
    let back = print_mesh(&model, PrintScope::Model, &PrintOptions::default());
    assert_eq!(back.objects.len(), 1);
    let again = &back.objects[0].triangles;
    assert_rel(volume(again), volume(&tris), 1e-5);
    let (l0, h0) = bounds(&tris);
    let (l1, h1) = bounds(again);
    assert_close(l0, l1, 1e-4);
    assert_close(h0, h1, 1e-4);
}

#[test]
fn combined_roundtrip_keeps_both_solids() {
    let d = dir("combined-roundtrip");
    std::fs::create_dir_all(&d).unwrap();
    let a = box_triangles(v(0.0, 0.0, 0.0), v(0.01, 0.01, 0.01));
    let b = box_triangles(v(0.02, 0.0, 0.0), v(0.03, 0.02, 0.01));
    let path = d.join("both.stl");
    write_combined(&[object("A", a.clone()), object("B", b.clone())], &path).unwrap();
    let read = stl::read(&std::fs::read(&path).unwrap(), "x").unwrap();
    assert_eq!(read[0].triangles.len(), a.len() + b.len());
}
