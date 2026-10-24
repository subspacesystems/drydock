// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! A DAE file shows its edges as soon as it opens (`docs/specs/m4-fix-dae-edges.md`).

use std::path::{Path, PathBuf};

use modeler::document::Document;
use modeler_dae::{FlatScene, kernel_scene};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn tables() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../modeler-dae/tests/fixtures/tables.dae")
}

/// Everything the viewport draws, in a comparable form.
fn drawn(scene: &FlatScene) -> String {
    format!(
        "{:?}",
        (
            &scene.materials,
            &scene.batches,
            &scene.lines,
            &scene.bounds
        )
    )
}

#[test]
fn an_opened_dae_draws_its_face_edges() {
    let doc = Document::open(&fixture("slab.dae")).unwrap();
    // The quad's four edges and the post; the file itself only lists the post.
    assert_eq!(doc.scene.line_count(), 5);
    assert_eq!(doc.stats.lines, 5);
    assert_eq!(doc.stats.triangles, 2);
}

#[test]
fn an_opened_dae_draws_what_an_edit_would_draw() {
    for path in [fixture("slab.dae"), tables()] {
        let mut doc = Document::open(&path).unwrap();
        let opened = drawn(&doc.scene);
        let stats = doc.stats.clone();
        assert_eq!(opened, drawn(&kernel_scene(doc.kernel.as_ref().unwrap())));
        doc.refresh();
        assert_eq!(opened, drawn(&doc.scene), "{path:?}");
        assert_eq!(stats, doc.stats, "{path:?}");
    }
}

#[test]
fn opening_a_dae_leaves_it_unmodified() {
    let doc = Document::open(&tables()).unwrap();
    assert!(!doc.modified);
    assert!(doc.scene.line_count() > 0);
}
