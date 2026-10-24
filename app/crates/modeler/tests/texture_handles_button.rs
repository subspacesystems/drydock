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
