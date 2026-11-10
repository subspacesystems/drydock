// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Toolbar and menu bar of the window, kept apart from `App` so tests can drive them
//! with `egui_kittest`. Each function draws into the given `Ui` and returns the action the
//! user chose this frame, if any.
//!
//! Behaviour: `docs/specs/m3-app-ui.md`.

use crate::dialog::{
    DialogButton, DialogWindow, button_row, dialog_window, field_label, focus_when_window_active,
    modal_dialog_window,
};
use crate::dock::PanelId;
use crate::document::Stats;
use crate::icons::Icon;
use crate::input::{Action, EditKey, Tool};
use crate::mesh_export::ExportFormat;
use crate::popup::{
    PopupMode, PopupWindow, popup_mode, popup_root_inner, popup_supported, popup_window,
    wayland_session,
};
use crate::toolbars::{self, ToolbarLayout, ToolbarsMenuEvent};
use drydock_kernel::units::{LengthFormat, format_with};
use drydock_view::StandardView;
use eframe::egui::{
    Align2, Id, InnerResponse, Label, Rect, Response, RichText, ScrollArea, Sense, Shape, Stroke,
    TextStyle, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2,
};
use std::path::PathBuf;
use std::time::Duration;

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
    /// The selection holds at least one group or component (Grow by Clearance and Shrink by
    /// Clearance need it; `docs/specs/m7-clearance-offset-ui.md`).
    pub can_offset: bool,
    /// A model is open (the Tools menu's 3D Text... item needs it;
    /// `docs/specs/m7-text-3d-ui.md`).
    pub can_text: bool,
    /// The model holds at least one group or component (Export 3MF... and Export STL...
    /// need it; `docs/specs/m7-export-ui.md`).
    pub can_export: bool,
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
    /// The Tags panel is open.
    pub tags: bool,
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
    use eframe::egui::{self, Vec2};

    let title = if prompt.group {
        "Make Group"
    } else {
        "Make Component"
    };
    let mut open = true;
    let mut outcome = None;
    let window = DialogWindow {
        title,
        size: Vec2::new(340.0, 130.0),
    };
    modal_dialog_window(
        ctx,
        egui::Id::new("name_prompt"),
        &window,
        &mut open,
        |ui| {
            let field_id = egui::Id::new("name_prompt_field");
            focus_when_window_active(ui, field_id);
            ui.label(field_label("Name"));
            ui.add(egui::TextEdit::singleline(&mut prompt.name).id(field_id));
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
        },
    );

    if outcome.is_none() && !open {
        outcome = Some(PromptOutcome::Cancel);
    }
    outcome
}

/// The toolbar's buttons, top to bottom: tool and button label.
pub const TOOL_BUTTONS: [(Tool, &str); 18] = [
    (Tool::Select, "Select"),
    (Tool::Line, "Line"),
    (Tool::Rectangle, "Rectangle"),
    (Tool::Circle, "Circle"),
    (Tool::Arc, "Arc"),
    (Tool::Arc3, "3-Point Arc"),
    (Tool::PushPull, "Extrude"),
    (Tool::Offset, "Offset"),
    (Tool::FollowMe, "Follow Me"),
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
    &[
        Tool::PushPull,
        Tool::Offset,
        Tool::FollowMe,
        Tool::Move,
        Tool::Rotate,
        Tool::Scale,
    ],
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
        Tool::Offset => "I",
        Tool::FollowMe => "F",
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
pub const GIT_HASH: &str = env!("DRYDOCK_GIT_HASH");

/// The release version of this build (`DRYDOCK_VERSION` when the build sets it).
pub const VERSION: &str = env!("DRYDOCK_VERSION");

/// One line for the About window: name, version and commit.
pub fn about_text() -> String {
    format!("Drydock {VERSION} ({GIT_HASH})")
}

/// The line under the version in the About window.
pub const PRODUCT_LINE: &str = "A Subspace Systems product.";

/// Width in points of the lockup image in the About window; the height follows the SVG's
/// aspect ratio (3410 x 748).
pub const LOCKUP_WIDTH: f32 = 341.0;

/// The brand lockup SVG for the dark theme (`dark`) or the light one.
pub fn lockup_svg(dark: bool) -> &'static [u8] {
    if dark {
        include_bytes!("../../../../design/brand/logo/drydock-lockup-h-dark.svg")
    } else {
        include_bytes!("../../../../design/brand/logo/drydock-lockup-h-light.svg")
    }
}

/// The image URI the lockup is registered under: distinct for the two themes, ending `.svg`.
pub fn lockup_uri(dark: bool) -> &'static str {
    if dark {
        "bytes://drydock-lockup-dark.svg"
    } else {
        "bytes://drydock-lockup-light.svg"
    }
}

