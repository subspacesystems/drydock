// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use std::path::{Path, PathBuf};

use modeler::document::{Document, format_size};

fn slab() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/slab.dae")
}

#[test]
fn open_reports_stats() {
    let doc = Document::open(&slab()).expect("slab.dae opens");
    assert_eq!(doc.path, slab());
    assert_eq!(doc.stats.triangles, 2);
    // The quad's four edges and the post (m4-fix-dae-edges).
    assert_eq!(doc.stats.lines, 5);
    assert_eq!(doc.stats.materials, 1);
    let want = [2.0, 3.0, 4.0];
    for (got, want) in doc.stats.size_m.iter().zip(want) {
        assert!((got - want).abs() < 1e-5, "{:?}", doc.stats.size_m);
    }
}

#[test]
fn stats_match_the_scene() {
    let doc = Document::open(&slab()).unwrap();
    assert_eq!(doc.stats.triangles, doc.scene.triangle_count());
    assert_eq!(doc.stats.lines, doc.scene.line_count());
    assert_eq!(doc.stats.materials, doc.scene.materials.len());
}

#[test]
fn empty_model_has_zero_size() {
    let dir = std::env::temp_dir().join(format!("modeler-doc-empty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("empty.dae");
    std::fs::write(
        &path,
        r##"<?xml version="1.0"?><COLLADA xmlns="http://www.collada.org/2005/11/COLLADASchema" version="1.4.1"><asset><unit meter="1" name="meter"/><up_axis>Z_UP</up_axis></asset><library_visual_scenes><visual_scene id="s"/></library_visual_scenes><scene><instance_visual_scene url="#s"/></scene></COLLADA>"##,
    )
    .unwrap();
    let doc = Document::open(&path).expect("empty model opens");
    assert_eq!(doc.stats.triangles, 0);
    assert_eq!(doc.stats.lines, 0);
    assert_eq!(doc.stats.size_m, [0.0, 0.0, 0.0]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_file_error_names_the_file() {
    let err = match Document::open(Path::new("/nonexistent/dir/lost.dae")) {
        Ok(_) => panic!("should fail"),
        Err(e) => e,
    };
    assert!(err.contains("lost.dae"), "{err}");
    assert!(!err.contains('\n'), "{err}");
}

#[test]
fn malformed_file_error_names_the_file() {
    let dir = std::env::temp_dir().join(format!("modeler-doc-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("broken.dae");
    std::fs::write(&path, "<COLLADA><oops>").unwrap();
    let err = match Document::open(&path) {
        Ok(_) => panic!("should fail"),
        Err(e) => e,
    };
    assert!(err.contains("broken.dae"), "{err}");
    assert!(!err.contains('\n'), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn title_is_file_name_then_product() {
    let doc = Document::open(&slab()).unwrap();
    assert_eq!(doc.title(), "slab.dae - modeler");
}

#[test]
fn size_text() {
    assert_eq!(
        format_size([4.115, 6.706, 2.438]),
        "4.115 x 6.706 x 2.438 m"
    );
    assert_eq!(format_size([0.0, 0.0, 0.0]), "0.000 x 0.000 x 0.000 m");
    assert_eq!(format_size([2.0, 3.0, 4.0]), "2.000 x 3.000 x 4.000 m");
    assert_eq!(
        format_size([12.3456, 0.0004, 100.0]),
        "12.346 x 0.000 x 100.000 m"
    );
}
