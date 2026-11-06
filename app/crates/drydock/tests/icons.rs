// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for the icon button (`docs/specs/m6-icons.md`, kept by
//! `docs/specs/m10-brand-icons.md`). The drawing itself is tested in `icons_svg.rs`.

use drydock::icons::{self, BUTTON_SIZE, Icon};
use drydock::input::Tool;
use eframe::egui::{self, Rect, Vec2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

#[derive(Default)]
struct ButtonState {
    clicks: usize,
    rect: Option<Rect>,
    selected: bool,
}

fn button_harness(selected: bool) -> Harness<'static, ButtonState> {
    let state = ButtonState {
        selected,
        ..ButtonState::default()
    };
    Harness::builder()
        .with_size(vec2(200.0, 120.0))
        .build_ui_state(
            |ui, state: &mut ButtonState| {
                let response = icons::icon_button(
                    ui,
                    Icon::Tool(Tool::Line),
                    "Line",
                    "Line (L)",
                    state.selected,
                );
                state.rect = Some(response.rect);
                if response.clicked() {
                    state.clicks += 1;
                }
            },
            state,
        )
}

#[test]
fn icon_button_is_found_by_label_and_clicks() {
    let mut h = button_harness(false);
    h.run();
    h.get_by_label("Line").click();
    h.run();
    assert_eq!(h.state().clicks, 1);
}

#[test]
fn icon_button_is_button_size() {
    let mut h = button_harness(false);
    h.run();
    let rect = h.state().rect.expect("drawn");
    assert_eq!(rect.size(), Vec2::splat(BUTTON_SIZE));
}

#[test]
fn icon_button_reports_whether_it_is_selected() {
    use egui::accesskit::Toggled;
    for selected in [false, true] {
        let mut h = button_harness(selected);
        h.run();
        let toggled = h.get_by_label("Line").accesskit_node().toggled();
        let want = if selected {
            Toggled::True
        } else {
            Toggled::False
        };
        assert_eq!(toggled, Some(want), "selected = {selected}");
    }
}

#[test]
fn icon_button_shows_its_tooltip_on_hover() {
    let mut h = button_harness(false);
    h.run();
    h.get_by_label("Line").hover();
    for _ in 0..90 {
        h.step();
    }
    h.run();
    assert!(
        h.query_by_label("Line (L)").is_some(),
        "hovering shows the tooltip"
    );
}
