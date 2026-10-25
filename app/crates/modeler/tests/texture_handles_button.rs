// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Texture Position panel's handles button, driven with egui_kittest.
//! See docs/specs/m4-app-texture-handles.md.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use modeler::texture_panel::handles_button;

struct State {
    available: bool,
    on: bool,
    clicks: usize,
}

fn harness(available: bool, on: bool) -> Harness<'static, State> {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut State| {
            if handles_button(ui, state.available, state.on) {
                state.clicks += 1;
            }
        },
        State {
            available,
            on,
            clicks: 0,
        },
    );
    h.run();
    h
}

#[test]
fn off_and_available_offers_to_position_in_the_viewport() {
    let mut h = harness(true, false);
    assert!(h.query_by_label("Done").is_none());
    assert!(
        !h.get_by_label("Position in Viewport")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_label("Position in Viewport").click();
    h.run();
    assert_eq!(h.state().clicks, 1);
}

#[test]
fn off_and_unavailable_is_disabled() {
    let mut h = harness(false, false);
    assert!(
        h.get_by_label("Position in Viewport")
            .accesskit_node()
            .is_disabled()
    );
    h.get_by_label("Position in Viewport").click();
    h.run();
    assert_eq!(h.state().clicks, 0);
}

#[test]
fn on_shows_done_even_when_unavailable() {
    for available in [true, false] {
        let mut h = harness(available, true);
        assert!(h.query_by_label("Position in Viewport").is_none());
        assert!(!h.get_by_label("Done").accesskit_node().is_disabled());
        h.get_by_label("Done").click();
        h.run();
        assert_eq!(h.state().clicks, 1, "available = {available}");
    }
}

/// The section the app shows: heading, then the handles button, then the controls.
fn section(available: bool, on: bool) -> Harness<'static, State> {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut State| {
            let mut form = modeler::texture_panel::TextureForm::default();
            let (_, toggled) = modeler::texture_panel::texture_position_section(
                ui,
                &mut form,
                state.available,
                state.on,
            );
            if toggled {
                state.clicks += 1;
            }
        },
        State {
            available,
            on,
            clicks: 0,
        },
    );
    h.run();
    h
}

#[test]
fn the_section_puts_the_button_under_the_heading_and_above_the_controls() {
    let h = section(true, false);
    let heading = h.get_by_label("Texture Position").rect();
    let button = h.get_by_label("Position in Viewport").rect();
    let reset = h.get_by_label("Reset Texture").rect();
    assert!(button.top() >= heading.bottom(), "{heading:?} {button:?}");
    assert!(reset.top() >= button.bottom(), "{button:?} {reset:?}");
}

#[test]
fn the_section_reports_the_button_click() {
    let mut h = section(true, true);
    h.get_by_label("Done").click();
    h.run();
    assert_eq!(h.state().clicks, 1);
}
