// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Toolbar and menu bar of the window, kept apart from `App` so tests can drive them
//! with `egui_kittest`. Each function draws into the given `Ui` and returns the action the
//! user chose this frame, if any.
//!
//! Behaviour: `docs/specs/m3-app-ui.md`.

use crate::dialog::{DialogButton, button_row, field_label};
use crate::dock::PanelId;
use crate::document::Stats;
use crate::icons::Icon;
use crate::input::{Action, EditKey, Tool};
use crate::toolbars::{self, ToolbarLayout, ToolbarsMenuEvent};
use eframe::egui::{
    Align2, InnerResponse, Label, Rect, Response, RichText, Sense, Shape, Stroke, TextStyle, Ui,
    WidgetInfo, WidgetType, pos2, vec2,
};
use modeler_kernel::units::{LengthFormat, format_with};
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
    /// Overhanging faces are tinted (the View menu's Overhangs item is checked;
    /// `docs/specs/m7-overhang.md`).
    pub overhangs: bool,
    /// A fit check is showing (the Tools menu's Fit Check item is checked;
    /// `docs/specs/m7-clearance-ui.md`).
    pub fit_check: bool,
    /// Exactly two groups or components are selected (Fit Check needs it).
    pub can_fit_check: bool,
    /// The Shadows window is showing (the View menu's Shadows item is checked;
    /// `docs/specs/m5-app-shadows-ui.md`).
    pub shadows: bool,
    /// The Entity Info panel is open (View menu; `docs/specs/m6-dock-ui.md`).
    pub entity_info: bool,
    /// The Materials panel is open (View menu; `docs/specs/m6-dock-ui.md`).
    pub materials: bool,
    /// The Components panel is open (View menu; `docs/specs/m6-dock-ui.md`).
    pub components: bool,
    /// The Texture Position panel is open (View menu; `docs/specs/m6-texture-panel.md`).
    pub texture_position: bool,
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

/// The clearance field: "Clearance", a one-line text box holding `text` (millimeters), "mm".
/// When the box loses focus or Enter is pressed: a `text` that `fit_check::parse_clearance_mm`
/// accepts is rewritten with `format_mm` and returned (meters); any other text is replaced
/// by `format_mm(current_m)` and `None` is returned. Otherwise `None`.
/// Behaviour: `docs/specs/m7-clearance-ui.md`.
pub fn clearance_field(
    ui: &mut eframe::egui::Ui,
    text: &mut String,
    current_m: f64,
) -> Option<f64> {
    use eframe::egui;

    let edited_id = ui.make_persistent_id("clearance_field_edited");
    let mut edited = ui
        .data(|data| data.get_temp::<bool>(edited_id))
        .unwrap_or(false);
    let mut committed = None;
    ui.horizontal(|ui| {
        ui.label("Clearance");
        let response = ui.add(egui::TextEdit::singleline(text).desired_width(48.0));
        ui.label("mm");
        edited |= response.changed();
        if edited && response.lost_focus() {
            edited = false;
            if let Some(meters) = crate::fit_check::parse_clearance_mm(text) {
                *text = crate::fit_check::format_mm(meters);
                committed = Some(meters);
            } else {
                *text = crate::fit_check::format_mm(current_m);
            }
        }
    });
    ui.data_mut(|data| data.insert_temp(edited_id, edited));
    committed
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
            ui.label(field_label("Name"));
            let field = ui.add(egui::TextEdit::singleline(&mut prompt.name));
            field.request_focus();
            match button_row(
                ui,
                &[
                    DialogButton {
                        label: "OK",
                        default: true,
                        enabled: true,
                    },
                    DialogButton {
                        label: "Cancel",
                        default: false,
                        enabled: true,
                    },
                ],
            ) {
                Some(0) => {
                    outcome = Some(PromptOutcome::Ok(prompt.name.trim().to_owned()));
                }
                Some(1) => outcome = Some(PromptOutcome::Cancel),
                _ => {}
            }
        });

    if outcome.is_none() {
        let escape = ctx.input(|i| i.key_pressed(Key::Escape));
        if escape || !open {
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
    (Tool::PushPull, "Extrude"),
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
            if button_row(
                ui,
                &[DialogButton {
                    label: "Close",
                    default: false,
                    enabled: true,
                }],
            ) == Some(0)
            {
                close = true;
            }
        });
    if close {
        *open = false;
    }
}

