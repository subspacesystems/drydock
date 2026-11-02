// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m1-dae-to-kernel on real exported models. Runs only when DRYDOCK_CORPUS points at the
//! corpus (the test oracle sets it); workers can't run it locally. Bounds were measured
//! independently by the orchestrator.

mod common;

use std::path::PathBuf;

use common::*;
use drydock_dae::{flatten, import_file, to_kernel};
use drydock_kernel::model::BodyId;
use drydock_kernel::topology::Violation;

/// Bodies with more edges than this aren't validated (validation is quadratic).
const VALIDATE_MAX_EDGES: usize = 300;

/// Kernel faces per file triangle, at most (summed over bodies, not instances).
const MAX_FACE_RATIO: f64 = 0.4;

fn corpus() -> Option<PathBuf> {
    match std::env::var_os("DRYDOCK_CORPUS") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => {
            eprintln!("DRYDOCK_CORPUS not set; skipping corpus checks (the test oracle runs them)");
            None
        }
    }
}

#[test]
fn corpus_models_convert() {
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
        let started = std::time::Instant::now();
        let m = to_kernel(&d).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        eprintln!(
            "{name}: converted in {:.1} s",
            started.elapsed().as_secs_f64()
        );
        // `Model::validate` would validate every body's geometry (quadratic); check the
        // model-level rules here and small bodies below.
        for (id, info) in m.definitions() {
            if info.group {
                assert_eq!(m.instances_of(id).len(), 1, "{name}: group {}", info.name);
            }
        }
        assert_eq!(m.materials().count(), d.materials.len(), "{name}");

        // The corpus is exported two-sided: `flatten` draws both twins, the kernel folds
        // them into one face. So a material's front plus back area is what `flatten` draws
        // with it. Faces without a twin have a default back too, so the default material
        // isn't compared.
        let (front, back) = kernel_areas(&m);
        let mut both = front.clone();
        for (k, a) in back {
            *both.entry(k).or_insert(0.0) += a;
        }
        both.remove(&None);
        let mut flat = flat_areas(&flatten(&d));
        flat.remove(&None);
        // Per material within 2.5 %: 01 has a few faces repeated in the file (dropped as
        // duplicates, 1.5 % of one material). The total is much tighter.
        assert_areas_match(&name, &both, &flat, 0.025, 1e-6);
        let (total, want): (f64, f64) = (both.values().sum(), flat.values().sum());
        assert!(
            (total - want).abs() <= 1e-4 * want,
            "{name}: total area {total}, flatten {want}"
        );

        let file_triangles: usize = d
            .meshes
            .iter()
            .flat_map(|mesh| mesh.primitives.iter())
            .filter(|p| p.kind == drydock_dae::PrimitiveKind::Triangles)
            .map(|p| p.indices.len() / 3)
            .sum();
        let mut faces = 0;
        let bodies = std::iter::once(BodyId::Root)
            .chain(m.definitions().map(|(id, _)| BodyId::Definition(id)));
        for body in bodies {
            let ctx = m.body(body).unwrap().geometry();
            faces += ctx.face_count();
            if ctx.edge_count() <= VALIDATE_MAX_EDGES {
                let bad: Vec<Violation> = ctx
                    .validate()
                    .into_iter()
                    .filter(|v| !matches!(v, Violation::EdgesIntersect(..)))
                    .collect();
                assert!(bad.is_empty(), "{name} {body:?}: {bad:?}");
            }
        }
        eprintln!("{name}: {file_triangles} triangles -> {faces} faces");
        assert!(
            (faces as f64) <= MAX_FACE_RATIO * file_triangles as f64,
            "{name}: {faces} faces from {file_triangles} triangles"
        );
    }
}