/// Registers both lockups with `ctx` (and the SVG loader); safe to call repeatedly.
pub fn install_lockups(ctx: &eframe::egui::Context) {
    egui_extras::install_image_loaders(ctx);
    for dark in [false, true] {
        ctx.include_bytes(lockup_uri(dark), lockup_svg(dark));
    }
}

/// The names in the repository's `CONTRIBUTORS` file.
pub const CONTRIBUTORS: &str = include_str!("../../../../CONTRIBUTORS");

/// The About window's lines, top to bottom (`docs/specs/m4-ui-menus.md`).
pub fn about_lines() -> Vec<String> {
    let mut lines = vec![
        about_text(),
        PRODUCT_LINE.to_owned(),
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
    if !ctx.fonts(|fonts| fonts.families().contains(&crate::look::medium())) {
        ctx.set_fonts(crate::look::font_definitions());
    }
    install_lockups(ctx);
    let mut close = false;
    let window = DialogWindow {
        title: "About Drydock",
        size: vec2(380.0, 440.0),
    };
    dialog_window(ctx, &window, open, |ui| {
        let dark = ui.visuals().dark_mode;
        ui.add(
            eframe::egui::Image::new(lockup_uri(dark))
                .fit_to_exact_size(vec2(LOCKUP_WIDTH, LOCKUP_WIDTH * 748.0 / 3410.0))
                .alt_text("Subspace Drydock"),
        );
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
    let window = DialogWindow {
        title: "Model Info",
        size: vec2(360.0, 300.0),
    };
    dialog_window(ctx, &window, open, |ui| {
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

/// The corners of the Measurements box (the status bar's measurement and typed text):
/// 2 points on the left, 8 on the right (`docs/specs/m10-brand-numbers.md`).
pub fn measurements_corners() -> eframe::egui::CornerRadius {
    eframe::egui::CornerRadius {
        nw: 2,
        sw: 2,
        ne: 8,
        se: 8,
    }
}

/// The measurement in the status bar: the label and value of `measurement` (if any), then
/// the typed text (if any), drawn into `ui` (`docs/specs/m4-ui-status.md`). The value and
/// the typed text are monospace; what is shown goes inside a frame with
/// [`measurements_corners`], nothing shown no frame (`docs/specs/m10-brand-numbers.md`).
pub fn status_measurement(
    ui: &mut eframe::egui::Ui,
    measurement: Option<(&str, &str)>,
    typed: &str,
) {
    if measurement.is_none() && typed.is_empty() {
        return;
    }
    let widget = ui.style().visuals.widgets.noninteractive;
    let frame = eframe::egui::Frame::new()
        .inner_margin(eframe::egui::Margin::symmetric(8, 3))
        .corner_radius(measurements_corners())
        .fill(widget.bg_fill)
        .stroke(widget.bg_stroke);
    frame.show(ui, |ui| {
        if let Some((label, value)) = measurement {
            ui.label(label);
            if !value.is_empty() {
                ui.monospace(value);
            }
        }
        if !typed.is_empty() {
            ui.monospace(typed);
        }
    });
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
    let in_submenu = ui
        .ctx()
        .data(|data| data.get_temp::<bool>(Id::new(IN_SUBMENU)).unwrap_or(false));
    let role = if open_submenu(ui.ctx()).is_some() && !in_submenu {
        WidgetType::Button
    } else {
        WidgetType::Checkbox
    };
    response.widget_info(|| WidgetInfo::selected(role, enabled, checked, label));

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

/// Room left between a menu's bottom and the window's bottom edge.
pub(crate) const MENU_MARGIN: f32 = 8.0;

/// The menu bar's bottom, put in the context for [`submenu_button`] to cap submenus.
const MENU_BAR_BOTTOM: &str = "drydock_menu_bar_bottom";

/// The label of the top-level menu that is open now.
const OPEN_MENU: &str = "drydock_open_menu";

/// The label of the submenu that is open now.
const OPEN_SUBMENU: &str = "drydock_open_submenu";

/// The submenu row's rectangle, for placing its pop-up.
const SUBMENU_ROW: &str = "drydock_submenu_row";

/// The submenu pop-up's last content size.
const SUBMENU_SIZE: &str = "drydock_submenu_size";

/// Whether the rows being drawn are the open submenu's (so its check items stay check boxes).
const IN_SUBMENU: &str = "drydock_in_submenu";

/// A top-level menu title's rectangle, kept for [`request_menu`].
const MENU_TITLE: &str = "drydock_menu_title";

/// A menu step requested by a focused pop-up window, waiting for the next `menu_bar`.
const MENU_STEP: &str = "drydock_menu_step";

/// The menu a step moved to and when, while its pop-up is still due to take focus.
const MENU_STEP_FOCUS: &str = "drydock_menu_step_focus";

/// When the root viewport lost focus, for [`close_menus_from_root`]'s grace time.
const ROOT_UNFOCUSED_SINCE: &str = "drydock_root_unfocused_since";

/// The root viewport's focus state on the previous frame.
const ROOT_FOCUSED_LAST: &str = "drydock_root_focused_last";

/// The frame time of the last [`close_menus_from_root`] call.
const ROOT_CLOSE_TIME: &str = "drydock_root_close_time";

/// A menu that closed because its pop-up lost focus, and when (to keep the title click that
/// caused the loss from reopening it).
const MENU_CLOSED: &str = "drydock_menu_closed";

/// The root viewport's inner rectangle (a pop-up's own `inner_rect` is its window's).
const ROOT_INNER: &str = "drydock_root_inner";

/// The labels of the menu bar's menus, in order.
const MENU_LABELS: [&str; 5] = ["File", "Edit", "View", "Tools", "Help"];

/// Wrap a menu's rows so content that doesn't fit scrolls inside the menu instead of
/// overflowing the window.
fn menu_scroll<R>(ui: &mut Ui, max_height: f32, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let max_height = max_height.max(0.0);
    ui.set_max_height(max_height);
    ScrollArea::vertical()
        .auto_shrink([true, true])
        .max_height(max_height)
        .show(ui, add_contents)
        .inner
}

/// A menu's rows in a scope that [`Ui::close`] can close. Returns the rows' result and
/// whether they asked to close.
pub(crate) fn popup_rows<R>(
    ui: &mut Ui,
    max_height: f32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> (R, bool) {
    ui.scope_builder(UiBuilder::new().closable(), |ui| {
        let inner = menu_scroll(ui, max_height, add_contents);
        (inner, ui.should_close())
    })
    .inner
}

/// The height cap for a pop-up menu's rows.
pub(crate) fn popup_max_height(
    ui: &Ui,
    mode: PopupMode,
    popup_y: f32,
    inline_bar_bottom: f32,
) -> f32 {
    let frame = ui.spacing().menu_margin.sum().y;
    let inline = ui.ctx().content_rect().bottom() - inline_bar_bottom - MENU_MARGIN - frame;
    match mode {
        PopupMode::Window => ui
            .ctx()
            .input(|input| input.viewport().monitor_size)
            .map(|monitor| monitor.y - popup_y - MENU_MARGIN - frame)
            .unwrap_or(inline),
        PopupMode::Inline => inline,
    }
}

/// Clears the open submenu.
fn clear_submenu(ctx: &eframe::egui::Context) {
    let label = open_submenu(ctx);
    ctx.data_mut(|data| {
        data.remove_temp::<String>(Id::new(OPEN_SUBMENU));
        data.remove::<Rect>(Id::new(SUBMENU_ROW));
        data.remove_temp::<eframe::egui::Vec2>(Id::new(SUBMENU_SIZE));
    });
    if let Some(label) = label
        && popup_mode(ctx.embed_viewports(), popup_supported()) == PopupMode::Window
    {
        let title = submenu_popup_title(&label);
        ctx.send_viewport_cmd_to(
            crate::popup::popup_viewport_id(&title),
            eframe::egui::ViewportCommand::Close,
        );
    }
}

/// Puts back the window flags of a pop-up whose focus-loss close we ignored (its submenu
/// has the focus).
fn restore_popup_window_state(ctx: &eframe::egui::Context, title: &str) {
    let viewport_id = crate::popup::popup_viewport_id(title);
    ctx.data_mut(|data| {
        data.insert_temp(viewport_id.0.with("popup-shown"), true);
        data.insert_temp(viewport_id.0.with("popup-focused"), true);
    });
}

/// The pop-up title (and viewport name) of the top-level menu labelled `label`, e.g. "File":
/// distinct per label, never empty (`docs/specs/m12-menu-popups.md`).
pub fn menu_popup_title(label: &str) -> String {
    format!("drydock-menu-{label}")
}

/// Where a top-level menu's pop-up goes: the left edge and bottom of its title button
/// (`title`, in the calling viewport's points) in screen points, i.e. plus `root_inner.min`
/// when that is known.
pub fn menu_popup_position(root_inner: Option<Rect>, title: Rect) -> eframe::egui::Pos2 {
    let position = title.left_bottom();
    root_inner.map_or(position, |root| position + root.min.to_vec2())
}

/// The label of the top-level menu that is open now, if any.
pub fn open_menu(ctx: &eframe::egui::Context) -> Option<String> {
    ctx.data(|data| data.get_temp::<String>(Id::new(OPEN_MENU)))
}

/// Sets (or clears) the open top-level menu.
fn set_open_menu(ctx: &eframe::egui::Context, label: Option<&str>) {
    ctx.data_mut(|data| {
        if let Some(label) = label {
            data.insert_temp(Id::new(OPEN_MENU), label.to_owned());
        } else {
            data.remove_temp::<String>(Id::new(OPEN_MENU));
        }
    });
}

/// Opens the top-level menu labelled `label` as if its title had been clicked (an unknown
/// label opens nothing). The next `menu_bar` call shows it.
pub fn request_menu(ctx: &eframe::egui::Context, label: &str) {
    if MENU_LABELS.contains(&label) {
        set_open_menu(ctx, Some(label));
    }
}

/// The pop-up title (and viewport name) of the submenu labelled `label` (e.g. "Toolbars"):
/// distinct per label and from every [`menu_popup_title`] (`docs/specs/m12-submenu-popups.md`).
pub fn submenu_popup_title(label: &str) -> String {
    format!("drydock-submenu-{label}")
}

/// Where a submenu's pop-up of size `size` goes, in screen points. `row` is the submenu's
/// row in the calling viewport's points. Preferred: its top-left at the row's right edge and
/// top, plus `root_inner.min` when known. When `monitor` (the monitor size) is known and the
/// pop-up would pass its right edge, its right edge goes at the row's left edge instead; when
/// it would pass the bottom, it moves up until its bottom is at the monitor's bottom. Never
/// left of or above 0 in either direction.
pub fn submenu_popup_position(
    root_inner: Option<Rect>,
    row: Rect,
    size: eframe::egui::Vec2,
    monitor: Option<eframe::egui::Vec2>,
) -> eframe::egui::Pos2 {
    let offset = root_inner.map_or(eframe::egui::Vec2::ZERO, |root| root.min.to_vec2());
    let mut x = row.right() + offset.x;
    let mut y = row.top() + offset.y;
    if let Some(monitor) = monitor {
        if x + size.x > monitor.x {
            x = row.left() + offset.x - size.x;
        }
        if y + size.y > monitor.y {
            y = monitor.y - size.y;
        }
    }
    pos2(x.max(0.0), y.max(0.0))
}

/// The label of the submenu that is open now, if any (at most one at a time).
pub fn open_submenu(ctx: &eframe::egui::Context) -> Option<String> {
    ctx.data(|data| data.get_temp::<String>(Id::new(OPEN_SUBMENU)))
}

/// Sets (or clears) the open submenu.
fn set_open_submenu(ctx: &eframe::egui::Context, label: Option<&str>) {
    ctx.data_mut(|data| {
        if let Some(label) = label {
            data.insert_temp(Id::new(OPEN_SUBMENU), label.to_owned());
        } else {
            data.remove_temp::<String>(Id::new(OPEN_SUBMENU));
        }
    });
}

/// What the main window's input does to the open menus while they are OS pop-up windows
/// (`m12-menu-close-outside`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootClose {
    /// Leave everything open.
    Nothing,
    /// Close the open submenu only.
    Submenu,
    /// Close the submenu and the menu.
    All,
}

/// How long (seconds) the main window must have been unfocused, with no pop-up focused,
/// before the menus close: focus moving from the main window to the pop-up takes a moment.
pub const FOCUS_GRACE: f64 = 0.25;

/// What the main window saw this frame.
#[derive(Clone, Copy, Debug)]
pub struct RootInput<'a> {
    /// A top-level menu is open.
    pub menu_open: bool,
    /// A submenu is open.
    pub submenu_open: bool,
    /// Where a primary press happened this frame in the main window, if one did.
    pub press: Option<eframe::egui::Pos2>,
    /// Escape was pressed this frame in the main window.
    pub escape: bool,
    /// The menu-bar title rectangles (main window coordinates).
    pub titles: &'a [Rect],
    /// Seconds the main window has been unfocused after having had focus; `None` while it
    /// has focus or never had it.
    pub unfocused_for: Option<f64>,
    /// Some pop-up window has focus.
    pub popup_has_focus: bool,
}

/// The rules: nothing open gives `Nothing`. Otherwise, in this order: the main window
/// unfocused for at least [`FOCUS_GRACE`] with no pop-up focused gives `All`; Escape gives
/// `Submenu` when a submenu is open, else `All`; a primary press not on any title gives
/// `All` (on a title: `Nothing`, the title's own click handles it).
pub fn root_close(input: &RootInput<'_>) -> RootClose {
    if !input.menu_open {
        return RootClose::Nothing;
    }
    if !input.popup_has_focus && input.unfocused_for.is_some_and(|secs| secs >= FOCUS_GRACE) {
        return RootClose::All;
    }
    if input.escape {
        return if input.submenu_open {
            RootClose::Submenu
        } else {
            RootClose::All
        };
    }
    if let Some(press) = input.press
        && !input.titles.iter().any(|title| title.contains(press))
    {
        return RootClose::All;
    }
    RootClose::Nothing
}

/// How long (seconds) after a pop-up asked for a menu step the new menu counts as focused
/// even though its pop-up has not yet reported focus.
pub const STEP_FOCUS_WINDOW: f64 = 0.5;

/// When the main window should be repainted so a focus-loss close happens on time: `Some`
/// of the seconds left until [`FOCUS_GRACE`] is reached when a menu is open, the main
/// window is unfocused (`unfocused_for` is `Some` and below the grace), and no pop-up has
/// focus; `None` in every other case (including when the grace has already passed).
pub fn root_close_repaint_after(input: &RootInput<'_>) -> Option<f64> {
    if !input.menu_open || input.popup_has_focus {
        return None;
    }
    match input.unfocused_for {
        Some(secs) if secs < FOCUS_GRACE => Some(FOCUS_GRACE - secs),
        _ => None,
    }
}

/// Applies [`root_close`] to the main window's input. Does nothing unless `mode` is
/// `PopupMode::Window` and a menu is open (inline pop-ups close themselves). Reads the
/// ROOT viewport's input, consumes the Escape it acts on, records unfocused time and the
/// pop-up windows' focus, and returns whether it closed anything. Call it from `menu_bar`.
pub fn close_menus_from_root(ctx: &eframe::egui::Context, mode: PopupMode) -> bool {
    if mode != PopupMode::Window {
        return false;
    }
    let embedded = ctx.embed_viewports();
    let now = ctx.input_for(eframe::egui::ViewportId::ROOT, |input| input.time);
    let first_call = ctx.data_mut(|data| {
        let id = Id::new(ROOT_CLOSE_TIME);
        let first = data.get_temp::<f64>(id) != Some(now);
        if first {
            data.insert_temp(id, now);
        }
        first
    });
    if embedded && !first_call {
        let Some(_) = open_menu(ctx) else {
            return false;
        };
        let escape = ctx.input_for(eframe::egui::ViewportId::ROOT, |input| {
            input.key_pressed(eframe::egui::Key::Escape)
        });
        if escape {
            ctx.input_mut_for(eframe::egui::ViewportId::ROOT, |input| {
                input.consume_key(eframe::egui::Modifiers::NONE, eframe::egui::Key::Escape);
            });
        }
        clear_submenu(ctx);
        set_open_menu(ctx, None);
        return true;
    }
    let Some(menu_label) = open_menu(ctx) else {
        ctx.data_mut(|data| data.remove_temp::<(String, f64, bool)>(Id::new(MENU_STEP_FOCUS)));
        return false;
    };
    let submenu_label = open_submenu(ctx);
    let (now, focused, press, escape) = ctx.input_for(eframe::egui::ViewportId::ROOT, |input| {
        let press = input
            .pointer
            .button_pressed(eframe::egui::PointerButton::Primary)
            .then(|| input.pointer.interact_pos())
            .flatten();
        (
            input.time,
            input.viewport().focused,
            press,
            input.key_pressed(eframe::egui::Key::Escape),
        )
    });

    let unfocused_for = ctx.data_mut(|data| {
        let last_id = Id::new(ROOT_FOCUSED_LAST);
        let since_id = Id::new(ROOT_UNFOCUSED_SINCE);
        match focused {
            Some(true) => {
                data.remove_temp::<f64>(since_id);
                data.insert_temp(last_id, Some(true));
                None
            }
            Some(false) => {
                if data.get_temp::<Option<bool>>(last_id).flatten() == Some(true) {
                    data.insert_temp(since_id, now);
                }
                data.insert_temp(last_id, Some(false));
                data.get_temp::<f64>(since_id).map(|since| now - since)
            }
            None => {
                data.insert_temp(last_id, None::<bool>);
                data.remove_temp::<f64>(since_id);
                None
            }
        }
    });

    let titles: Vec<Rect> = ctx.data(|data| {
        MENU_LABELS
            .iter()
            .filter_map(|label| data.get_temp::<Rect>(Id::new((MENU_TITLE, *label))))
            .collect()
    });
    let popup_titles = [
        Some(menu_popup_title(&menu_label)),
        submenu_label.as_deref().map(submenu_popup_title),
    ];
    let popup_has_focus = ctx.data(|data| {
        popup_titles.into_iter().flatten().any(|title| {
            data.get_temp::<bool>(
                crate::popup::popup_viewport_id(&title)
                    .0
                    .with("popup-has-focus"),
            )
            .unwrap_or(false)
        })
    });
    let step_focus_pending = ctx.data_mut(|data| {
        let id = Id::new(MENU_STEP_FOCUS);
        let Some((label, since, reported_focus)) = data.get_temp::<(String, f64, bool)>(id) else {
            return false;
        };
        if label != menu_label {
            data.remove_temp::<(String, f64, bool)>(id);
            return false;
        }
        let focused = data
            .get_temp::<bool>(
                crate::popup::popup_viewport_id(&menu_popup_title(&label))
                    .0
                    .with("popup-has-focus"),
            )
            .unwrap_or(false);
        if focused && !reported_focus {
            data.insert_temp(id, (label, since, true));
        }
        if now - since >= STEP_FOCUS_WINDOW && (focused || reported_focus) {
            data.remove_temp::<(String, f64, bool)>(id);
            false
        } else {
            // Still waiting for the new pop-up to report focus. Keep the menus up and let
            // `menu_bar_inner` ask for focus again (the pop-up may take a few frames).
            true
        }
    });
    let popup_has_focus = popup_has_focus || step_focus_pending;

    let input = RootInput {
        menu_open: true,
        submenu_open: submenu_label.is_some(),
        press,
        escape,
        titles: &titles,
        unfocused_for,
        popup_has_focus,
    };
    if let Some(secs) = root_close_repaint_after(&input) {
        ctx.request_repaint_after_for(
            Duration::from_secs_f64(secs),
            eframe::egui::ViewportId::ROOT,
        );
    }
    let close = root_close(&input);
    if close == RootClose::Nothing {
        return false;
    }
    if escape {
        ctx.input_mut_for(eframe::egui::ViewportId::ROOT, |input| {
            input.consume_key(eframe::egui::Modifiers::NONE, eframe::egui::Key::Escape);
        });
    }
    match close {
        RootClose::Submenu => clear_submenu(ctx),
        RootClose::All => {
            clear_submenu(ctx);
            set_open_menu(ctx, None);
            ctx.data_mut(|data| data.remove_temp::<(String, f64, bool)>(Id::new(MENU_STEP_FOCUS)));
        }
        RootClose::Nothing => {}
    }
    !embedded
}

/// Asks the menu bar to move the open menu `step` titles along (wrapping), as the arrow
/// keys do; for a pop-up window that has the keyboard focus. The next `menu_bar` call
/// applies it; with no menu open it does nothing.
pub fn request_menu_step(ctx: &eframe::egui::Context, step: i32) {
    ctx.data_mut(|data| data.insert_temp(Id::new(MENU_STEP), step));
}

/// A top-level menu title whose rows open in a pop-up and scroll when taller than the window.
fn top_menu<R>(ui: &mut Ui, label: &str, add_contents: impl FnOnce(&mut Ui) -> R) {
    let ctx = ui.ctx().clone();
    let is_open = open_menu(&ctx).as_deref() == Some(label);
    let response = ui.add(eframe::egui::Button::new(label).selected(is_open));
    ctx.data_mut(|data| data.insert_temp(Id::new((MENU_TITLE, label)), response.rect));

    if response.clicked() {
        let now = ctx.input(|input| input.time);
        let recently_closed = ctx.data(|data| {
            data.get_temp::<(String, f64)>(Id::new(MENU_CLOSED))
                .is_some_and(|(closed, at)| closed == label && now - at < 0.3)
        });
        if is_open {
            set_open_menu(&ctx, None);
            clear_submenu(&ctx);
        } else if !recently_closed {
            set_open_menu(&ctx, Some(label));
            clear_submenu(&ctx);
        }
    }

    let hovered = response.hovered()
        || ctx
            .pointer_hover_pos()
            .is_some_and(|pos| response.rect.contains(pos));
    let pointer_moved = ctx.input(|input| input.pointer.delta() != eframe::egui::Vec2::ZERO);
    let open_before = open_menu(&ctx);
    if open_before.is_some()
        && !response.clicked()
        && hovered
        && pointer_moved
        && open_before.as_deref() != Some(label)
    {
        set_open_menu(&ctx, Some(label));
        clear_submenu(&ctx);
    }

    if open_menu(&ctx).as_deref() != Some(label) {
        return;
    }

    let title = menu_popup_title(label);
    let root_inner = ctx
        .data(|data| data.get_temp::<Option<Rect>>(Id::new(ROOT_INNER)))
        .flatten()
        .or_else(|| ctx.input(|input| input.viewport().inner_rect));
    let position = menu_popup_position(
        popup_root_inner(root_inner, wayland_session()),
        response.rect,
    );
    let mode = popup_mode(ctx.embed_viewports(), popup_supported());
    let bar_bottom = ctx
        .data(|data| data.get_temp::<f32>(Id::new(MENU_BAR_BOTTOM)))
        .unwrap_or_else(|| response.rect.bottom());
    let max_height = popup_max_height(ui, mode, position.y, bar_bottom);
    let popup = PopupWindow {
        title: &title,
        position,
    };
    let mut shown = true;
    let mut closed = false;
    let mut content = Some(add_contents);
    popup_window(&ctx, &popup, &mut shown, |ui| {
        if let Some(content) = content.take() {
            let (_, asked_to_close) = popup_rows(ui, max_height, content);
            closed = asked_to_close;
        }
    });

    if closed || open_menu(&ctx).as_deref() != Some(label) {
        set_open_menu(&ctx, None);
        clear_submenu(&ctx);
    } else if !shown {
        if open_submenu(&ctx).is_some() {
            restore_popup_window_state(&ctx, &title);
        } else {
            set_open_menu(&ctx, None);
            clear_submenu(&ctx);
            let now = ctx.input(|input| input.time);
            ctx.data_mut(|data| {
                data.insert_temp(Id::new(MENU_CLOSED), (label.to_owned(), now));
            });
        }
    }
}

/// A submenu row with a drawn arrow; `add_contents` fills the submenu.
pub fn submenu_button<R>(
    ui: &mut Ui,
    label: &str,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<Option<R>> {
    let ctx = ui.ctx().clone();
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

    let is_open = open_submenu(&ctx).as_deref() == Some(label);
    let visuals = ui.style().interact(&response);
    let painter = ui.painter().clone();
    if response.hovered() || is_open {
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

    let viewport_inner = ctx.input(|input| input.viewport().inner_rect);
    let root_inner = ctx
        .data(|data| data.get_temp::<Option<Rect>>(Id::new(ROOT_INNER)))
        .flatten()
        .or(viewport_inner);
    let row_in_root = match (viewport_inner, root_inner) {
        (Some(viewport), Some(root)) => row.translate(viewport.min - root.min),
        _ => row,
    };

    if response.clicked() {
        if is_open {
            set_open_submenu(&ctx, None);
        } else {
            set_open_submenu(&ctx, Some(label));
            ctx.data_mut(|data| data.insert_temp(Id::new(SUBMENU_ROW), row_in_root));
        }
    }

    let open_label = open_submenu(&ctx);
    if open_label.as_deref() != Some(label) || open_menu(&ctx).is_none() {
        return InnerResponse::new(None, response);
    }

    let row_rect = ctx
        .data(|data| data.get_temp::<Rect>(Id::new(SUBMENU_ROW)))
        .unwrap_or(row_in_root);
    let size = ctx
        .data(|data| data.get_temp::<eframe::egui::Vec2>(Id::new(SUBMENU_SIZE)))
        .unwrap_or_else(|| row.size());
    let monitor = ctx.input(|input| input.viewport().monitor_size);
    let mut position = submenu_popup_position(
        popup_root_inner(root_inner, wayland_session()),
        row_rect,
        size,
        monitor,
    );
    let title = submenu_popup_title(label);
    let mode = popup_mode(ctx.embed_viewports(), popup_supported());
    if mode == PopupMode::Window
        && let (Some(viewport), Some(root)) = (viewport_inner, root_inner)
        && (position.x - (row_rect.right() + root.min.x)).abs() < 0.5
    {
        position.x = position.x.max(viewport.right());
    }
    let bar_bottom = ctx
        .data(|data| data.get_temp::<f32>(Id::new(MENU_BAR_BOTTOM)))
        .unwrap_or_else(|| ui.max_rect().top());
    let max_height = popup_max_height(ui, mode, position.y, bar_bottom);
    let popup = PopupWindow {
        title: &title,
        position,
    };
    let mut shown = true;
    let mut content = Some(add_contents);
    let mut content_size = size;
    let mut closed = false;
    let mut escaped = false;
    let mut result = None;
    ctx.data_mut(|data| data.insert_temp(Id::new(IN_SUBMENU), true));
    popup_window(&ctx, &popup, &mut shown, |ui| {
        escaped = ui.input(|input| input.key_pressed(eframe::egui::Key::Escape));
        let start = ui.min_rect().min;
        if let Some(content) = content.take() {
            let (inner, asked_to_close) = popup_rows(ui, max_height, content);
            result = Some(inner);
            closed = asked_to_close;
        }
        content_size = ui.min_rect().max - start;
    });
    ctx.data_mut(|data| data.remove_temp::<bool>(Id::new(IN_SUBMENU)));
    ctx.data_mut(|data| data.insert_temp(Id::new(SUBMENU_SIZE), content_size));

    if escaped || closed {
        clear_submenu(&ctx);
        set_open_menu(&ctx, None);
    } else if !shown && open_submenu(&ctx).as_deref() == Some(label) {
        clear_submenu(&ctx);
    }

    InnerResponse::new(result, response)
}

/// The View > Toolbars submenu, keeping its parent menu's outcome up to date.
fn toolbars_submenu(
    ui: &mut Ui,
    layout: &mut ToolbarLayout,
    toolbars_changed: &mut bool,
    customize_toolbars: &mut bool,
    reset_toolbars: &mut bool,
) {
    submenu_button(ui, "Toolbars", |ui| {
        match toolbars::toolbars_menu(ui, layout) {
            Some(ToolbarsMenuEvent::Changed) => *toolbars_changed = true,
            Some(ToolbarsMenuEvent::Customize) => {
                *customize_toolbars = true;
                ui.close();
            }
            Some(ToolbarsMenuEvent::Reset) => {
                *reset_toolbars = true;
                ui.close();
            }
            None => {}
        }
    });
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
    let root_inner = ui.ctx().input(|input| input.viewport().inner_rect);
    ui.ctx()
        .data_mut(|data| data.insert_temp(Id::new(ROOT_INNER), root_inner));

    let mode = popup_mode(ui.ctx().embed_viewports(), popup_supported());
    close_menus_from_root(ui.ctx(), mode);
    let requested_step = ui
        .ctx()
        .data_mut(|data| data.remove_temp::<i32>(Id::new(MENU_STEP)));

    if open_submenu(ui.ctx()).is_none()
        && let Some(label) = open_menu(ui.ctx())
        && let Some(index) = MENU_LABELS.iter().position(|known| *known == label)
    {
        let step = if let Some(step) = requested_step {
            Some(step as isize)
        } else if ui.ctx().input_mut(|input| {
            input.consume_key(eframe::egui::Modifiers::NONE, eframe::egui::Key::ArrowRight)
        }) {
            Some(1)
        } else if ui.ctx().input_mut(|input| {
            input.consume_key(eframe::egui::Modifiers::NONE, eframe::egui::Key::ArrowLeft)
        }) {
            Some(-1)
        } else {
            None
        };
        if let Some(step) = step {
            let next = (index as isize + step).rem_euclid(MENU_LABELS.len() as isize) as usize;
            let label = MENU_LABELS[next];
            set_open_menu(ui.ctx(), Some(label));
            if requested_step.is_some() {
                let now = ui.ctx().input(|input| input.time);
                ui.ctx().data_mut(|data| {
                    data.insert_temp(Id::new(MENU_STEP_FOCUS), (label.to_owned(), now, false));
                });
            }
        }
    }
    if let Some((label, _, _)) = ui
        .ctx()
        .data(|data| data.get_temp::<(String, f64, bool)>(Id::new(MENU_STEP_FOCUS)))
        && open_menu(ui.ctx()).as_deref() == Some(label.as_str())
    {
        ui.ctx().send_viewport_cmd_to(
            crate::popup::popup_viewport_id(&menu_popup_title(&label)),
            eframe::egui::ViewportCommand::Focus,
        );
    }

    eframe::egui::MenuBar::new().ui(ui, |ui| {
        ui.ctx()
            .data_mut(|data| data.insert_temp(Id::new(MENU_BAR_BOTTOM), ui.min_rect().bottom()));
        top_menu(ui, "File", |ui| {
            if menu_item(ui, None, "New", "Ctrl+N", false).clicked() {
                chosen = Some(Action::New);
                ui.close();
            }
            if menu_item(ui, None, "Open...", "Ctrl+O", false).clicked() {
                chosen = Some(Action::Open);
                ui.close();
            }
            if menu_item(ui, None, "Import...", "", false).clicked() {
                chosen = Some(Action::Import);
                ui.close();
            }
            ui.separator();
            for format in [ExportFormat::ThreeMf, ExportFormat::Stl] {
                let label = format!("Export {}...", format.label());
                let item = ui
                    .add_enabled_ui(state.can_export, |ui| {
                        menu_item(ui, None, &label, "", false)
                    })
                    .inner;
                if item.clicked() {
                    chosen = Some(Action::ExportMesh(format));
                    ui.close();
                }
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
        top_menu(ui, "Edit", |ui| {
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
        top_menu(ui, "View", |ui| {
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
                (state.tags, "Tags", Action::TogglePanel(PanelId::Tags)),
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
                toolbars_submenu(
                    ui,
                    layout,
                    &mut toolbars_changed,
                    &mut customize_toolbars,
                    &mut reset_toolbars,
                );
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
        top_menu(ui, "Tools", |ui| {
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
            let (grow, shrink) = ui
                .add_enabled_ui(state.can_offset, |ui| {
                    (
                        menu_item(ui, None, "Grow by Clearance", "", false),
                        menu_item(ui, None, "Shrink by Clearance", "", false),
                    )
                })
                .inner;
            if grow.clicked() {
                chosen = Some(Action::GrowByClearance);
                ui.close();
            }
            if shrink.clicked() {
                chosen = Some(Action::ShrinkByClearance);
                ui.close();
            }
            let text = ui
                .add_enabled_ui(state.can_text, |ui| {
                    menu_item(ui, None, "3D Text...", "", false)
                })
                .inner;
            if text.clicked() {
                chosen = Some(Action::Text3d);
                ui.close();
            }
            if menu_item(ui, None, "Print Setup...", "", false).clicked() {
                chosen = Some(Action::PrintSetup);
                ui.close();
            }
        });
        top_menu(ui, "Help", |ui| {
            if menu_item(ui, None, "User Guide", "F1", false).clicked() {
                chosen = Some(Action::UserGuide);
                ui.close();
            }
            if menu_item(ui, None, "About Drydock...", "", false).clicked() {
                chosen = Some(Action::About);
                ui.close();
            }
        });
    });

    if open_menu(ui.ctx()).is_none() {
        clear_submenu(ui.ctx());
    }

    MenuOutcome {
        action: chosen,
        toolbars_changed,
        customize_toolbars,
        reset_toolbars,
    }
}
