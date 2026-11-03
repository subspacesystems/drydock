// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The rest of the guide pages (`docs/specs/m8-guide-text-more.md`): they exist, follow the
//! first six in the contents, have the required headings and key facts, are linked from the
//! welcome page, and the whole guide builds with no broken link.

use std::path::PathBuf;

use drydock_guide::{build, parse_summary, render_markdown};

/// Page path, level-1 title, required level-2 headings, words each page must mention.
const PAGES: &[(&str, &str, &[&str], &[&str])] = &[
    (
        "modify.md",
        "Moving, rotating and scaling",
        &["Move", "Rotate", "Scale", "Flip", "Erase"],
        &[
            "M", "Q", "S", "E", "Ctrl", "Shift", "Delete", "copy", "Enter", "Esc", "Edit",
        ],
    ),
    (
        "measuring.md",
        "Measuring and units",
        &["Tape Measure", "Guides", "Choosing units", "Typing lengths"],
        &[
            "T",
            "Ctrl",
            "Model Info",
            "Millimeters",
            "Architectural",
            "Precision",
            "undo",
        ],
    ),
    (
        "groups.md",
        "Groups and components",
        &[
            "Making a group or component",
            "Editing inside",
            "The Components panel",
            "Exploding",
            "Copies of a component",
        ],
        &[
            "double",
            "Esc",
            "Hide Rest of Model",
            "search",
            "list",
            "tiles",
            "name",
        ],
    ),
    (
        "materials.md",
        "Materials and painting",
        &[
            "Paint Bucket",
            "The Materials panel",
            "Textures",
            "Picking a material from the model",
        ],
        &[
            "B",
            "Alt",
            "swatch",
            "opacity",
            "Purge",
            "image",
            "Texture Position",
        ],
    ),
    (
        "files.md",
        "Files, import and export",
        &[
            "Saving and opening",
            "Importing models",
            "Exporting for printing",
        ],
        &[
            ".drydock",
            "STL",
            "3MF",
            "DAE",
            "millimeters",
            "recent",
            "tolerance",
            "skipped",
            "Selection",
        ],
    ),
    (
        "printing.md",
        "3D printing",
        &[
            "Is it a solid?",
            "Repairing",
            "Print Setup",
            "Overhangs",
            "Fit check and clearance",
            "3D text",
        ],
        &[
            "Entity Info",
            "Repair",
            "Drop to Bed",
            "Bed X",
            "45",
            "yellow",
            "Clearance",
            "Emboss",
            "Engrave",
            "Open edges",
        ],
    ),
    (
        "shortcuts.md",
        "Keyboard shortcuts",
        &[
            "Tools",
            "Views and navigation",
            "Files and editing",
            "Modifier keys",
        ],
        &[
            "Ctrl+Z",
            "Ctrl+Y",
            "Delete",
            "F1",
            "Shift+Z",
            "Shift+P",
            "Ctrl+Shift+S",
        ],
    ),
];

fn guide_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../docs/guide")
}

fn read(page: &str) -> String {
    std::fs::read_to_string(guide_src().join(page)).unwrap_or_else(|e| panic!("{page}: {e}"))
}

#[test]
fn contents_lists_the_new_pages_after_the_first_six() {
    let entries = parse_summary(&read("SUMMARY.md")).expect("SUMMARY.md parses");
    let listed: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
    let first_six = [
        "welcome.md",
        "window.md",
        "navigation.md",
        "selecting.md",
        "drawing.md",
        "extrude.md",
    ];
    assert_eq!(
        listed[..6],
        first_six,
        "the first six entries keep their order"
    );
    let rest: Vec<&str> = PAGES.iter().map(|p| p.0).collect();
    assert_eq!(listed[6..], rest[..], "the new pages follow, in this order");
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
        let min_page = if *page == "shortcuts.md" { 120 } else { 250 };
        for s in r
            .sections
            .iter()
            .filter(|s| headings.contains(&s.heading.as_str()))
        {
            let min = if *page == "shortcuts.md" { 8 } else { 15 };
            assert!(
                s.text.split_whitespace().count() >= min,
                "{page}: section {:?} is too short",
                s.heading
            );
        }
        let words: usize = r
            .sections
            .iter()
            .map(|s| s.text.split_whitespace().count())
            .sum();
        assert!(words >= min_page, "{page}: only {words} words");
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
fn welcome_links_every_new_page() {
    let r = render_markdown(&read("welcome.md"));
    for (page, ..) in PAGES {
        assert!(
            r.links.iter().any(|l| l == page),
            "welcome.md does not link {page}"
        );
    }
}

#[test]
fn guide_builds_with_no_broken_links() {
    let out = std::env::temp_dir().join(format!("drydock-guide-more-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    let report = build(&guide_src(), &out).unwrap_or_else(|e| panic!("{e}"));
    for (page, ..) in PAGES {
        assert!(report.pages.iter().any(|p| p == page), "{page} not built");
        let html = page.replace(".md", ".html");
        assert!(out.join(&html).is_file(), "{html} not written");
    }
    assert!(
        report.orphans.is_empty(),
        "pages not in the contents: {:?}",
        report.orphans
    );
    let _ = std::fs::remove_dir_all(&out);
}
