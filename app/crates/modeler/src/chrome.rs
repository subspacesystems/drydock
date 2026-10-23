// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Toolbar and menu bar of the window, kept apart from `App` so tests can drive them
//! with `egui_kittest`. Each function draws into the given `Ui` and returns the action the
//! user chose this frame, if any.
//!
//! Behaviour: `docs/specs/m3-app-ui.md`.

use crate::input::{Action, EditKey, Tool};
use modeler_view::StandardView;

/// What the menus need to know about the editor.
#[derive(Clone, Copy, Debug, Default)]
pub struct MenuState<'a> {
    /// Label of the step Undo would undo, if any.
    pub undo: Option<&'a str>,
    /// Label of the step Redo would redo, if any.
    pub redo: Option<&'a str>,
    /// The camera is in parallel projection.
    pub parallel: bool,
    /// Something is selected (Flip needs it).
    pub has_selection: bool,
}

/// The toolbar's buttons, top to bottom: tool and button label.
pub const TOOL_BUTTONS: [(Tool, &str); 14] = [
    (Tool::Select, "Select"),
    (Tool::Line, "Line"),
    (Tool::Rectangle, "Rectangle"),
    (Tool::Circle, "Circle"),
    (Tool::Arc, "Arc"),
    (Tool::Arc3, "3-Point Arc"),
    (Tool::PushPull, "Push/Pull"),
    (Tool::Move, "Move"),
    (Tool::Rotate, "Rotate"),
    (Tool::Scale, "Scale"),
    (Tool::Tape, "Tape Measure"),
    (Tool::Orbit, "Orbit"),
    (Tool::Pan, "Pan"),
    (Tool::Zoom, "Zoom"),
];

/// The tool buttons; `current` is drawn selected. Clicking one returns
/// `Action::SelectTool`.
pub fn toolbar(ui: &mut eframe::egui::Ui, current: Tool) -> Option<Action> {
    let mut chosen = None;
    ui.vertical(|ui| {
        ui.set_width(92.0);
        for (tool, label) in TOOL_BUTTONS {
            if ui.selectable_label(current == tool, label).clicked() {
                chosen = Some(Action::SelectTool(tool));
            }
        }
    });
    chosen
}

/// The commit the binary was built from (short hash, or "unknown").
pub const GIT_HASH: &str = env!("MODELER_GIT_HASH");

/// One line for the About window: name, version and commit.
pub fn about_text() -> String {
    format!("modeler {} ({GIT_HASH})", env!("CARGO_PKG_VERSION"))
}

/// The About window while `open`; its Close button clears `open`.
pub fn about_window(ctx: &eframe::egui::Context, open: &mut bool) {
    if !*open {
        return;
    }
    let mut close = false;
    eframe::egui::Window::new("About modeler")
        .collapsible(false)
        .resizable(false)
        .open(open)
        .show(ctx, |ui| {
            ui.label(about_text());
            if ui.button("Close").clicked() {
                close = true;
            }
        });
    if close {
        *open = false;
    }
}

/// The File, Edit, View and Help menus.
pub fn menu_bar(ui: &mut eframe::egui::Ui, state: &MenuState<'_>) -> Option<Action> {
    let mut chosen = None;
    eframe::egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("New    Ctrl+N").clicked() {
                chosen = Some(Action::New);
                ui.close();
            }
            if ui.button("Open...    Ctrl+O").clicked() {
                chosen = Some(Action::Open);
                ui.close();
            }
            ui.separator();
            if ui.button("Save    Ctrl+S").clicked() {
                chosen = Some(Action::Save);
                ui.close();
            }
            if ui.button("Save As...    Ctrl+Shift+S").clicked() {
                chosen = Some(Action::SaveAs);
                ui.close();
            }
        });
        ui.menu_button("Edit", |ui| {
            let undo_text = match state.undo {
                Some(label) => format!("Undo {label}    Ctrl+Z"),
                None => "Undo    Ctrl+Z".to_owned(),
            };
            if ui
                .add_enabled(state.undo.is_some(), eframe::egui::Button::new(undo_text))
                .clicked()
            {
                chosen = Some(Action::Edit(EditKey::Undo));
                ui.close();
            }
            let redo_text = match state.redo {
                Some(label) => format!("Redo {label}    Ctrl+Y"),
                None => "Redo    Ctrl+Y".to_owned(),
            };
            if ui
                .add_enabled(state.redo.is_some(), eframe::egui::Button::new(redo_text))
                .clicked()
            {
                chosen = Some(Action::Edit(EditKey::Redo));
                ui.close();
            }
            ui.separator();
            for (axis, label) in [
                (0, "Flip Along Red"),
                (1, "Flip Along Green"),
                (2, "Flip Along Blue"),
            ] {
                if ui
                    .add_enabled(state.has_selection, eframe::egui::Button::new(label))
                    .clicked()
                {
                    chosen = Some(Action::Flip(axis));
                    ui.close();
                }
            }
        });
        ui.menu_button("View", |ui| {
            let projection = if state.parallel {
                "Switch to Perspective    Shift+P"
            } else {
                "Switch to Parallel Projection    Shift+P"
            };
            if ui.button(projection).clicked() {
                chosen = Some(Action::ToggleProjection);
                ui.close();
            }
            if ui.button("Zoom Extents    Shift+Z").clicked() {
                chosen = Some(Action::ZoomExtents);
                ui.close();
            }
            ui.separator();
            for (label, view) in [
                ("Iso    F2", StandardView::Iso),
                ("Top    F3", StandardView::Top),
                ("Front    F4", StandardView::Front),
                ("Right    F5", StandardView::Right),
                ("Back    F6", StandardView::Back),
                ("Left    F7", StandardView::Left),
                ("Bottom    F8", StandardView::Bottom),
            ] {
                if ui.button(label).clicked() {
                    chosen = Some(Action::View(view));
                    ui.close();
                }
            }
        });
        ui.menu_button("Help", |ui| {
            if ui.button("About modeler").clicked() {
                chosen = Some(Action::About);
                ui.close();
            }
        });
    });
    chosen
}
