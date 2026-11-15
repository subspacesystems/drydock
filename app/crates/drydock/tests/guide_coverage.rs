// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Every tool and every menu item has an entry in the user guide
//! (`docs/specs/m8-guide-text-more.md`). Tools come from the toolbar's own table, so a new
//! tool fails this test until the guide names it; menu items are listed below and must be
//! kept in step with `chrome::menu_bar_with_toolbars` (a new menu item goes here too).

use std::path::PathBuf;

use drydock::chrome::{TOOL_BUTTONS, tool_shortcut};

/// (menu, item label as the menu shows it, ignoring the dots after it).
const MENU_ITEMS: &[(&str, &str)] = &[
    ("File", "New"),
    ("File", "Open"),
    ("File", "Import"),
    ("File", "Export 3MF"),
    ("File", "Export STL"),
    ("File", "Save"),
    ("File", "Save As"),
    ("File", "Exit"),
    ("Edit", "Undo"),
    ("Edit", "Redo"),
    ("Edit", "Cut"),
    ("Edit", "Copy"),
    ("Edit", "Paste"),
    ("Edit", "Erase"),
    ("Edit", "Make Component"),
    ("Edit", "Make Group"),
    ("Edit", "Explode"),
    ("Edit", "Flip Along Red"),
    ("Edit", "Flip Along Green"),
    ("Edit", "Flip Along Blue"),
    ("View", "Switch to Parallel Projection"),
    ("View", "Switch to Perspective"),
    ("View", "Zoom Extents"),
    ("View", "Iso"),
    ("View", "Top"),
    ("View", "Front"),
    ("View", "Right"),
    ("View", "Back"),
    ("View", "Left"),
    ("View", "Bottom"),
    ("View", "Hide Rest of Model"),
    ("View", "Axes"),
    ("View", "Overhangs"),
    ("View", "Entity Info"),
    ("View", "Materials"),
    ("View", "Components"),
    ("View", "Shadows"),
    ("View", "Texture Position"),
    ("View", "Toolbars"),
    ("View", "Reset Panels"),
    ("View", "Model Info"),
    ("Tools", "Fit Check"),
    ("Tools", "Grow by Clearance"),
    ("Tools", "Shrink by Clearance"),
    ("Tools", "3D Text"),
    ("Tools", "Print Setup"),
    ("Help", "User Guide"),
    ("Help", "Check for Updates"),
    ("Help", "About Drydock"),
];

fn guide_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../docs/guide")
}

fn all_pages() -> Vec<(String, String)> {
    let mut pages = Vec::new();
    for entry in std::fs::read_dir(guide_src()).expect("docs/guide exists") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|e| e == "md") {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).expect("page reads");
            pages.push((name, text));
        }
    }
    pages
}

#[test]
fn every_menu_item_is_written_as_menu_arrow_item() {
    let pages = all_pages();
    for (menu, item) in MENU_ITEMS {
        let needle = format!("{menu} > {item}").to_lowercase();
        assert!(
            pages
                .iter()
                .any(|(_, text)| text.to_lowercase().contains(&needle)),
            "no guide page mentions \"{menu} > {item}\""
        );
    }
}

#[test]
fn every_tool_is_named_in_the_guide() {
    let pages = all_pages();
    for (_, label) in TOOL_BUTTONS {
        let needle = label.to_lowercase();
        assert!(
            pages
                .iter()
                .any(|(_, text)| text.to_lowercase().contains(&needle)),
            "no guide page names the tool {label:?}"
        );
    }
}

#[test]
fn the_shortcut_page_has_one_row_per_tool() {
    let text = std::fs::read_to_string(guide_src().join("shortcuts.md")).expect("shortcuts.md");
    for (tool, label) in TOOL_BUTTONS {
        let key = format!("`{}`", tool_shortcut(tool));
        let row = text
            .lines()
            .any(|l| l.to_lowercase().contains(&label.to_lowercase()) && l.contains(&key));
        assert!(row, "shortcuts.md has no line with {label:?} and {key}");
    }
}

#[test]
fn the_shortcut_page_lists_the_menu_shortcuts() {
    let text = std::fs::read_to_string(guide_src().join("shortcuts.md")).expect("shortcuts.md");
    for key in [
        "Ctrl+N",
        "Ctrl+O",
        "Ctrl+S",
        "Ctrl+Shift+S",
        "Ctrl+Z",
        "Ctrl+Y",
        "Ctrl+X",
        "Ctrl+C",
        "Ctrl+V",
        "Delete",
        "Shift+P",
        "Shift+Z",
        "F1",
        "F2",
        "F3",
        "F4",
        "F5",
        "F6",
        "F7",
        "F8",
    ] {
        assert!(
            text.contains(&format!("`{key}`")),
            "shortcuts.md should list `{key}` in backticks"
        );
    }
}
