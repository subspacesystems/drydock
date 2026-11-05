// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m9-guide-dev: a plain `cargo build` makes the guide Help > User Guide opens.
//! See docs/specs/m9-guide-dev.md.

use drydock::guide_help::{GUIDE_INDEX, find_guide, guide_candidates};
use std::path::{Path, PathBuf};

fn dev_guide() -> PathBuf {
    PathBuf::from(env!("DRYDOCK_DEV_GUIDE"))
}

#[test]
fn the_build_made_the_start_page() {
    let index = dev_guide().join(GUIDE_INDEX);
    assert!(index.is_file(), "{} is missing", index.display());
    let html = std::fs::read_to_string(&index).unwrap();
    assert!(html.contains("<html"), "index.html is not a page");
}

#[test]
fn every_summary_page_was_built() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/guide");
    let summary = std::fs::read_to_string(src.join("SUMMARY.md")).unwrap();
    // Every `(page.md)` link in SUMMARY.md has its .html next to index.html.
    let mut seen = 0;
    for part in summary.split("](").skip(1) {
        let target = part.split(')').next().unwrap();
        if let Some(stem) = target.strip_suffix(".md") {
            let page = dev_guide().join(format!("{stem}.html"));
            assert!(page.is_file(), "{} is missing", page.display());
            seen += 1;
        }
    }
    assert!(seen >= 5, "SUMMARY.md lists {seen} pages");
    assert!(dev_guide().join("search-index.json").is_file());
    assert!(dev_guide().join("style.css").is_file());
}

#[test]
fn the_dev_guide_is_outside_the_source_tree() {
    // Not written into docs/ or app/target/guide: the folder is under cargo's own build output.
    let dev = dev_guide();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs");
    let dev = dev.canonicalize().unwrap();
    assert!(!dev.starts_with(src.canonicalize().unwrap()));
}

#[test]
fn installed_layout_wins_over_the_dev_guide() {
    let c = guide_candidates(Some(Path::new("/opt/drydock")));
    let found = find_guide(&c, |p| p.is_file() && !p.starts_with("/opt"));
    assert_eq!(found, Some(dev_guide().join(GUIDE_INDEX)));
    let found = find_guide(&c, |_| true);
    assert_eq!(
        found,
        Some(Path::new("/opt/drydock/guide").join(GUIDE_INDEX))
    );
}
