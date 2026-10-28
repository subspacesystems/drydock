// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-toolbar-customize-3.md`: renaming a toolbar, in the
//! layout and in the Customize window.

use eframe::egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use modeler::toolbars::{
    CustomizeState, ToolbarLayout, ToolbarsMenuEvent, customize_panel, toolbars_menu,
};

const DRAWING: usize = 1;

fn names(layout: &ToolbarLayout) -> Vec<String> {
    layout.toolbars().iter().map(|t| t.name.clone()).collect()
}

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

struct State {
    layout: ToolbarLayout,
    custom: CustomizeState,
    changed: Vec<bool>,
}

fn panel(selected: usize) -> Harness<'static, State> {
    let mut h = Harness::builder()
        .with_size(vec2(700.0, 800.0))
        .build_ui_state(
            |ui, state: &mut State| {
                let changed = customize_panel(ui, &mut state.layout, &mut state.custom);
                state.changed.push(changed);
            },
            State {
                layout: ToolbarLayout::default(),
                custom: CustomizeState {
                    selected,
                    ..CustomizeState::default()
                },
                changed: Vec::new(),
            },
        );
    h.run();
    h
}

fn rename_text(h: &Harness<'_, State>) -> Option<(usize, String)> {
    h.state().custom.rename.clone()
}

fn changed_since(h: &mut Harness<'_, State>, action: impl FnOnce(&mut Harness<'_, State>)) -> bool {
    h.state_mut().changed.clear();
    action(h);
    h.run();
    h.state().changed.iter().any(|&c| c)
}

#[test]
fn rename_field_holds_the_selected_toolbars_name() {
    let mut h = panel(DRAWING);
    h.get_by_label("Toolbar name:");
    h.get_by_label("Rename toolbar");
    assert_eq!(rename_text(&h), Some((DRAWING, "Drawing".to_string())));
    let changed = changed_since(&mut h, |h| h.get_by_label("Camera").click());
    assert!(!changed);
    assert_eq!(rename_text(&h), Some((4, "Camera".to_string())));
}

#[test]
fn rename_button_renames_the_selected_toolbar() {
    let mut h = panel(DRAWING);
    h.state_mut().custom.rename = Some((DRAWING, "Sketch".to_string()));
    h.run();
    let changed = changed_since(&mut h, |h| h.get_by_label("Rename toolbar").click());
    assert!(changed);
    assert_eq!(h.state().layout.toolbars()[DRAWING].name, "Sketch");
    assert_eq!(rename_text(&h), Some((DRAWING, "Sketch".to_string())));
    // The picker shows the new name.
    h.get_by_label("Sketch");
}

#[test]
fn typing_a_name_and_pressing_enter_renames() {
    let mut h = panel(DRAWING);
    h.state_mut().custom.rename = Some((DRAWING, String::new()));
    h.run();
    h.get_by_label("Toolbar name:").click();
    h.run();
    h.get_by_label("Toolbar name:").type_text("Shapes");
    h.run();
    assert_eq!(rename_text(&h), Some((DRAWING, "Shapes".to_string())));
    // Typing alone renames nothing.
    assert_eq!(h.state().layout.toolbars()[DRAWING].name, "Drawing");
    let changed = changed_since(&mut h, |h| h.key_press(Key::Enter));
    assert!(changed);
    assert_eq!(h.state().layout.toolbars()[DRAWING].name, "Shapes");
}

#[test]
fn a_taken_name_is_numbered_and_the_field_shows_it() {
    let mut h = panel(DRAWING);
    h.state_mut().custom.rename = Some((DRAWING, "Camera".to_string()));
    h.run();
    h.get_by_label("Rename toolbar").click();
    h.run();
    assert_eq!(h.state().layout.toolbars()[DRAWING].name, "Camera 2");
    assert_eq!(rename_text(&h), Some((DRAWING, "Camera 2".to_string())));
}

#[test]
fn an_empty_name_changes_nothing_and_restores_the_field() {
    let mut h = panel(DRAWING);
    h.state_mut().custom.rename = Some((DRAWING, "  ".to_string()));
    h.run();
    let changed = changed_since(&mut h, |h| h.get_by_label("Rename toolbar").click());
    assert!(!changed);
    assert_eq!(h.state().layout, ToolbarLayout::default());
    assert_eq!(rename_text(&h), Some((DRAWING, "Drawing".to_string())));
}

#[test]
fn field_follows_the_selection_after_delete_create_and_reset() {
    // Deleting Modification leaves the selection on index 2, now Construction.
    let mut h = panel(2);
    h.get_by_label("Delete toolbar").click();
    h.run();
    assert_eq!(h.state().custom.selected, 2);
    assert_eq!(rename_text(&h), Some((2, "Construction".to_string())));

    // A new toolbar is selected and the field shows its name.
    h.state_mut().custom.new_name = "Mine".to_string();
    h.run();
    h.get_by_label("Create toolbar").click();
    h.run();
    let selected = h.state().custom.selected;
    assert_eq!(h.state().layout.toolbars()[selected].name, "Mine");
    assert_eq!(rename_text(&h), Some((selected, "Mine".to_string())));

    // Reset brings back the default names; a renamed toolbar's field goes back too.
    let mut h = panel(DRAWING);
    h.state_mut().custom.rename = Some((DRAWING, "Sketch".to_string()));
    h.run();
    h.get_by_label("Rename toolbar").click();
    h.run();
    h.get_by_label("Reset Toolbars").click();
    h.run();
    assert_eq!(names(&h.state().layout)[DRAWING], "Drawing");
    assert_eq!(rename_text(&h), Some((DRAWING, "Drawing".to_string())));
}
