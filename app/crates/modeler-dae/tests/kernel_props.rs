// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m1-dae-to-kernel: random height-field grids. Cells may be missing, triangles carry
//! random materials, segments lie on grid lines, and a back twin may cover every triangle.

mod common;

use std::collections::BTreeMap;

use common::*;
use glam::DVec3;
use modeler_dae::{UpAxis, to_kernel};
use proptest::prelude::*;

#[derive(Clone, Debug)]
struct Grid {
    nx: usize,
    ny: usize,
    /// Height (0 or 1 m) of each grid point, row by row.
    heights: Vec<u8>,
    /// Per cell.
    present: Vec<bool>,
    /// Per cell: split along the other diagonal.
    flip: Vec<bool>,
    /// Per triangle (two per cell): 0 = no material, 1 or 2 = material 0 or 1.
    mats: Vec<u8>,
    /// Back twin material per triangle when `two_sided`.
    backs: Vec<u8>,
    two_sided: bool,
    /// Horizontal grid lines first (`(nx) * (ny + 1)`), then vertical (`(nx + 1) * ny`).
    segments: Vec<bool>,
    /// Order in which the triangles are written.
    order: Vec<usize>,
    unit: f64,
    up: UpAxis,
}

fn grid() -> impl Strategy<Value = Grid> {
    (1usize..=5, 1usize..=5, any::<bool>())
        .prop_flat_map(|(nx, ny, flat)| {
            let points = (nx + 1) * (ny + 1);
            let cells = nx * ny;
            let lines = nx * (ny + 1) + (nx + 1) * ny;
            let heights = if flat {
                Just(vec![0u8; points]).boxed()
            } else {
                proptest::collection::vec(0u8..=1, points).boxed()
            };
            (
                Just((nx, ny)),
                heights,
                proptest::collection::vec(prop::bool::weighted(0.85), cells),
                proptest::collection::vec(any::<bool>(), cells),
                prop_oneof![
                    Just(vec![1u8; 2 * cells]),
                    proptest::collection::vec(0u8..=2, 2 * cells)
                ],
                proptest::collection::vec(0u8..=2, 2 * cells),
                any::<bool>(),
                proptest::collection::vec(prop::bool::weighted(0.1), lines),
                Just((0..2 * cells).collect::<Vec<usize>>()).prop_shuffle(),
                prop_oneof![Just(1.0), Just(0.0254)],
                prop_oneof![Just(UpAxis::Z), Just(UpAxis::Y), Just(UpAxis::X)],
            )
        })
        .prop_map(
            |(
                (nx, ny),
                heights,
                present,
                flip,
                mats,
                backs,
                two_sided,
                segments,
                order,
                unit,
                up,
            )| {
                Grid {
                    nx,
                    ny,
                    heights,
                    present,
                    flip,
                    mats,
                    backs,
                    two_sided,
                    segments,
                    order,
                    unit,
                    up,
                }
            },
        )
}

impl Grid {
    fn point(&self, x: usize, y: usize) -> [f32; 3] {
        [
            x as f32,
            y as f32,
            self.heights[y * (self.nx + 1) + x] as f32,
        ]
    }

    /// The two triangles of every cell (counter-clockwise from above), present or not.
    fn triangles(&self) -> Vec<[[f32; 3]; 3]> {
        let mut out = Vec::new();
        for y in 0..self.ny {
            for x in 0..self.nx {
                let c = y * self.nx + x;
                let (p00, p10, p11, p01) = (
                    self.point(x, y),
                    self.point(x + 1, y),
                    self.point(x + 1, y + 1),
                    self.point(x, y + 1),
                );
                if self.flip[c] {
                    out.push([p00, p10, p01]);
                    out.push([p10, p11, p01]);
                } else {
                    out.push([p00, p10, p11]);
                    out.push([p00, p11, p01]);
                }
            }
        }
        out
    }

