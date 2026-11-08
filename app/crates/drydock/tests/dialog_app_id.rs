// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-dialog-app-id: dialog windows, modal prompts and floating panels carry Drydock's
//! application id and icon, like the main window.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use drydock::dialog::{DialogWindow, dialog_window, modal_dialog_window};
use drydock::dock::PanelId;
use drydock::floating::floating_panel_window;
use drydock::window_icon::{APP_ID, window_icon, with_app_identity};
use eframe::egui::{self, Rect, ViewportBuilder, ViewportId, pos2, vec2};

#[test]
fn helper_sets_app_id_and_icon() {
    let builder = with_app_identity(ViewportBuilder::default());
    assert_eq!(builder.app_id.as_deref(), Some(APP_ID));
    let icon = builder.icon.expect("icon set");
    assert_eq!(icon.width, window_icon().width);
}

/// Runs `show` for a few frames with real-window viewports and returns the builders egui was
/// asked to create, by viewport id. Immediate viewports reach the renderer hook (set per
/// thread); deferred ones appear in the frame's viewport output.
fn builders(show: impl Fn(&egui::Context)) -> HashMap<ViewportId, ViewportBuilder> {
    let found = Rc::new(RefCell::new(HashMap::new()));
    let sink = Rc::clone(&found);
    egui::Context::set_immediate_viewport_renderer(move |_ctx, mut viewport| {
        sink.borrow_mut()
            .insert(viewport.ids.this, viewport.builder);
        // The content callback must run once; give it a Ui of a scratch context.
        let mut output = egui::Context::default().run_ui(egui::RawInput::default(), |ui| {
            (viewport.viewport_ui_cb)(ui)
        });
        output.textures_delta.clear();
    });
    let ctx = egui::Context::default();
    ctx.set_embed_viewports(false);
    for _ in 0..3 {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| show(ui.ctx()));
        output.textures_delta.clear();
        for (id, viewport) in output.viewport_output {
            if id != ViewportId::ROOT {
                found.borrow_mut().insert(id, viewport.builder);
            }
        }
    }
    found.take()
}

fn assert_identity(found: &HashMap<ViewportId, ViewportBuilder>) {
    assert_eq!(
        found.len(),
        1,
        "one child viewport expected: {}",
        found.len()
    );
    for builder in found.values() {
        assert_eq!(builder.app_id.as_deref(), Some(APP_ID));
        assert!(builder.icon.is_some(), "icon missing");
    }
}

#[test]
fn dialog_window_carries_app_id_and_icon() {
    let window = DialogWindow {
        title: "App id probe",
        size: vec2(200.0, 100.0),
    };
    assert_identity(&builders(|ctx| {
        let mut open = true;
        dialog_window(ctx, &window, &mut open, |ui| {
            ui.label("x");
        });
    }));
}

#[test]
fn modal_prompt_carries_app_id_and_icon() {
    let window = DialogWindow {
        title: "Modal app id probe",
        size: vec2(200.0, 100.0),
    };
    assert_identity(&builders(|ctx| {
        let mut open = true;
        modal_dialog_window(ctx, egui::Id::new("m"), &window, &mut open, |ui| {
            ui.label("x");
        });
    }));
}

#[test]
fn floating_panel_carries_app_id_and_icon() {
    assert_identity(&builders(|ctx| {
        floating_panel_window(
            ctx,
            PanelId::Materials,
            Rect::from_min_size(pos2(10.0, 10.0), vec2(300.0, 200.0)),
            |ui| {
                ui.label("x");
            },
        );
    }));
}
