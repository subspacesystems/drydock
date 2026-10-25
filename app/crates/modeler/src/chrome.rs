// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Toolbar and menu bar of the window, kept apart from `App` so tests can drive them
//! with `egui_kittest`. Each function draws into the given `Ui` and returns the action the
//! user chose this frame, if any.
//!
//! Behaviour: `docs/specs/m3-app-ui.md`.

use crate::document::{Stats, format_size};
use crate::input::{Action, EditKey, Tool};
use modeler_view::StandardView;
use std::path::PathBuf;

/// What the menus need to know about the editor.
#[derive(Clone, Copy, Debug, Default)]
pub struct MenuState<'a> {
    /// Label of the step Undo would undo, if any.
    pub undo: Option<&'a str>,
    /// Label of the step Redo would redo, if any.
    pub redo: Option<&'a str>,
    /// The camera is in parallel projection.
    pub parallel: bool,
    /// Something is selected (Flip, Erase, Cut and Copy need it).
    pub has_selection: bool,
    /// The clipboard holds something (Paste needs it).
    pub has_clipboard: bool,
    /// Exactly one group or component is selected (Explode needs it).
    pub can_explode: bool,
    /// The rest of the model is hidden while editing inside an instance (the View menu
    /// item offers the opposite).
    pub hide_rest: bool,
    /// The axes are shown (the View menu's Axes item is checked; `docs/specs/m4-ui-axes.md`).
    pub axes: bool,
    /// The Shadows window is showing (the View menu's Shadows item is checked;
    /// `docs/specs/m5-app-shadows-ui.md`).
    pub shadows: bool,
    /// The File menu's recent files, most recent first (`docs/specs/m4-ui-menus.md`).
    pub recent: &'a [PathBuf],
    /// The current tool (checked in the Tools menu).
    pub tool: Tool,
}

/// The name prompt shown before Make Component / Make Group.
#[derive(Clone, Debug, PartialEq)]
pub struct NamePrompt {
    /// Making a group (else a component).
    pub group: bool,
    /// The text in the name field.
    pub name: String,
}

impl NamePrompt {
    /// A prompt with an empty name field.
    pub fn new(group: bool) -> NamePrompt {
        NamePrompt {
            group,
            name: String::new(),
        }
    }
}

/// How the name prompt was answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptOutcome {
    /// OK or Enter, with the name as typed (trimmed).
    Ok(String),
    /// Cancel, Escape or the window's close button.
    Cancel,
}

/// The name prompt window; `Some` on the frame it is answered.
/// Behaviour: `docs/specs/m4-app-components-ui.md`.
pub fn name_prompt(ctx: &eframe::egui::Context, prompt: &mut NamePrompt) -> Option<PromptOutcome> {
    use eframe::egui::{self, Key};

    let title = if prompt.group {
        "Make Group"
    } else {
        "Make Component"
    };
    let mut open = true;
    let mut outcome = None;
    egui::Window::new(title)
        .id(egui::Id::new("name_prompt"))
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label("Name");
            let field = ui.add(egui::TextEdit::singleline(&mut prompt.name));
            field.request_focus();
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    outcome = Some(PromptOutcome::Ok(prompt.name.trim().to_owned()));
                }
                if ui.button("Cancel").clicked() {
                    outcome = Some(PromptOutcome::Cancel);
                }
            });
        });

    if outcome.is_none() {
        let (enter, escape) =
            ctx.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
        if enter {
            outcome = Some(PromptOutcome::Ok(prompt.name.trim().to_owned()));
        } else if escape || !open {
            outcome = Some(PromptOutcome::Cancel);
        }
    }
    outcome
}

/// The toolbar's buttons, top to bottom: tool and button label.
pub const TOOL_BUTTONS: [(Tool, &str); 16] = [
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
    (Tool::Erase, "Erase"),
    (Tool::Paint, "Paint Bucket"),
    (Tool::Orbit, "Orbit"),
    (Tool::Pan, "Pan"),
    (Tool::Zoom, "Zoom"),
];

/// The tools by kind, in toolbar order: select, drawing, modifying, measuring and
/// painting, camera. The Tools menu puts a separator between groups.
pub const TOOL_GROUPS: [&[Tool]; 5] = [
    &[Tool::Select],
    &[
        Tool::Line,
        Tool::Rectangle,
        Tool::Circle,
        Tool::Arc,
        Tool::Arc3,
    ],
    &[Tool::PushPull, Tool::Move, Tool::Rotate, Tool::Scale],
    &[Tool::Tape, Tool::Erase, Tool::Paint],
    &[Tool::Orbit, Tool::Pan, Tool::Zoom],
];

