// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-toolbar-customize-3.md`: renaming a toolbar, in the
//! layout. The Customize window's rename dialog is tested in `tests/confirm.rs` (m6-confirm).

use drydock::toolbars::{ToolbarLayout, ToolbarsMenuEvent, toolbars_menu};
use eframe::egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

const DRAWING: usize = 1;

#[test]
fn rename_sets_the_name_and_keeps_everything_else() {
    let mut layout = ToolbarLayout::default();
    assert!(layout.rename_toolbar(DRAWING, "Sketch"));
    assert_eq!(layout.toolbars()[DRAWING].name, "Sketch");
    let before = ToolbarLayout::default();
    let (a, b) = (&layout.toolbars()[DRAWING], &before.toolbars()[DRAWING]);
    assert_eq!(a.tools, b.tools);
    assert_eq!(a.visible, b.visible);
    assert_eq!(a.placement, b.placement);
    for index in [0, 2, 3, 4] {
        assert_eq!(layout.toolbars()[index], before.toolbars()[index]);
    }
}

#[test]
fn rename_cleans_the_name() {
    let mut layout = ToolbarLayout::default();
    assert!(layout.rename_toolbar(DRAWING, "  My\ntools\r\n "));
    assert_eq!(layout.toolbars()[DRAWING].name, "My tools");
}

#[test]
fn rename_to_nothing_or_the_same_name_changes_nothing() {
    let mut layout = ToolbarLayout::default();
    assert!(!layout.rename_toolbar(DRAWING, ""));
    assert!(!layout.rename_toolbar(DRAWING, "   \n"));
    assert!(!layout.rename_toolbar(DRAWING, "Drawing"));
    assert!(!layout.rename_toolbar(DRAWING, " Drawing "));
    assert!(!layout.rename_toolbar(99, "Sketch"));
    assert_eq!(layout, ToolbarLayout::default());
}

#[test]
fn rename_to_another_toolbars_name_gets_a_number() {
    let mut layout = ToolbarLayout::default();
    assert!(layout.rename_toolbar(DRAWING, "Camera"));
    assert_eq!(layout.toolbars()[DRAWING].name, "Camera 2");
    layout.add_toolbar("Mine");
    assert!(layout.rename_toolbar(5, "Camera"));
    assert_eq!(layout.toolbars()[5].name, "Camera 3");
    // Its own current name doesn't count as taken.
    assert!(!layout.rename_toolbar(5, "Camera 3"));
}

#[test]
fn renamed_layout_survives_the_file() {
    let mut layout = ToolbarLayout::default();
    layout.rename_toolbar(DRAWING, "Sketch tools");
    let back = ToolbarLayout::parse(&layout.to_text());
    assert_eq!(back, layout);
}

#[test]
fn view_menu_lists_the_new_name() {
    let mut layout = ToolbarLayout::default();
    layout.rename_toolbar(DRAWING, "Sketch");
    let mut h = Harness::builder()
        .with_size(vec2(400.0, 400.0))
        .build_ui_state(
            |ui, layout: &mut ToolbarLayout| {
                let _: Option<ToolbarsMenuEvent> = toolbars_menu(ui, layout);
            },
            layout,
        );
    h.run();
    h.get_by_label("Sketch");
    assert!(h.query_by_label("Drawing").is_none());
}
