// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m0-dae-import: real exported models. Runs only when MODELER_CORPUS points at the
//! corpus (the test oracle sets it); workers can't run it locally. Expected numbers were
//! measured independently by the orchestrator. Models added to the corpus later get the
//! generic checks until their numbers are added here.

use std::path::{Path, PathBuf};

use modeler_dae::{FlatScene, flatten, import_file};

struct Expect {
    dir: &'static str,
    triangles: usize,
    lines: usize,
    materials: usize,
    definitions: usize,
    cameras: usize,
    /// World bounds in meters, rounded to 0.1 mm.
    min: [f32; 3],
    max: [f32; 3],
}

const EXPECTED: &[Expect] = &[
    Expect {
        dir: "01-honeydew-manor",
        triangles: 404398,
        lines: 2518,
        materials: 114,
        definitions: 319,
        cameras: 1,
        min: [-61.9880, -49.4514, -0.4587],
        max: [54.0814, 55.5366, 26.8106],
    },
    Expect {
        dir: "02-cedar canyon",
        triangles: 3046,
        lines: 4,
        materials: 22,
        definitions: 1,
        cameras: 3,
        min: [-7.0539, -4.0035, -0.0224],
        max: [24.0569, 24.1035, 1.6256],
    },
    Expect {
        dir: "03-shedhouse",
        triangles: 2172,
        lines: 1,
        materials: 8,
        definitions: 8,
        cameras: 1,
        min: [-4.2634, -1.5519, -0.0000],
        max: [6.0911, 10.3039, 6.5079],
    },
    Expect {
        dir: "04-canning-shed",
        triangles: 36302,
        lines: 103,
        materials: 31,
        definitions: 30,
        cameras: 1,
        min: [-5.2979, -3.3844, -0.0000],
        max: [1.3569, 0.9748, 3.3523],
    },
    Expect {
        dir: "05-house",
        triangles: 362340,
        lines: 708,
        materials: 128,
        definitions: 102,
        cameras: 1,
        min: [-0.8403, 0.1639, -0.1714],
        max: [14.5265, 21.1074, 3.2528],
    },
    Expect {
        dir: "06-shop-office",
        triangles: 175,
        lines: 0,
        materials: 3,
        definitions: 4,
        cameras: 1,
        min: [-0.0000, 0.0000, 0.0000],
        max: [4.1148, 6.7056, 2.4384],
    },
    Expect {
        dir: "07-key-holder",
        triangles: 1089,
        lines: 0,
        materials: 1,
        definitions: 5,
        cameras: 1,
        min: [-0.0066, -0.0066, -0.2414],
        max: [0.1752, 0.0564, 0.1000],
    },
    Expect {
        dir: "08-desk-and-chair",
        triangles: 29142,
        lines: 108,
        materials: 36,
        definitions: 68,
        cameras: 1,
        min: [-0.0000, 0.0000, -0.0000],
        max: [1.0905, 1.2700, 1.4311],
    },
];

fn corpus() -> Option<PathBuf> {
    match std::env::var_os("MODELER_CORPUS") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => {
            eprintln!("MODELER_CORPUS not set; skipping corpus checks (the test oracle runs them)");
            None
        }
    }
}

fn models(root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .expect("corpus directory is readable")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("model.dae").is_file())
        .collect();
    dirs.sort();
    dirs
}

fn check_generic(name: &str, scene: &FlatScene) {
    for (bi, b) in scene.batches.iter().enumerate() {
        assert_eq!(
            b.normals.len(),
            b.positions.len(),
            "{name}: batch {bi} normals"
        );
        assert!(
            b.texcoords.is_empty() || b.texcoords.len() == b.positions.len(),
            "{name}: batch {bi} texcoords"
        );
        assert_eq!(b.indices.len() % 3, 0, "{name}: batch {bi} index count");
        assert!(
            b.indices.iter().all(|&i| (i as usize) < b.positions.len()),
            "{name}: batch {bi} index out of range"
        );
        assert!(
            b.positions.iter().flatten().all(|v| v.is_finite()),
            "{name}: batch {bi} non-finite position"
        );
        for n in &b.normals {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!(
                (len - 1.0).abs() < 1e-3,
                "{name}: batch {bi} normal {n:?} not unit length"
            );
        }
    }
    for m in &scene.materials {
        if let Some(tex) = &m.texture {
            assert!(
                tex.is_file(),
                "{name}: material {} texture {} missing",
                m.name,
                tex.display()
            );
        }
    }
}

#[test]
fn corpus_models_import() {
    let Some(root) = corpus() else { return };
    let dirs = models(&root);
    assert!(!dirs.is_empty(), "no models in {}", root.display());
    for e in EXPECTED {
        assert!(
            root.join(e.dir).join("model.dae").is_file(),
            "expected corpus model {} is missing",
            e.dir
        );
    }
    for dir in dirs {
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let model =
            import_file(&dir.join("model.dae")).unwrap_or_else(|err| panic!("{name}: {err}"));
        let scene = flatten(&model);
        check_generic(&name, &scene);
        let Some(e) = EXPECTED.iter().find(|e| e.dir == name) else {
            eprintln!("{name}: no expected numbers yet; generic checks only");
            continue;
        };
        assert_eq!(scene.triangle_count(), e.triangles, "{name}: triangles");
        assert_eq!(scene.line_count(), e.lines, "{name}: line segments");
        assert_eq!(model.materials.len(), e.materials, "{name}: materials");
        assert_eq!(
            model.definitions.len(),
            e.definitions,
            "{name}: component definitions"
        );
        assert_eq!(model.cameras.len(), e.cameras, "{name}: cameras");
        let b = scene.bounds.unwrap_or_else(|| panic!("{name}: empty"));
        for i in 0..3 {
            assert!(
                (b.min[i] - e.min[i]).abs() < 2e-3,
                "{name}: min {:?} vs {:?}",
                b.min,
                e.min
            );
            assert!(
                (b.max[i] - e.max[i]).abs() < 2e-3,
                "{name}: max {:?} vs {:?}",
                b.max,
                e.max
            );
        }
    }
}