/// Cut, Copy and Paste requested by this frame's events, in order. egui turns Ctrl+X/C/V
/// (Cmd on macOS) into `Event::Cut`/`Copy`/`Paste` and sends no key event for them.
/// Behaviour: `docs/specs/m4-fix-clipboard-keys.md`.
pub fn clipboard_actions(events: &[eframe::egui::Event]) -> Vec<Action> {
    use eframe::egui::{Event, Key};

    let mut actions = Vec::new();
    for event in events {
        let action = match event {
            Event::Copy => Some(Action::Copy),
            Event::Cut => Some(Action::Cut),
            Event::Paste(_) => Some(Action::Paste),
            Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } if shortcut_modifiers(modifiers) => match key {
                Key::C => Some(Action::Copy),
                Key::X => Some(Action::Cut),
                Key::V => Some(Action::Paste),
                _ => None,
            },
            _ => None,
        };
        if let Some(action) = action {
            actions.push(action);
        }
    }
    actions
}

/// Ctrl (or the platform command key) alone: the clipboard shortcut modifier.
fn shortcut_modifiers(modifiers: &eframe::egui::Modifiers) -> bool {
    (modifiers.ctrl || modifiers.command) && !modifiers.shift && !modifiers.alt
}

/// The key that picks `tool` (`input::key`), as shown in the Tools menu.
pub fn tool_shortcut(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "Space",
        Tool::Line => "L",
        Tool::Rectangle => "R",
        Tool::Circle => "C",
        Tool::Arc => "A",
        Tool::Arc3 => "Shift+A",
        Tool::PushPull => "P",
        Tool::Move => "M",
        Tool::Rotate => "Q",
        Tool::Scale => "S",
        Tool::Tape => "T",
        Tool::Erase => "E",
        Tool::Paint => "B",
        Tool::Orbit => "O",
        Tool::Pan => "H",
        Tool::Zoom => "Z",
    }
}

/// The File menu's text for each recent file, in list order (`docs/specs/m4-ui-menus.md`).
pub fn recent_labels(paths: &[PathBuf]) -> Vec<String> {
    let names: Vec<String> = paths
        .iter()
        .map(|path| match path.file_name() {
            Some(name) => name.to_string_lossy().into_owned(),
            None => path.display().to_string(),
        })
        .collect();
    paths
        .iter()
        .zip(&names)
        .enumerate()
        .map(|(index, (path, name))| {
            let mut label = format!("{}  {name}", index + 1);
            if names.iter().filter(|other| *other == name).count() > 1
                && let Some(folder) = path
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .map(|folder| folder.to_string_lossy())
            {
                label.push_str(&format!(" ({folder})"));
            }
            label
        })
        .collect()
}

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

/// The names in the repository's `CONTRIBUTORS` file.
pub const CONTRIBUTORS: &str = include_str!("../../../../CONTRIBUTORS");

/// The About window's lines, top to bottom (`docs/specs/m4-ui-menus.md`).
pub fn about_lines() -> Vec<String> {
    let mut lines = vec![
        about_text(),
        format!("License: {}", env!("CARGO_PKG_LICENSE")),
        "Copyright (c) 2026 Subspace Systems".to_owned(),
        "Contributors:".to_owned(),
    ];
    lines.extend(
        CONTRIBUTORS
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(str::to_owned),
    );
    lines
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
            for line in about_lines() {
                ui.label(line);
            }
            if ui.button("Close").clicked() {
                close = true;
            }
        });
    if close {
        *open = false;
    }
}

/// The Model Info window's lines, top to bottom (`docs/specs/m4-ui-status.md`).
pub fn model_info_lines(stats: &Stats, kernel: bool) -> Vec<String> {
    let mut lines = vec![
        format!("Triangles: {}", stats.triangles),
        format!("Lines: {}", stats.lines),
        format!("Materials: {}", stats.materials),
        format!("Size: {}", format_size(stats.size_m)),
    ];
    if kernel {
        lines.push(format!("Faces: {}", stats.faces));
        lines.push(format!("Edges: {}", stats.edges));
    }
    lines
}

/// The Model Info line naming the graphics adapter (`docs/specs/m4-ui-gpu-info.md`).
pub fn adapter_line(name: &str, backend: &str, device_type: &str) -> String {
    let name = name.trim();
    let name = if name.is_empty() {
        "unknown adapter"
    } else {
        name
    };
    let backend = backend.trim();
    let device_type = device_type.trim();
    let details = match (backend.is_empty(), device_type.is_empty()) {
        (false, false) => format!(" ({backend}, {device_type})"),
        (false, true) => format!(" ({backend})"),
        (true, false) => format!(" ({device_type})"),
        (true, true) => String::new(),
    };
    format!("Graphics: {name}{details}")
}

