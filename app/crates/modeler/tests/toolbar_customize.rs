// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-toolbar-customize.md`: the View > Toolbars menu
//! contents and the Customize window contents, driven with egui_kittest.

use eframe::egui::accesskit::Toggled;
use eframe::egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use modeler::toolbars::{
    CustomizeState, Placement, ToolbarLayout, ToolbarsMenuEvent, customize_panel, toolbars_menu,
};

struct MenuState {
    layout: ToolbarLayout,
    events: Vec<ToolbarsMenuEvent>,
}

fn menu(layout: ToolbarLayout) -> Harness<'static, MenuState> {
    let mut h = Harness::builder()
        .with_size(vec2(400.0, 400.0))
        .build_ui_state(
            |ui, state: &mut MenuState| {
                if let Some(event) = toolbars_menu(ui, &mut state.layout) {
                    state.events.push(event);
                }
            },
            MenuState {
                layout,
                events: Vec::new(),
            },
        );
    h.run();
    h
}

struct PanelState {
    layout: ToolbarLayout,
    custom: CustomizeState,
    changed: Vec<bool>,
}

fn panel(layout: ToolbarLayout, selected: usize) -> Harness<'static, PanelState> {
    let mut h = Harness::builder()
        .with_size(vec2(500.0, 700.0))
        .build_ui_state(
            |ui, state: &mut PanelState| {
                let changed = customize_panel(ui, &mut state.layout, &mut state.custom);
                state.changed.push(changed);
            },
            PanelState {
                layout,
                custom: CustomizeState {
                    selected,
                    new_name: String::new(),
                    ..CustomizeState::default()
                },
                changed: Vec::new(),
            },
        );
    h.run();
    h
}

fn changed_since(
    h: &mut Harness<'_, PanelState>,
    action: impl FnOnce(&mut Harness<'_, PanelState>),
) -> bool {
    h.state_mut().changed.clear();
    action(h);
    h.run();
    h.state().changed.iter().any(|&c| c)
}

#[test]
fn menu_lists_every_toolbar_with_its_visibility() {
    let mut layout = ToolbarLayout::default();
    layout.set_visible(2, false);
    let h = menu(layout);
    for (i, t) in h.state().layout.toolbars().iter().enumerate() {
        let node = h.get_by_label(&t.name);
        let on = node.accesskit_node().toggled() == Some(Toggled::True);
        assert_eq!(on, i != 2, "{}", t.name);
    }
    h.get_by_label("Customize...");
    h.get_by_label("Reset Toolbars");
}

#[test]
fn menu_checkbox_toggles_visibility() {
    let mut h = menu(ToolbarLayout::default());
    h.get_by_label("Drawing").click();
    h.run();
    assert!(!h.state().layout.toolbars()[1].visible);
    assert_eq!(h.state().events, vec![ToolbarsMenuEvent::Changed]);
    h.get_by_label("Drawing").click();
    h.run();
    assert!(h.state().layout.toolbars()[1].visible);
    assert_eq!(h.state().events.len(), 2);
}

#[test]
fn menu_customize_only_reports() {
    let mut h = menu(ToolbarLayout::default());
    h.get_by_label("Customize...").click();
    h.run();
    assert_eq!(h.state().events, vec![ToolbarsMenuEvent::Customize]);
    assert_eq!(h.state().layout, ToolbarLayout::default());
}

#[test]
fn menu_reset_restores_the_default() {
    let mut layout = ToolbarLayout::default();
    layout.set_visible(0, false);
    layout.add_toolbar("Mine");
    let mut h = menu(layout);
    h.get_by_label("Reset Toolbars").click();
    h.run();
    assert_eq!(h.state().layout, ToolbarLayout::default());
    assert_eq!(h.state().events, vec![ToolbarsMenuEvent::Changed]);
}

#[test]
fn panel_shows_the_selected_toolbars_tools() {
    let h = panel(ToolbarLayout::default(), 1);
    for label in [
        "Drag Line",
        "Drag Rectangle",
        "Drag Circle",
        "Drag Arc",
        "Drag 3-Point Arc",
        "Drag Move",
        "Drag Select",
    ] {
        h.get_by_label(label);
    }
    // The per-tool buttons are gone: dragging replaces them (round 3).
    for label in [
        "Remove Line",
        "Add Move",
        "Move Line earlier",
        "Move Line later",
    ] {
        assert!(h.query_by_label(label).is_none(), "{label} should be gone");
    }
}

#[test]
fn panel_picker_switches_toolbar_without_changing_layout() {
    let mut h = panel(ToolbarLayout::default(), 0);
    let changed = changed_since(&mut h, |h| h.get_by_label("Modification").click());
    assert!(!changed);
    assert_eq!(h.state().custom.selected, 2);
    assert_eq!(h.state().layout, ToolbarLayout::default());
}

#[test]
fn panel_creates_and_selects_a_toolbar() {
    let mut h = panel(ToolbarLayout::default(), 0);
    h.get_by_label("New toolbar name:").click();
    h.get_by_label("New toolbar name:").type_text("Mine");
    h.run();
    assert_eq!(h.state().custom.new_name, "Mine");
    let changed = changed_since(&mut h, |h| h.get_by_label("Create toolbar").click());
    assert!(changed);
    let layout = &h.state().layout;
    assert_eq!(layout.toolbars().len(), 6);
    assert_eq!(layout.toolbars()[5].name, "Mine");
    assert!(matches!(
        layout.toolbars()[5].placement,
        Placement::Floating { .. }
    ));
    assert_eq!(h.state().custom.selected, 5);
    assert_eq!(h.state().custom.new_name, "");
    assert!(layout.toolbars()[5].tools.is_empty());
}

#[test]
fn panel_creates_with_an_empty_name() {
    let mut h = panel(ToolbarLayout::default(), 0);
    h.get_by_label("Create toolbar").click();
    h.run();
    assert_eq!(h.state().layout.toolbars().len(), 6);
    assert_eq!(h.state().layout.toolbars()[5].name, "Toolbar");
}

#[test]
fn panel_deletes_and_clamps_selection() {
    let mut h = panel(ToolbarLayout::default(), 4);
    let changed = changed_since(&mut h, |h| h.get_by_label("Delete toolbar").click());
    assert!(changed);
    assert_eq!(h.state().layout.toolbars().len(), 4);
    assert_eq!(h.state().custom.selected, 3);
    assert!(
        h.state()
            .layout
            .toolbars()
            .iter()
            .all(|t| t.name != "Camera")
    );
}

#[test]
fn panel_out_of_range_selection_is_clamped() {
    let h = panel(ToolbarLayout::default(), 99);
    assert_eq!(h.state().custom.selected, 4);
    h.get_by_label("Drag Orbit");
}

#[test]
fn panel_reset_restores_the_default() {
    let mut layout = ToolbarLayout::default();
    layout.add_toolbar("Mine");
    layout.remove_tool(1, 0);
    let mut h = panel(layout, 1);
    let changed = changed_since(&mut h, |h| h.get_by_label("Reset Toolbars").click());
    assert!(changed);
    assert_eq!(h.state().layout, ToolbarLayout::default());
}
