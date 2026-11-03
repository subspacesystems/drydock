// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The first real guide pages (`docs/specs/m8-guide-text-start.md`): they exist, are
//! listed in the contents, have the required headings and key facts, and the whole
//! guide builds with no broken link.

use std::path::PathBuf;

use drydock_guide::{build, parse_summary, render_markdown};

/// Page path, level-1 title, required level-2 headings, words each page must mention.
const PAGES: &[(&str, &str, &[&str], &[&str])] = &[
    (
        "welcome.md",
        "Welcome to Drydock",
        &["What Drydock is", "Where to start"],
        &[
            "navigation.md",
            "selecting.md",
            "drawing.md",
            "extrude.md",
            "window.md",
        ],
    ),
    (
        "window.md",
        "The Drydock window",
        &["Menus", "Toolbars and panels", "The measurements box"],
        &["File", "Edit", "View", "Tools", "Help", "F1"],
    ),
    (
        "navigation.md",
        "Moving around",
        &[
            "Orbit",
            "Pan",
            "Zoom",
            "Standard views",
            "Perspective and parallel",
        ],
        &[
            "middle", "scroll", "Shift", "F2", "F8", "Shift+Z", "Shift+P",
        ],
    ),
    (
        "selecting.md",
        "Selecting",
        &[
            "Clicking",
            "Adding and removing",
            "Selecting with a box",
            "Selecting connected geometry",
        ],
        &[
            "Space", "Shift", "Ctrl", "Alt", "double", "triple", "left", "right",
        ],
    ),
    (
        "drawing.md",
        "Drawing",
        &[
            "Lines",
            "Rectangles",
            "Circles",
            "Arcs",
            "Typing values",
            "Locking a direction",
        ],
        &[
            "L", "R", "C", "A", "Shift+A", "Enter", "Esc", "segments", "arrow",
        ],
    ),
    (
        "extrude.md",
        "Extrude",
        &["Pushing a face", "Typing a distance", "Repeating a push"],
        &["P", "click", "Enter", "double", "Esc", "undo"],
    ),
];

fn guide_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../docs/guide")
}

fn read(page: &str) -> String {
    std::fs::read_to_string(guide_src().join(page)).unwrap_or_else(|e| panic!("{page}: {e}"))
}

#[test]
fn contents_lists_the_pages_in_order() {
    let entries = parse_summary(&read("SUMMARY.md")).expect("SUMMARY.md parses");
    let listed: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    let mut at = 0;
    for (page, ..) in PAGES {
        let i = listed
            .iter()
            .position(|p| p == page)
            .unwrap_or_else(|| panic!("{page} is not in SUMMARY.md"));
        assert!(i >= at, "{page} is out of order in SUMMARY.md");
        at = i;
    }
    assert_eq!(listed[0], "welcome.md", "the welcome page comes first");
}

#[test]
fn pages_have_title_and_headings() {
    for (page, title, headings, _) in PAGES {
        let r = render_markdown(&read(page));
        assert_eq!(r.title, *title, "{page}: title");
        for h in *headings {
            assert!(
                r.headings.iter().any(|x| x.level == 2 && x.text == *h),
                "{page}: missing level-2 heading {h:?}"
            );
        }
    }
}

#[test]
fn pages_mention_the_key_facts() {
    for (page, _, _, words) in PAGES {
        let text = read(page);
        for w in *words {
            assert!(text.contains(w), "{page}: should mention {w:?}");
        }
    }
}

#[test]
fn pages_have_real_text() {
    for (page, _, headings, _) in PAGES {
        let r = render_markdown(&read(page));
        for s in r
            .sections
            .iter()
            .filter(|s| headings.contains(&s.heading.as_str()))
        {
            assert!(
                s.text.split_whitespace().count() >= 15,
                "{page}: section {:?} is too short",
                s.heading
            );
        }
        let words: usize = r
            .sections
            .iter()
            .map(|s| s.text.split_whitespace().count())
            .sum();
        assert!(words >= 250, "{page}: only {words} words");
    }
}

#[test]
fn pages_use_the_product_name_and_no_other_marks() {
    for (page, ..) in PAGES {
        let lower = read(page).to_lowercase();
        for banned in ["sketchup", "trimble", "last software"] {
            assert!(!lower.contains(banned), "{page}: contains {banned:?}");
        }
        assert!(
            !lower
                .split(|c: char| !c.is_alphanumeric())
                .any(|w| w == "modeler"),
            "{page}: says \"modeler\" (the product is Drydock)"
        );
    }
}

#[test]
fn guide_builds_with_no_broken_links() {
    let out = std::env::temp_dir().join(format!("drydock-guide-text-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let report = build(&guide_src(), &out).unwrap_or_else(|e| panic!("{e}"));
    for (page, ..) in PAGES {
        assert!(report.pages.iter().any(|p| p == page), "{page} not built");
        let html = page.replace(".md", ".html");
        assert!(out.join(&html).is_file(), "{html} not written");
    }
    assert!(out.join("index.html").is_file());
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn every_page_is_linked_from_the_welcome_page() {
    let r = render_markdown(&read("welcome.md"));
    for (page, ..) in PAGES.iter().skip(1) {
        assert!(
            r.links.iter().any(|l| l == page),
            "welcome.md does not link {page}"
        );
    }
}
