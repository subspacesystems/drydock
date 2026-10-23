// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-texture-scene on real exported models: after DAE import, the kernel scene puts the
//! file's texture coordinates back on textured faces. Runs only when MODELER_CORPUS points
//! at the corpus checkout.

use std::collections::HashMap;
use std::path::PathBuf;

use modeler_dae::{FlatScene, flatten, import_file, kernel_scene, to_kernel};

fn corpus() -> Option<PathBuf> {
    match std::env::var_os("MODELER_CORPUS") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => {
            eprintln!("MODELER_CORPUS not set; skipping corpus checks (the test oracle runs them)");
            None
        }
    }
}

type Cell = (i64, i64, i64);
type Vertex = (usize, [f32; 3], [f32; 2]);

/// Textured vertices of `s` by grid cell: (material index, position, uv). Both scenes list
/// the materials in the document's order.
fn textured_vertices(s: &FlatScene, cell: f32) -> HashMap<Cell, Vec<Vertex>> {
    let mut out: HashMap<Cell, Vec<_>> = HashMap::new();
    for b in &s.batches {
        let Some(index) = b.material else { continue };
        if s.materials[index].texture.is_none() {
            continue;
        }
        for (pos, uv) in b.positions.iter().zip(&b.texcoords) {
            let key = (
                (pos[0] / cell).floor() as i64,
                (pos[1] / cell).floor() as i64,
                (pos[2] / cell).floor() as i64,
            );
            out.entry(key).or_default().push((index, *pos, *uv));
        }
    }
    out
}

#[test]
fn corpus_textures_keep_their_coordinates() {
    let Some(root) = corpus() else { return };
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("corpus directory is readable")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("model.dae").is_file())
        .collect();
    dirs.sort();
    assert!(!dirs.is_empty(), "no models in {}", root.display());

    let mut textured_models = 0;
    for dir in dirs {
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let d = import_file(&dir.join("model.dae")).expect("corpus model imports");
        let m = to_kernel(&d).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let s = kernel_scene(&m);
        let f = flatten(&d);

        for (i, (fm, sm)) in f.materials.iter().zip(&s.materials).enumerate() {
            assert_eq!(fm.texture, sm.texture, "{name}: material {i} texture");
        }
        for b in &s.batches {
            let textured = b.material.is_some_and(|i| s.materials[i].texture.is_some());
            let want = if textured { b.positions.len() } else { 0 };
            assert_eq!(b.texcoords.len(), want, "{name}: batch {:?}", b.material);
            assert!(
                b.texcoords.iter().flatten().all(|x| x.is_finite()),
                "{name}: texture coordinates are finite"
            );
        }

        let Some(bounds) = f.bounds else { continue };
        let size = (0..3)
            .map(|k| bounds.max[k] - bounds.min[k])
            .fold(0.0f32, f32::max)
            .max(1e-3);
        let tol = 1e-5 * size;
        let cell = tol * 4.0;
        let got = textured_vertices(&s, cell);
        let mut total = 0usize;
        let mut matched = 0usize;
        for b in &f.batches {
            let Some(index) = b.material else { continue };
            if f.materials[index].texture.is_none() || b.texcoords.is_empty() {
                continue;
            }
            for (pos, uv) in b.positions.iter().zip(&b.texcoords) {
                total += 1;
                let key = [0, 1, 2].map(|k| (pos[k] / cell).floor() as i64);
                let mut found = false;
                'search: for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            let Some(list) = got.get(&(key[0] + dx, key[1] + dy, key[2] + dz))
                            else {
                                continue;
                            };
                            for (gid, gpos, guv) in list {
                                let near = (0..3).all(|k| (gpos[k] - pos[k]).abs() <= tol);
                                let uv_tol = 1e-3 * uv[0].abs().max(uv[1].abs()).max(1.0);
                                let same_uv = (0..2).all(|k| (guv[k] - uv[k]).abs() <= uv_tol);
                                if *gid == index && near && same_uv {
                                    found = true;
                                    break 'search;
                                }
                            }
                        }
                    }
                }
                if found {
                    matched += 1;
                }
            }
        }
        if total == 0 {
            continue;
        }
        textured_models += 1;
        let share = matched as f64 / total as f64;
        eprintln!("{name}: {matched} of {total} textured vertices keep their coordinates");
        assert!(
            share >= 0.97,
            "{name}: only {matched} of {total} textured vertices keep their coordinates"
        );
    }
    assert!(textured_models > 0, "the corpus has textured models");
}
