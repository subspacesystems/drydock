// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! STL reader on random triangles (`docs/specs/m7-import-stl.md`).

use modeler_mesh::stl::read;
use proptest::prelude::*;

type Tri = [[f32; 3]; 3];

fn binary(header: &[u8; 80], tris: &[Tri]) -> Vec<u8> {
    let mut out = header.to_vec();
    out.extend_from_slice(&(tris.len() as u32).to_le_bytes());
    for t in tris {
        out.extend_from_slice(&[0u8; 12]);
        for c in t {
            for x in c {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        out.extend_from_slice(&[0u8; 2]);
    }
    out
}

fn ascii(tris: &[Tri]) -> String {
    let mut s = String::from("solid r\n");
    for t in tris {
        s.push_str("  facet normal 0 0 1\n    outer loop\n");
        for c in t {
            s.push_str(&format!("      vertex {} {} {}\n", c[0], c[1], c[2]));
        }
        s.push_str("    endloop\n  endfacet\n");
    }
    s.push_str("endsolid r\n");
    s
}

fn coord() -> impl Strategy<Value = f32> {
    prop_oneof![-1.0e4f32..1.0e4, -1.0f32..1.0, Just(0.0f32), Just(-0.0f32)]
}

fn tri() -> impl Strategy<Value = Tri> {
    [
        [coord(), coord(), coord()],
        [coord(), coord(), coord()],
        [coord(), coord(), coord()],
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn binary_round_trip(
        tris in proptest::collection::vec(tri(), 0..40),
        header in proptest::array::uniform32(any::<u8>()),
        solid in any::<bool>(),
    ) {
        // Any header, including one that starts with `solid`.
        let mut h = [0u8; 80];
        h[..32].copy_from_slice(&header);
        if solid {
            h[..6].copy_from_slice(b"solid ");
        }
        let objs = read(&binary(&h, &tris), "p").unwrap();
        prop_assert_eq!(objs.len(), 1);
        prop_assert_eq!(objs[0].triangles.len(), tris.len());
        for (a, b) in objs[0].triangles.iter().zip(&tris) {
            for i in 0..3 {
                for j in 0..3 {
                    prop_assert_eq!(a[i][j].to_bits(), b[i][j].to_bits());
                }
            }
        }
    }

    #[test]
    fn ascii_round_trip(tris in proptest::collection::vec(tri(), 0..40)) {
        let objs = read(ascii(&tris).as_bytes(), "p").unwrap();
        prop_assert_eq!(objs.len(), 1);
        prop_assert_eq!(&objs[0].name, "r");
        // Display writes the shortest text that parses back to the same f32 (-0 as "-0").
        prop_assert_eq!(&objs[0].triangles, &tris);
    }

    #[test]
    fn cutting_a_binary_file_never_panics(
        tris in proptest::collection::vec(tri(), 1..10),
        cut in 1usize..400,
    ) {
        let mut bytes = binary(&[0u8; 80], &tris);
        let keep = bytes.len().saturating_sub(cut);
        bytes.truncate(keep);
        prop_assert!(read(&bytes, "p").is_err());
    }
}