    fn segment_list(&self) -> Vec<[[f32; 3]; 2]> {
        let mut out = Vec::new();
        let mut k = 0;
        for y in 0..=self.ny {
            for x in 0..self.nx {
                if self.segments[k] {
                    out.push([self.point(x, y), self.point(x + 1, y)]);
                }
                k += 1;
            }
        }
        for y in 0..self.ny {
            for x in 0..=self.nx {
                if self.segments[k] {
                    out.push([self.point(x, y), self.point(x, y + 1)]);
                }
                k += 1;
            }
        }
        out
    }

    fn document(&self) -> modeler_dae::Model {
        let mut d = doc();
        d.unit_meters = self.unit;
        d.up_axis = self.up;
        d.materials.push(material("m0", "M0", [1.0, 0.0, 0.0, 1.0]));
        d.materials.push(material("m1", "M1", [0.0, 1.0, 0.0, 1.0]));
        let all = self.triangles();
        let mut prims = Vec::new();
        for &t in &self.order {
            if !self.present[t / 2] {
                continue;
            }
            let symbol = ["none", "a", "b"][self.mats[t] as usize];
            prims.push(tris(Some(symbol), &[all[t]]));
        }
        if self.two_sided {
            for &t in &self.order {
                if !self.present[t / 2] {
                    continue;
                }
                let tri = all[t];
                let symbol = ["none", "a", "b"][self.backs[t] as usize];
                prims.push(tris(Some(symbol), &[[tri[0], tri[2], tri[1]]]));
            }
        }
        prims.push(segs(&self.segment_list()));
        let g = add_mesh(&mut d, prims, &[("a", 0), ("b", 1)]);
        d.scene.geometry.push(g);
        d
    }

    /// Expected world area per front and back material.
    fn expected(&self) -> (BTreeMap<Key, f64>, BTreeMap<Key, f64>) {
        let all = self.triangles();
        let mut front = BTreeMap::new();
        let mut back = BTreeMap::new();
        let key = |m: u8| if m == 0 { None } else { Some(m as usize - 1) };
        for (t, tri) in all.iter().enumerate() {
            if !self.present[t / 2] {
                continue;
            }
            let p = |i: usize| DVec3::new(tri[i][0] as f64, tri[i][1] as f64, tri[i][2] as f64);
            let area = (p(1) - p(0)).cross(p(2) - p(0)).length() * 0.5 * self.unit * self.unit;
            *front.entry(key(self.mats[t])).or_insert(0.0) += area;
            let b = if self.two_sided {
                key(self.backs[t])
            } else {
                None
            };
            *back.entry(b).or_insert(0.0) += area;
        }
        (front, back)
    }

    fn single_flat_face_expected(&self) -> bool {
        self.heights.iter().all(|&h| h == 0)
            && self.present.iter().all(|&p| p)
            && self.mats.iter().all(|&m| m == self.mats[0])
            && (!self.two_sided || self.backs.iter().all(|&b| b == self.backs[0]))
            && self.segments.iter().all(|&s| !s)
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn grids_convert_to_valid_faces_with_same_area(g in grid()) {
        let d = g.document();
        let m = to_kernel(&d).expect("grid converts");
        prop_assert!(m.validate().is_empty());
        let ctx = m.root().geometry();
        let violations = ctx.validate();
        prop_assert!(violations.is_empty(), "violations: {:?}", violations);

        let (front, back) = kernel_areas(&m);
        let (want_front, want_back) = g.expected();
        assert_areas_match("front", &front, &want_front, 1e-9, 1e-12);
        assert_areas_match("back", &back, &want_back, 1e-9, 1e-12);

        let present = g.present.iter().filter(|&&p| p).count();
        prop_assert!(ctx.face_count() <= 2 * present);
        if present == 0 {
            prop_assert_eq!(ctx.face_count(), 0);
        }
        if g.single_flat_face_expected() {
            prop_assert_eq!(ctx.face_count(), 1);
            let face = ctx.faces().next().unwrap().1;
            prop_assert!(face.inner.is_empty());
            prop_assert_eq!(face.outer.uses.len(), 2 * (g.nx + g.ny));
            prop_assert_eq!(ctx.vertex_count(), 2 * (g.nx + g.ny));
        }
    }
}