/// The Model Info window's lines, top to bottom (`docs/specs/m4-ui-status.md`); the size in
/// `format` (`docs/specs/m6-units-ui.md`).
pub fn model_info_lines(stats: &Stats, kernel: bool, format: LengthFormat) -> Vec<String> {
    let mut lines = vec![
        format!("Triangles: {}", stats.triangles),
        format!("Lines: {}", stats.lines),
        format!("Materials: {}", stats.materials),
        format!(
            "Size: {} x {} x {}",
            format_with(stats.size_m[0], format),
            format_with(stats.size_m[1], format),
            format_with(stats.size_m[2], format)
        ),
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
            if button_row(
                ui,
                &[DialogButton {
                    label: "Close",
                    default: false,
                    enabled: true,
                }],
            ) == Some(0)
            {
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

/// Where the parts of one menu row go (`docs/specs/m6-menus.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemLayout {
    /// The icon's square, if the row has an icon.
    pub icon: Option<Rect>,
    /// Left edge of the label text.
    pub label_left: f32,
    /// The shortcut text's box, right-aligned in the row.
    pub shortcut: Rect,
}

/// Lay out a menu row of `row` for a shortcut text `shortcut_width` wide.
pub fn menu_item_layout(row: Rect, has_icon: bool, shortcut_width: f32) -> ItemLayout {
    let icon = has_icon.then(|| {
        Rect::from_center_size(
            pos2(row.left() + 4.0 + 8.0, row.center().y),
            vec2(16.0, 16.0),
        )
    });
    let label_left = row.left() + 26.0;
    let shortcut = Rect::from_min_size(
        pos2(row.right() - 6.0 - shortcut_width, row.top()),
        vec2(shortcut_width, row.height()),
    );
    ItemLayout {
        icon,
        label_left,
        shortcut,
    }
}

/// One menu row: optional icon, `label`, and `shortcut` right-aligned (empty for none).
pub fn menu_item(
    ui: &mut Ui,
    icon: Option<Icon>,
    label: &str,
    shortcut: &str,
    selected: bool,
) -> Response {
    let font_id = TextStyle::Body.resolve(ui.style());
    let text_color = ui.visuals().text_color();
    let label_galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font_id.clone(), text_color);
    let shortcut_galley = (!shortcut.is_empty()).then(|| {
        ui.painter()
            .layout_no_wrap(shortcut.to_owned(), font_id.clone(), text_color)
    });
    let shortcut_width = shortcut_galley
        .as_ref()
        .map_or(0.0, |galley| galley.size().x);
    let row_height = ui
        .spacing()
        .interact_size
        .y
        .max(label_galley.size().y)
        .max(
            shortcut_galley
                .as_ref()
                .map_or(0.0, |galley| galley.size().y),
        )
        .max(if icon.is_some() { 16.0 } else { 0.0 });
    let mut needed = 26.0 + label_galley.size().x + 6.0;
    if !shortcut.is_empty() {
        needed += 24.0 + shortcut_width;
    }
    let width = if ui.is_sizing_pass() {
        needed
    } else {
        ui.available_width().max(needed)
    };
    let (row, response) = ui.allocate_exact_size(vec2(width, row_height), Sense::click());

    let layout = menu_item_layout(row, icon.is_some(), shortcut_width);
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, selected, label));

    let visuals = ui.style().interact_selectable(&response, selected);
    let painter = ui.painter().clone();
    if response.hovered() {
        painter.rect_filled(row, 4.0, visuals.weak_bg_fill);
    }
    let color = visuals.fg_stroke.color;
    if let (Some(icon), Some(icon_rect)) = (icon, layout.icon) {
        if selected {
            let box_rect = icon_rect.expand(3.0);
            painter.rect(
                box_rect,
                3.0,
                ui.visuals().selection.bg_fill,
                Stroke::new(1.0, ui.visuals().selection.stroke.color),
                eframe::egui::StrokeKind::Inside,
            );
        }
        crate::icons::paint(&painter, icon_rect, icon, color);
    }
    painter.text(
        pos2(layout.label_left, row.center().y),
        Align2::LEFT_CENTER,
        label,
        font_id.clone(),
        color,
    );
    if let Some(galley) = shortcut_galley {
        let rect = Rect::from_min_size(
            pos2(
                layout.shortcut.right() - galley.size().x,
                layout.shortcut.center().y - galley.size().y / 2.0,
            ),
            galley.size(),
        );
        ui.place(
            rect,
            Label::new(RichText::new(shortcut).font(font_id)).selectable(false),
        );
    }
    response
}

