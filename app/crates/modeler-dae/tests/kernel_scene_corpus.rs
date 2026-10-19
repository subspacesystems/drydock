// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m1-kernel-scene on real exported models. Runs only when MODELER_CORPUS points at the
//! corpus (the test oracle sets it); workers can't run it locally.

mod common;

use std::path::PathBuf;

use common::*;
use modeler_dae::{flatten, import_file, kernel_scene, to_kernel};

fn corpus() -> Option<PathBuf> {
    match std::env::var_os("MODELER_CORPUS") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => {
            eprintln!("MODELER_CORPUS not set; skipping corpus checks (the test oracle runs them)");
            None
        }
    }
}

#[test]
fn corpus_models_draw_like_flatten() {
    let Some(root) = corpus() else { return };
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("corpus directory is readable")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("model.dae").is_file())
        .collect();
    dirs.sort();
    assert!(!dirs.is_empty(), "no models in {}", root.display());

    for dir in dirs {
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let d = import_file(&dir.join("model.dae")).expect("corpus model imports");
        let m = to_kernel(&d).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let started = std::time::Instant::now();
        let s = kernel_scene(&m);
        let took = started.elapsed().as_secs_f64();
        let f = flatten(&d);
        eprintln!(
            "{name}: drawn in {took:.2} s, {} triangles (flatten {})",
            s.triangle_count(),
            f.triangle_count()
        );
        assert!(took < 20.0, "{name}: kernel_scene took {took:.1} s");
        assert_eq!(s.materials.len(), f.materials.len(), "{name}");

        // Back sides with a material are drawn as copies, like the exporter's twins. Sides
        // without a material aren't drawn by either in a comparable way, so the default
        // material isn't compared (same tolerance as m1-dae-to-kernel's corpus test).
        let mut got = flat_areas(&s);
        got.remove(&None);
        let mut want = flat_areas(&f);
        want.remove(&None);
        assert_areas_match(&name, &got, &want, 0.025, 1e-6);
        let (total, want_total): (f64, f64) = (got.values().sum(), want.values().sum());
        assert!(
            (total - want_total).abs() <= 1e-4 * want_total,
            "{name}: total area {total}, flatten {want_total}"
        );

        let (sb, fb) = (s.bounds.expect("bounds"), f.bounds.expect("bounds"));
        let size: f32 = (0..3).map(|k| fb.max[k] - fb.min[k]).fold(0.0, f32::max);
        for k in 0..3 {
            assert!(
                (sb.min[k] - fb.min[k]).abs() <= 1e-4 * size
                    && (sb.max[k] - fb.max[k]).abs() <= 1e-4 * size,
                "{name}: bounds {sb:?}, flatten {fb:?}"
            );
        }
    }
}
