// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m2-pick-imported on real exported models: picking, snapping and highlighting reach the
//! geometry inside imported instances. Runs only when DRYDOCK_CORPUS points at the corpus.

use std::path::PathBuf;

use drydock_dae::{import_file, to_kernel};
use drydock_kernel::infer::{Probe, Snap, infer_model, pick_model};
use drydock_kernel::model::BodyId;
use drydock_kernel::select::{BoxMode, Entity, box_select, instance_segments};
use glam::{DVec2, DVec3};

fn top_project(q: DVec3) -> Option<DVec2> {
    Some(DVec2::new(q.x, q.y) * 10.0)
}

/// A probe looking straight down at world (x, y); radius 5 px is 0.5 m.
fn top(x: f64, y: f64) -> Probe<'static> {
    Probe {
        cursor: DVec2::new(x, y) * 10.0,
        origin: DVec3::new(x, y, 10_000.0),
        dir: DVec3::new(0.0, 0.0, -1.0),
        parallel: true,
        project: &top_project,
        radius: 5.0,
        anchor: None,
        axes: [DVec3::X, DVec3::Y, DVec3::Z],
        centers: &[],
        lock: None,
    }
}

#[test]
fn the_house_can_be_picked_snapped_to_and_highlighted() {
    let Some(dir) = std::env::var_os("DRYDOCK_CORPUS") else {
        eprintln!("DRYDOCK_CORPUS not set; skipping corpus checks (the test oracle runs them)");
        return;
    };
    let path = PathBuf::from(dir).join("05-house/model.dae");
    let doc = import_file(&path).expect("import");
    let model = to_kernel(&doc).expect("convert");
    let root = model.root();

    // A window box over everything finds the model's instances.
    let everything = box_select(
        &model,
        BodyId::Root,
        &top_project,
        DVec2::splat(-1.0e6),
        DVec2::splat(1.0e6),
        BoxMode::Window,
    );
    let instances: Vec<_> = everything
        .iter()
        .filter_map(|e| match e {
            Entity::Instance(i) => Some(*i),
            _ => None,
        })
        .collect();
    assert_eq!(instances.len(), root.instances().count());

    // Each has edges to highlight; collect them all.
    let mut segments = Vec::new();
    for &i in &instances {
        let s = instance_segments(&model, BodyId::Root, i);
        assert!(!s.is_empty(), "instance {i:?} has no edges");
        segments.extend(s);
    }

    // The highest point of the house, seen from above, can be picked and snapped to.
    let top_point = segments
        .iter()
        .flat_map(|&(a, b)| [a, b])
        .max_by(|a, b| a.z.total_cmp(&b.z))
        .expect("a point");
    let probe = top(top_point.x, top_point.y);
    let hit = pick_model(&model, &probe).expect("something under the cursor");
    if root.geometry().face_count() == 0 && root.geometry().edge_count() == 0 {
        assert!(matches!(hit, Entity::Instance(_)), "{hit:?}");
    }
    let inf = infer_model(&model, &probe).expect("an inference");
    assert!(
        matches!(
            inf.snap,
            Snap::Endpoint(_) | Snap::Midpoint(_) | Snap::OnEdge(_)
        ),
        "{:?}",
        inf.snap
    );
    assert!(
        inf.point.distance(top_point) < 0.6,
        "{:?} is far from {top_point:?}",
        inf.point
    );
}