/// A menu row that shows or hides something: a check mark in the icon column while
/// `checked`, no row highlight for it (`docs/specs/m6-menus-2.md`).
pub fn menu_check_item(ui: &mut Ui, label: &str, checked: bool) -> Response {
    let font_id = TextStyle::Body.resolve(ui.style());
    let text_color = ui.visuals().text_color();
    let label_galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font_id.clone(), text_color);
    let row_height = ui.spacing().interact_size.y.max(label_galley.size().y);
    let needed = 26.0 + label_galley.size().x + 6.0;
    let width = if ui.is_sizing_pass() {
        needed
    } else {
        ui.available_width().max(needed)
    };
    let (row, response) = ui.allocate_exact_size(vec2(width, row_height), Sense::click());

    let layout = menu_item_layout(row, true, 0.0);
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, checked, label));

    let visuals = ui.style().interact(&response);
    let painter = ui.painter().clone();
    if response.hovered() {
        painter.rect_filled(row, 4.0, visuals.weak_bg_fill);
    }
    let color = visuals.fg_stroke.color;
    if checked && let Some(icon) = layout.icon {
        let p1 = pos2(icon.left() + 3.5, icon.center().y);
        let p2 = pos2(icon.left() + 6.5, icon.center().y + 3.0);
        let p3 = pos2(icon.right() - 3.0, icon.center().y - 3.0);
        painter.add(Shape::line(vec![p1, p2, p3], Stroke::new(1.8, color)));
    }
    painter.text(
        pos2(layout.label_left, row.center().y),
        Align2::LEFT_CENTER,
        label,
        font_id,
        color,
    );
    response
}

/// A submenu row with a drawn arrow; `add_contents` fills the submenu.
pub fn submenu_button<R>(
    ui: &mut Ui,
    label: &str,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<Option<R>> {
    let font_id = TextStyle::Body.resolve(ui.style());
    let text_color = ui.visuals().text_color();
    let label_galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font_id.clone(), text_color);
    let row_height = ui.spacing().interact_size.y.max(label_galley.size().y);
    let needed = 26.0 + label_galley.size().x + 24.0 + 6.0;
    let width = if ui.is_sizing_pass() {
        needed
    } else {
        ui.available_width().max(needed)
    };
    let (row, response) = ui.allocate_exact_size(vec2(width, row_height), Sense::click());

    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));

    let visuals = ui.style().interact(&response);
    let painter = ui.painter().clone();
    if response.hovered() {
        painter.rect_filled(row, 4.0, visuals.weak_bg_fill);
    }
    let color = visuals.fg_stroke.color;
    painter.text(
        pos2(menu_item_layout(row, false, 0.0).label_left, row.center().y),
        Align2::LEFT_CENTER,
        label,
        font_id,
        color,
    );
    let cx = row.right() - 12.0;
    let cy = row.center().y;
    painter.add(Shape::convex_polygon(
        vec![
            pos2(cx - 4.0, cy - 5.0),
            pos2(cx + 4.0, cy),
            pos2(cx - 4.0, cy + 5.0),
        ],
        color,
        Stroke::NONE,
    ));

    let popup = eframe::egui::containers::menu::SubMenu::new().show(ui, &response, add_contents);
    InnerResponse::new(popup.map(|inner| inner.inner), response)
}

/// What the menu bar did this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct MenuOutcome {
    /// The menu action the user chose, if any.
    pub action: Option<Action>,
    /// The View > Toolbars menu changed the layout.
    pub toolbars_changed: bool,
    /// The user asked for the Customize Toolbars window.
    pub customize_toolbars: bool,
    /// The user chose View > Toolbars > Reset Toolbars... (`docs/specs/m6-confirm.md`).
    pub reset_toolbars: bool,
}

/// The File, Edit, View and Help menus.
pub fn menu_bar(ui: &mut eframe::egui::Ui, state: &MenuState<'_>) -> Option<Action> {
    menu_bar_inner(ui, state, None).action
}

/// The menus with the View > Toolbars submenu wired to `toolbars`.
pub fn menu_bar_with_toolbars(
    ui: &mut eframe::egui::Ui,
    state: &MenuState<'_>,
    toolbars: &mut ToolbarLayout,
) -> MenuOutcome {
    menu_bar_inner(ui, state, Some(toolbars))
}

