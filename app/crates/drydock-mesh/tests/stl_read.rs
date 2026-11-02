// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! STL reader (`docs/specs/m7-import-stl.md`).

use drydock_mesh::MeshObject;
use drydock_mesh::stl::{StlError, read};

type Tri = [[f32; 3]; 3];

/// A binary STL with the given header text (padded with zeros to 80 bytes).
fn binary(header: &str, tris: &[Tri]) -> Vec<u8> {
    let mut out = vec![0u8; 80];
    out[..header.len()].copy_from_slice(header.as_bytes());
    out.extend_from_slice(&(tris.len() as u32).to_le_bytes());
    for t in tris {
        // A deliberately wrong normal: the reader must ignore it.
        for x in [0.0f32, 0.0, 9.0] {
            out.extend_from_slice(&x.to_le_bytes());
        }
        for c in t {
            for x in c {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        out.extend_from_slice(&[0x12, 0x34]);
    }
    out
}

const T1: Tri = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
const T2: Tri = [[1.5, -2.25, 3.0], [1e-3, 4.0e2, -0.0], [7.0, 8.0, 9.0]];

#[test]
fn binary_triangles_in_order() {
    let objs = read(&binary("exported", &[T1, T2]), "part").unwrap();
    assert_eq!(
        objs,
        vec![MeshObject {
            name: "part".into(),
            triangles: vec![T1, T2],
        }]
    );
}

#[test]
fn binary_keeps_corners_bit_for_bit() {
    let odd: Tri = [
        [f32::MIN_POSITIVE, 1.0e-30, 3.4e38],
        [-123.456_79, 0.1, 0.2],
        [1.0 / 3.0, 2.0 / 3.0, -7.0e-7],
    ];
    let objs = read(&binary("", &[odd]), "x").unwrap();
    let got = objs[0].triangles[0];
    for i in 0..3 {
        for j in 0..3 {
            assert_eq!(got[i][j].to_bits(), odd[i][j].to_bits());
        }
    }
}

#[test]
fn binary_header_starting_with_solid_is_still_binary() {
    let objs = read(&binary("solid cube made by some exporter", &[T1]), "p").unwrap();
    assert_eq!(objs.len(), 1);
    assert_eq!(objs[0].name, "p");
    assert_eq!(objs[0].triangles, vec![T1]);
}

#[test]
fn binary_with_no_triangles() {
    let objs = read(&binary("", &[]), "empty").unwrap();
    assert_eq!(
        objs,
        vec![MeshObject {
            name: "empty".into(),
            triangles: vec![],
        }]
    );
}

#[test]
fn binary_cut_short_is_truncated() {
    let mut bytes = binary("", &[T1, T2]);
    bytes.truncate(bytes.len() - 7);
    assert_eq!(
        read(&bytes, "p"),
        Err(StlError::Truncated {
            expected: 84 + 100,
            actual: 84 + 100 - 7,
        })
    );
}

#[test]
fn binary_with_extra_bytes_is_truncated_too() {
    let mut bytes = binary("", &[T1]);
    bytes.extend_from_slice(&[0, 0, 0]);
    assert_eq!(
        read(&bytes, "p"),
        Err(StlError::Truncated {
            expected: 134,
            actual: 137,
        })
    );
}

#[test]
fn huge_count_does_not_overflow() {
    let mut bytes = vec![0u8; 84];
    bytes[80..84].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        read(&bytes, "p"),
        Err(StlError::Truncated {
            expected: 84 + 50 * u32::MAX as usize,
            actual: 84,
        })
    );
}

#[test]
fn short_garbage_is_not_stl() {
    assert_eq!(read(b"", "p"), Err(StlError::NotStl));
    assert_eq!(read(b"hello", "p"), Err(StlError::NotStl));
    assert_eq!(read(&[0xff; 83], "p"), Err(StlError::NotStl));
}

const CUBE_CORNER: &str = "solid corner piece
  facet normal 0 0 -1
    outer loop
      vertex 0 0 0
      vertex 0 1 0
      vertex 1 0 0
    endloop
  endfacet
  facet normal 0 0 1
    outer loop
      vertex 1.5 -2.25 3
      vertex 1e-3 4.0E+02 -0
      vertex 7 8 9
    endloop
  endfacet
endsolid corner piece
";

#[test]
fn ascii_solid_with_name() {
    let objs = read(CUBE_CORNER.as_bytes(), "p").unwrap();
    assert_eq!(objs.len(), 1);
    assert_eq!(objs[0].name, "corner piece");
    assert_eq!(
        objs[0].triangles,
        vec![[[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]], T2]
    );
}

#[test]
fn ascii_keywords_any_case_and_layout() {
    let text = "\n\n   SOLID\nFacet Normal 0 0 1 Outer Loop\nVERTEX 0 0 0\n\
                vertex 1 0 0 vertex\n0 1 0\n  EndLoop EndFacet\nENDSOLID whatever\n";
    let objs = read(text.as_bytes(), "unnamed").unwrap();
    assert_eq!(
        objs,
        vec![MeshObject {
            name: "unnamed".into(),
            triangles: vec![T1],
        }]
    );
}

#[test]
fn ascii_several_solids() {
    let text = "solid a\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\n\
                vertex 0 1 0\nendloop\nendfacet\nendsolid a\n\nsolid b\nendsolid b\n\
                solid\nfacet normal 0 0 0\nouter loop\nvertex 1.5 -2.25 3\nvertex 0.001 400 0\n\
                vertex 7 8 9\nendloop\nendfacet\nendsolid\n";
    let objs = read(text.as_bytes(), "d").unwrap();
    let names: Vec<&str> = objs.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "d"]);
    assert_eq!(objs[0].triangles, vec![T1]);
    assert!(objs[1].triangles.is_empty());
    assert_eq!(objs[2].triangles.len(), 1);
}

