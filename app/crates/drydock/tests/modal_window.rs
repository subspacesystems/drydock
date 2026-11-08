// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-modal-window: `modal_dialog_window` through its embedded fallback (egui_kittest), and
//! the shared prompts built on it. See docs/specs/m11-modal-window.md.

use drydock::dialog::{DialogWindow, confirm_dialog, modal_dialog_window, name_dialog};
use eframe::egui::{self, Key, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

struct State {
    open: bool,
    main_clicks: u32,
    dialog_clicks: u32,
}

fn harness(open: bool) -> Harness<'static, State> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(600.0, 400.0))
        .build_ui_state(
            |ui, state: &mut State| {
                // The main window's own content, top left, outside the dialog.
                if ui.button("Main button").clicked() {
                    state.main_clicks += 1;
                }
                let win = DialogWindow {
                    title: "Modal Test",
                    size: Vec2::new(300.0, 120.0),
                };
                let mut open = state.open;
                modal_dialog_window(
                    ui.ctx(),
                    egui::Id::new("modal-test"),
                    &win,
                    &mut open,
                    |ui| {
                        ui.label("hello from the prompt");
                        if ui.button("Inside").clicked() {
                            state.dialog_clicks += 1;
                        }
                    },
                );
                state.open = open;
            },
            State {
                open,
                main_clicks: 0,
                dialog_clicks: 0,
            },
        );
    h.run();
    h
}

#[test]
fn embedded_modal_shows_the_title_and_the_content() {
    let h = harness(true);
    assert!(h.ctx.embed_viewports(), "kittest embeds viewports");
    h.get_by_label("Modal Test");
    h.get_by_label("hello from the prompt");
}

#[test]
fn nothing_is_drawn_while_closed_and_the_main_window_works() {
    let mut h = harness(false);
    assert!(h.query_by_label("Modal Test").is_none());
    assert!(h.query_by_label("hello from the prompt").is_none());
    h.get_by_label("Main button").click();
    h.run();
    assert_eq!(h.state().main_clicks, 1);
}

#[test]
fn while_open_a_click_in_the_main_window_does_nothing() {
    let mut h = harness(true);
    h.get_by_label("Main button").click();
    h.run();
    assert_eq!(h.state().main_clicks, 0, "the main window ignores clicks");
    assert!(h.state().open, "and the prompt stays");
}

#[test]
fn clicks_inside_the_prompt_work() {
    let mut h = harness(true);
    h.get_by_label("Inside").click();
    h.run();
    assert_eq!(h.state().dialog_clicks, 1);
}

#[test]
fn the_main_window_works_again_once_the_prompt_closes() {
    let mut h = harness(true);
    h.state_mut().open = false;
    h.run();
    h.get_by_label("Main button").click();
    h.run();
    assert_eq!(h.state().main_clicks, 1);
}

#[test]
fn escape_clears_open() {
    let mut h = harness(true);
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().open);
    assert!(h.query_by_label("hello from the prompt").is_none());
}

// ---- the shared prompts keep their behaviour on top of it ----

struct Ask {
    answers: Vec<Option<bool>>,
    name: String,
}

fn confirm_harness() -> Harness<'static, Ask> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(600.0, 400.0))
        .build_ui_state(
            |ui, state: &mut Ask| {
                let answer = confirm_dialog(
                    ui.ctx(),
                    egui::Id::new("modal-confirm"),
                    "Delete Toolbar",
                    "Delete the toolbar \"Camera\"?",
                    "Delete",
                );
                state.answers.push(answer);
            },
            Ask {
                answers: Vec::new(),
                name: String::new(),
            },
        );
    h.run();
    h
}

fn answered(state: &Ask) -> Vec<bool> {
    state.answers.iter().flatten().copied().collect()
}

#[test]
fn confirm_shows_the_title_and_message_and_answers_once() {
    let mut h = confirm_harness();
    h.get_by_label("Delete Toolbar");
    h.get_by_label_contains("Camera");
    assert!(answered(h.state()).is_empty());
    h.get_by_label("Delete").click();
    h.run();
    assert_eq!(answered(h.state()), vec![true]);
}

#[test]
fn confirm_escape_and_enter_cancel() {
    let mut h = confirm_harness();
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(answered(h.state()), vec![false]);
    let mut h = confirm_harness();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(answered(h.state()), vec![false], "Cancel is the default");
}

fn name_harness(name: &str) -> Harness<'static, Ask> {
    let mut h = Harness::builder()
        .with_size(Vec2::new(600.0, 400.0))
        .build_ui_state(
            |ui, state: &mut Ask| {
                let mut name = std::mem::take(&mut state.name);
                let answer = name_dialog(
                    ui.ctx(),
                    egui::Id::new("modal-name"),
                    "New Toolbar",
                    &mut name,
                );
                state.name = name;
                state.answers.push(answer);
            },
            Ask {
                answers: Vec::new(),
                name: name.to_owned(),
            },
        );
    h.run();
    h
}

#[test]
fn name_prompt_enter_answers_ok_with_a_name_and_escape_cancels() {
    let mut h = name_harness("Mine");
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(answered(h.state()), vec![true]);
    assert_eq!(h.state().name, "Mine");
    let mut h = name_harness("Mine");
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(answered(h.state()), vec![false]);
}

#[test]
fn name_prompt_enter_does_nothing_while_the_name_is_blank() {
    let mut h = name_harness("  ");
    h.key_press(Key::Enter);
    h.run();
    assert!(answered(h.state()).is_empty());
}
