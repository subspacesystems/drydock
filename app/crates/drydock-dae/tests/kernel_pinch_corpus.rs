// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-dae-pinch-faces on real exported models: merged faces whose outline touches itself
//! are split into a few faces instead of raw triangles, so far fewer edges are left between
//! two flat faces of the same material. Runs only when DRYDOCK_CORPUS points at the corpus
//! (the test oracle sets it). Bounds were measured independently by the orchestrator.

use std::path::PathBuf;

use drydock_dae::{import_file, to_kernel};
use drydock_kernel::model::{BodyId, Model};

fn corpus() -> Option<PathBuf> {
    match std::env::var_os("DRYDOCK_CORPUS") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => {
            eprintln!("DRYDOCK_CORPUS not set; skipping corpus checks (the test oracle runs them)");
            None
        }
    }
}

/// Edges with exactly two faces that lie in one plane, face the same way and carry the same
/// materials on both sides (texture placement ignored), summed over every body.
fn flat_seams(m: &Model) -> usize {
    let mut bodies = vec![BodyId::Root];
    bodies.extend(m.definitions().map(|(id, _)| BodyId::Definition(id)));
    let mut seams = 0;
    for body in bodies {
        let b = m.body(body).expect("body exists");
        let ctx = b.geometry();
        for (e, _) in ctx.edges() {
            let fs = ctx.faces_of_edge(e);
            let [f, g] = fs[..] else { continue };
            let (Some(pf), Some(pg)) = (ctx.face_plane(f), ctx.face_plane(g)) else {
                continue;
            };
            let (mf, mg) = (b.face_materials(f), b.face_materials(g));
            if pf.normal.dot(pg.normal) > 1.0 - 1e-9 && mf.front == mg.front && mf.back == mg.back {
                seams += 1;
            }
        }
    }
    seams
}

/// Flat seams left after import, at most, per corpus model. Today's triangle fallback leaves
/// thousands (desk-and-chair 2127, house 1599, honeydew-manor 1694).
const MAX_SEAMS: &[(&str, usize)] = &[
    ("01-honeydew-manor", 500),
    ("02-cedar canyon", 40),
    ("03-shedhouse", 15),
    ("04-canning-shed", 80),
    ("05-house", 350),
    ("06-shop-office", 5),
    ("07-key-holder", 15),
    ("08-desk-and-chair", 260),
];

#[test]
fn corpus_pinched_faces_leave_few_flat_seams() {
    let Some(root) = corpus() else { return };
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .expect("corpus directory is readable")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("model.dae").is_file())
        .collect();
    dirs.sort();
    assert!(!dirs.is_empty(), "no models in {}", root.display());

    let mut failures = Vec::new();
    for dir in dirs {
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let d = import_file(&dir.join("model.dae")).expect("corpus model imports");
        let m = to_kernel(&d).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let seams = flat_seams(&m);
        let faces: usize = std::iter::once(m.root().geometry().face_count())
            .chain(m.definitions().map(|(id, _)| {
                m.body(BodyId::Definition(id))
                    .unwrap()
                    .geometry()
                    .face_count()
            }))
            .sum();
        eprintln!("{name}: {seams} flat seams, {faces} faces");
        if let Some(&(_, max)) = MAX_SEAMS.iter().find(|(n, _)| *n == name)
            && seams > max
        {
            failures.push(format!("{name}: {seams} flat seams, at most {max}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
