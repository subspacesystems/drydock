// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Guide wording (`docs/specs/m8-guide-wording.md`): Drydock is described as a general
//! 3D modeler, printing stays on its own pages, and the tool is called Extrude.

use std::path::PathBuf;

fn guide_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../docs/guide")
}

fn pages() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(guide_src()).expect("guide folder") {
        let path = e.unwrap().path();
        if path.extension().is_some_and(|x| x == "md") {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            out.push((name, std::fs::read_to_string(&path).unwrap()));
        }
    }
    out
}

#[test]
fn no_page_says_push_pull() {
    for (name, text) in pages() {
        let low = text.to_lowercase();
        assert!(!low.contains("push/pull"), "{name}: says Push/Pull");
        assert!(!low.contains("push pull"), "{name}: says push pull");
        assert!(!low.contains("push-pull"), "{name}: links push-pull");
    }
}

#[test]
fn extrude_page_replaces_push_pull_page() {
    let dir = guide_src();
    assert!(dir.join("extrude.md").exists());
    assert!(!dir.join("push-pull.md").exists());
    let text = std::fs::read_to_string(dir.join("extrude.md")).unwrap();
    assert_eq!(text.lines().next(), Some("# Extrude"));
    let welcome = std::fs::read_to_string(dir.join("welcome.md")).unwrap();
    assert!(welcome.contains("(extrude.md)"), "welcome links extrude.md");
}

#[test]
fn welcome_intro_is_general_purpose() {
    let text = std::fs::read_to_string(guide_src().join("welcome.md")).unwrap();
    let paras: Vec<&str> = text
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty() && !p.starts_with('#'))
        .take(2)
        .collect();
    assert_eq!(paras.len(), 2);
    let first_two = paras.join("\n").to_lowercase();
    assert!(
        !first_two.contains("print"),
        "first two paragraphs: {first_two}"
    );
    assert!(
        first_two.contains("building"),
        "architecture is named first"
    );
}

#[test]
fn printing_framing_stays_on_its_own_pages() {
    for (name, text) in pages() {
        if ["printing.md", "files.md", "welcome.md"].contains(&name.as_str()) {
            continue;
        }
        let low = text.to_lowercase();
        assert!(!low.contains("print"), "{name}: mentions printing");
        assert!(!low.contains("solid part"), "{name}: says solid parts");
    }
}