/// The Model Info window while `open`, showing `lines`; its Close button clears `open`.
pub fn model_info_window(ctx: &eframe::egui::Context, open: &mut bool, lines: &[String]) {
    model_info_window_with(ctx, open, lines, |_| {});
}

/// [`model_info_window`] with `extra` drawing more widgets under the lines, above Close.
pub fn model_info_window_with(
    ctx: &eframe::egui::Context,
    open: &mut bool,
    lines: &[String],
    extra: impl FnOnce(&mut eframe::egui::Ui),
) {
    if !*open {
        return;
    }
    let mut close = false;
    eframe::egui::Window::new("Model Info")
        .collapsible(false)
        .resizable(false)
        .open(open)
        .show(ctx, |ui| {
            for line in lines {
                ui.label(line);
            }
            extra(ui);
            if ui.button("Close").clicked() {
                close = true;
            }
        });
    if close {
        *open = false;
    }
}

/// The measurement in the status bar: the label and value of `measurement` (if any), then
/// the typed text (if any), drawn into `ui` (`docs/specs/m4-ui-status.md`).
pub fn status_measurement(
    ui: &mut eframe::egui::Ui,
    measurement: Option<(&str, &str)>,
    typed: &str,
) {
    if let Some((label, value)) = measurement {
        ui.label(label);
        if !value.is_empty() {
            ui.monospace(value);
        }
    }
    if !typed.is_empty() {
        ui.monospace(typed);
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
            if !state.recent.is_empty() {
                ui.separator();
                for (index, (path, label)) in state
                    .recent
                    .iter()
                    .zip(recent_labels(state.recent))
                    .enumerate()
                {
                    if ui
                        .button(label)
                        .on_hover_text(path.display().to_string())
                        .clicked()
                    {
                        chosen = Some(Action::OpenRecent(index));
                        ui.close();
                    }
                }
            }
            ui.separator();
            if ui.button("Exit").clicked() {
                chosen = Some(Action::Exit);
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
            if ui
                .add_enabled(
                    state.has_selection,
                    eframe::egui::Button::new("Cut    Ctrl+X"),
                )
                .clicked()
            {
                chosen = Some(Action::Cut);
                ui.close();
            }
            if ui
                .add_enabled(
                    state.has_selection,
                    eframe::egui::Button::new("Copy    Ctrl+C"),
                )
                .clicked()
            {
                chosen = Some(Action::Copy);
                ui.close();
            }
            if ui
                .add_enabled(
                    state.has_clipboard,
                    eframe::egui::Button::new("Paste    Ctrl+V"),
                )
                .clicked()
            {
                chosen = Some(Action::Paste);
                ui.close();
            }
            ui.separator();
            if ui
                .add_enabled(
                    state.has_selection,
                    eframe::egui::Button::new("Erase    Delete"),
                )
                .clicked()
            {
                chosen = Some(Action::Erase);
                ui.close();
            }
            ui.separator();
            if ui
                .add_enabled(
                    state.has_selection,
                    eframe::egui::Button::new("Make Component..."),
                )
                .clicked()
            {
                chosen = Some(Action::MakeComponent);
                ui.close();
            }
            if ui
                .add_enabled(
                    state.has_selection,
                    eframe::egui::Button::new("Make Group..."),
                )
                .clicked()
            {
                chosen = Some(Action::MakeGroup);
                ui.close();
            }
            if ui
                .add_enabled(state.can_explode, eframe::egui::Button::new("Explode"))
                .clicked()
            {
                chosen = Some(Action::Explode);
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
            ui.separator();
            let label = if state.hide_rest {
                "Show Rest of Model"
            } else {
                "Hide Rest of Model"
            };
            if ui.button(label).clicked() {
                chosen = Some(Action::ToggleHideRest);
                ui.close();
            }
            ui.separator();
            if ui.selectable_label(state.axes, "Axes").clicked() {
                chosen = Some(Action::ToggleAxes);
                ui.close();
            }
            if ui.selectable_label(state.shadows, "Shadows").clicked() {
                chosen = Some(Action::ToggleShadows);
                ui.close();
            }
            ui.separator();
            if ui.button("Model Info").clicked() {
                chosen = Some(Action::ModelInfo);
                ui.close();
            }
        });
        ui.menu_button("Tools", |ui| {
            for (group_index, group) in TOOL_GROUPS.iter().enumerate() {
                if group_index > 0 {
                    ui.separator();
                }
                for tool in *group {
                    let label = TOOL_BUTTONS
                        .iter()
                        .find(|(button, _)| button == tool)
                        .map(|(_, label)| *label)
                        .unwrap_or("");
                    let text = format!("{label}    {}", tool_shortcut(*tool));
                    if ui.selectable_label(state.tool == *tool, text).clicked() {
                        chosen = Some(Action::SelectTool(*tool));
                        ui.close();
                    }
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