#[test]
fn ascii_without_endsolid_is_accepted() {
    let text = "solid open\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\n\
                vertex 0 1 0\nendloop\nendfacet\n";
    let objs = read(text.as_bytes(), "d").unwrap();
    assert_eq!(objs[0].name, "open");
    assert_eq!(objs[0].triangles, vec![T1]);
}

fn syntax_line(text: &str) -> usize {
    match read(text.as_bytes(), "d") {
        Err(StlError::Syntax { line, .. }) => line,
        other => panic!("expected a syntax error, got {other:?}"),
    }
}

#[test]
fn ascii_bad_number_reports_its_line() {
    let text = "solid s\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 zero 0\n";
    assert_eq!(syntax_line(text), 5);
}

#[test]
fn ascii_non_finite_numbers_are_refused() {
    for bad in ["nan", "inf", "-inf", "NaN"] {
        let text = format!(
            "solid s\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\n\
             vertex 0 {bad} 0\nendloop\nendfacet\nendsolid\n"
        );
        assert_eq!(syntax_line(&text), 6, "{bad}");
    }
}

#[test]
fn ascii_fourth_vertex_is_an_error_on_its_line() {
    let text = "solid s\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\n\
                vertex 1 1 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid\n";
    assert_eq!(syntax_line(text), 7);
}

#[test]
fn ascii_short_loop_is_an_error_at_endloop() {
    let text = "solid s\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\n\
                endloop\nendfacet\nendsolid\n";
    assert_eq!(syntax_line(text), 6);
}

#[test]
fn ascii_missing_keyword() {
    // `outer loop` missing.
    let text = "solid s\nfacet normal 0 0 1\nvertex 0 0 0\n";
    assert_eq!(syntax_line(text), 3);
    // `normal` missing.
    let text = "solid s\nfacet 0 0 1\n";
    assert_eq!(syntax_line(text), 2);
    // Text after endsolid that isn't another solid.
    let text = "solid s\nendsolid s\nfacet normal 0 0 1\n";
    assert_eq!(syntax_line(text), 3);
}

#[test]
fn ascii_end_of_file_inside_a_facet() {
    let text = "solid s\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\n";
    assert_eq!(syntax_line(text), 4);
    let text = "solid s\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\n\
                vertex 0 1 0\nendloop";
    assert_eq!(syntax_line(text), 7);
}

#[test]
fn ascii_that_is_not_utf8_is_not_stl() {
    let mut bytes = b"solid s\nfacet normal 0 0 1\n".to_vec();
    bytes.push(0xff);
    bytes.push(0xfe);
    assert_eq!(read(&bytes, "d"), Err(StlError::NotStl));
}

#[test]
fn error_messages() {
    assert_eq!(StlError::NotStl.to_string(), "not an STL file");
    assert_eq!(
        StlError::Truncated {
            expected: 134,
            actual: 100,
        }
        .to_string(),
        "binary STL is 100 bytes, expected 134"
    );
    assert_eq!(
        StlError::Syntax {
            line: 12,
            message: "expected `endloop`".into(),
        }
        .to_string(),
        "line 12: expected `endloop`"
    );
}