fn menu_bar_inner(
    ui: &mut eframe::egui::Ui,
    state: &MenuState<'_>,
    toolbars: Option<&mut ToolbarLayout>,
) -> MenuOutcome {
    let mut chosen = None;
    let mut toolbars_changed = false;
    let mut customize_toolbars = false;
    let mut reset_toolbars = false;
    eframe::egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if menu_item(ui, None, "New", "Ctrl+N", false).clicked() {
                chosen = Some(Action::New);
                ui.close();
            }
            if menu_item(ui, None, "Open...", "Ctrl+O", false).clicked() {
                chosen = Some(Action::Open);
                ui.close();
            }
            ui.separator();
            if menu_item(ui, None, "Save", "Ctrl+S", false).clicked() {
                chosen = Some(Action::Save);
                ui.close();
            }
            if menu_item(ui, None, "Save As...", "Ctrl+Shift+S", false).clicked() {
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
                    if menu_item(ui, None, &label, "", false)
                        .on_hover_text(path.display().to_string())
                        .clicked()
                    {
                        chosen = Some(Action::OpenRecent(index));
                        ui.close();
                    }
                }
            }
            ui.separator();
            if menu_item(ui, None, "Exit", "", false).clicked() {
                chosen = Some(Action::Exit);
                ui.close();
            }
        });
        ui.menu_button("Edit", |ui| {
            let undo_label = match state.undo {
                Some(label) => format!("Undo {label}"),
                None => "Undo".to_owned(),
            };
            let undo = ui
                .add_enabled_ui(state.undo.is_some(), |ui| {
                    menu_item(ui, None, &undo_label, "Ctrl+Z", false)
                })
                .inner;
            if undo.clicked() {
                chosen = Some(Action::Edit(EditKey::Undo));
                ui.close();
            }
            let redo_label = match state.redo {
                Some(label) => format!("Redo {label}"),
                None => "Redo".to_owned(),
            };
            let redo = ui
                .add_enabled_ui(state.redo.is_some(), |ui| {
                    menu_item(ui, None, &redo_label, "Ctrl+Y", false)
                })
                .inner;
            if redo.clicked() {
                chosen = Some(Action::Edit(EditKey::Redo));
                ui.close();
            }
            ui.separator();
            let cut = ui
                .add_enabled_ui(state.has_selection, |ui| {
                    menu_item(ui, None, "Cut", "Ctrl+X", false)
                })
                .inner;
            if cut.clicked() {
                chosen = Some(Action::Cut);
                ui.close();
            }
            let copy = ui
                .add_enabled_ui(state.has_selection, |ui| {
                    menu_item(ui, None, "Copy", "Ctrl+C", false)
                })
                .inner;
            if copy.clicked() {
                chosen = Some(Action::Copy);
                ui.close();
            }
            let paste = ui
                .add_enabled_ui(state.has_clipboard, |ui| {
                    menu_item(ui, None, "Paste", "Ctrl+V", false)
                })
                .inner;
            if paste.clicked() {
                chosen = Some(Action::Paste);
                ui.close();
            }
            ui.separator();
            let erase = ui
                .add_enabled_ui(state.has_selection, |ui| {
                    menu_item(ui, None, "Erase", "Delete", false)
                })
                .inner;
            if erase.clicked() {
                chosen = Some(Action::Erase);
                ui.close();
            }
            ui.separator();
            let make_component = ui
                .add_enabled_ui(state.has_selection, |ui| {
                    menu_item(ui, None, "Make Component...", "", false)
                })
                .inner;
            if make_component.clicked() {
                chosen = Some(Action::MakeComponent);
                ui.close();
            }
            let make_group = ui
                .add_enabled_ui(state.has_selection, |ui| {
                    menu_item(ui, None, "Make Group...", "", false)
                })
                .inner;
            if make_group.clicked() {
                chosen = Some(Action::MakeGroup);
                ui.close();
            }
            let explode = ui
                .add_enabled_ui(state.can_explode, |ui| {
                    menu_item(ui, None, "Explode", "", false)
                })
                .inner;
            if explode.clicked() {
                chosen = Some(Action::Explode);
                ui.close();
            }
            ui.separator();
            for (axis, label) in [
                (0, "Flip Along Red"),
                (1, "Flip Along Green"),
                (2, "Flip Along Blue"),
            ] {
                let flip = ui
                    .add_enabled_ui(state.has_selection, |ui| {
                        menu_item(ui, None, label, "", false)
                    })
                    .inner;
                if flip.clicked() {
                    chosen = Some(Action::Flip(axis));
                    ui.close();
                }
            }
        });
        ui.menu_button("View", |ui| {
            let projection = if state.parallel {
                "Switch to Perspective"
            } else {
                "Switch to Parallel Projection"
            };
            if menu_item(ui, None, projection, "Shift+P", false).clicked() {
                chosen = Some(Action::ToggleProjection);
                ui.close();
            }
            if menu_item(ui, None, "Zoom Extents", "Shift+Z", false).clicked() {
                chosen = Some(Action::ZoomExtents);
                ui.close();
            }
            ui.separator();
            for (label, shortcut, view) in [
                ("Iso", "F2", StandardView::Iso),
                ("Top", "F3", StandardView::Top),
                ("Front", "F4", StandardView::Front),
                ("Right", "F5", StandardView::Right),
                ("Back", "F6", StandardView::Back),
                ("Left", "F7", StandardView::Left),
                ("Bottom", "F8", StandardView::Bottom),
            ] {
                if menu_item(ui, None, label, shortcut, false).clicked() {
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
            if menu_item(ui, None, label, "", false).clicked() {
                chosen = Some(Action::ToggleHideRest);
                ui.close();
            }
            ui.separator();
            if menu_check_item(ui, "Axes", state.axes).clicked() {
                chosen = Some(Action::ToggleAxes);
                ui.close();
            }
            if menu_check_item(ui, "Overhangs", state.overhangs).clicked() {
                chosen = Some(Action::ToggleOverhangs);
                ui.close();
            }
            ui.separator();
            for (open, label, action) in [
                (
                    state.entity_info,
                    "Entity Info",
                    Action::TogglePanel(PanelId::EntityInfo),
                ),
                (
                    state.materials,
                    "Materials",
                    Action::TogglePanel(PanelId::Materials),
                ),
                (
                    state.components,
                    "Components",
                    Action::TogglePanel(PanelId::Components),
                ),
                (state.shadows, "Shadows", Action::ToggleShadows),
                (
                    state.texture_position,
                    "Texture Position",
                    Action::TogglePanel(PanelId::TexturePosition),
                ),
            ] {
                if menu_check_item(ui, label, open).clicked() {
                    chosen = Some(action);
                    ui.close();
                }
            }
            if let Some(layout) = toolbars {
                ui.separator();
                submenu_button(ui, "Toolbars", |ui| {
                    match toolbars::toolbars_menu(ui, layout) {
                        Some(ToolbarsMenuEvent::Changed) => toolbars_changed = true,
                        Some(ToolbarsMenuEvent::Customize) => {
                            customize_toolbars = true;
                            ui.close();
                        }
                        Some(ToolbarsMenuEvent::Reset) => {
                            reset_toolbars = true;
                            ui.close();
                        }
                        None => {}
                    }
                });
            }
            if menu_item(ui, None, "Reset Panels...", "", false).clicked() {
                chosen = Some(Action::ResetPanels);
                ui.close();
            }
            ui.separator();
            if menu_item(ui, None, "Model Info...", "", false).clicked() {
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
                    if menu_item(
                        ui,
                        Some(Icon::Tool(*tool)),
                        label,
                        tool_shortcut(*tool),
                        state.tool == *tool,
                    )
                    .clicked()
                    {
                        chosen = Some(Action::SelectTool(*tool));
                        ui.close();
                    }
                }
            }
            ui.separator();
            let fit_check = ui
                .add_enabled_ui(state.can_fit_check || state.fit_check, |ui| {
                    menu_check_item(ui, "Fit Check", state.fit_check)
                })
                .inner;
            if fit_check.clicked() {
                chosen = Some(Action::FitCheck);
                ui.close();
            }
        });
        ui.menu_button("Help", |ui| {
            if menu_item(ui, None, "About modeler...", "", false).clicked() {
                chosen = Some(Action::About);
                ui.close();
            }
        });
    });

    MenuOutcome {
        action: chosen,
        toolbars_changed,
        customize_toolbars,
        reset_toolbars,
    }
}
