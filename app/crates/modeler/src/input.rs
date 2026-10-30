// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Mouse and keyboard to viewport actions. Pure functions; the window code feeds them
//! egui input and applies the result to the camera.

use glam::DVec2;
use modeler_view::StandardView;

use crate::dock::PanelId;

/// Pixels of vertical drag per zoom step with the Zoom tool.
pub const ZOOM_DRAG_PIXELS_PER_STEP: f64 = 25.0;

/// The active viewport tool (left mouse button).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Tool {
    #[default]
    Orbit,
    Pan,
    Zoom,
    /// Click and box selection (`docs/specs/m2-app-editor.md`).
    Select,
    Line,
    /// Rectangle, circle and arc tools (`docs/specs/m2-app-shapes.md`).
    Rectangle,
    Circle,
    Arc,
    /// Three-point arc (`docs/specs/m2-arc-3pt-app.md`).
    Arc3,
    /// Push/Pull a face (`docs/specs/m3-app-pushpull.md`).
    PushPull,
    /// Rotate the selection (`docs/specs/m3-app-rotate.md`).
    Rotate,
    /// Move the selection (`docs/specs/m3-app-move.md`).
    Move,
    /// Scale the selection from a handle (`docs/specs/m3-app-scale-flip.md`).
    Scale,
    /// Measure distances and leave guides (`docs/specs/m3-app-tape.md`).
    Tape,
    /// Click an edge, face or instance to erase it (`docs/specs/m3-app-erase.md`).
    Erase,
    /// Paint Bucket: paint the clicked side or the selection; Alt samples
    /// (`docs/specs/m4-app-paint.md`).
    Paint,
}

/// A key for the editor (`docs/specs/m2-app-editor.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKey {
    /// Esc: drop the shape in progress and the typed text.
    Cancel,
    /// Send the typed text to the tool.
    Enter,
    /// Delete the last typed character.
    Backspace,
    Undo,
    Redo,
    /// Lock the drawing direction to an axis (0 red, 1 green, 2 blue), or release it.
    Lock(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// What an input does to the view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Orbit {
        dx: f64,
        dy: f64,
    },
    Pan {
        dx: f64,
        dy: f64,
    },
    Zoom {
        steps: f64,
        cursor_ndc: DVec2,
    },
    ZoomExtents,
    View(StandardView),
    ToggleProjection,
    SelectTool(Tool),
    /// Start a new empty document.
    New,
    Open,
    /// File > Import...: add a mesh file's groups to the open model.
    Import,
    /// File > Export 3MF... / Export STL...: show the export dialog (`docs/specs/m7-export-ui.md`).
    ExportMesh(crate::mesh_export::ExportFormat),
    /// Save to the open native file (a DAE document asks for a name instead).
    Save,
    /// Save under a new name.
    SaveAs,
    Edit(EditKey),
    /// Mirror the selection about the red (0), green (1) or blue (2) axis through its
    /// box center (menu only; `docs/specs/m3-ui-keys.md`).
    Flip(usize),
    /// Erase the selection (Delete key, Edit menu; `docs/specs/m3-app-erase.md`).
    Erase,
    /// Copy the selection to the clipboard (Ctrl+C, Edit menu;
    /// `docs/specs/m4-app-copy-paste.md`).
    Copy,
    /// Copy the selection to the clipboard and erase it (Ctrl+X, Edit menu).
    Cut,
    /// Start pasting the clipboard (Ctrl+V, Edit menu).
    Paste,
    /// Show the About window (Help menu).
    About,
    /// Make the selection a component, after asking for its name (Edit menu;
    /// `docs/specs/m4-app-components-ui.md`).
    MakeComponent,
    /// Make the selection a group, after asking for its name (Edit menu).
    MakeGroup,
    /// Explode the selected group or component (Edit menu).
    Explode,
    /// Hide (or show again) the rest of the model while editing inside an instance
    /// (View menu; `docs/specs/m4-app-edit-view.md`).
    ToggleHideRest,
    /// Show or hide the axes at the origin (View menu; `docs/specs/m4-ui-axes.md`).
    ToggleAxes,
    /// Tint overhanging faces or stop (View menu; `docs/specs/m7-overhang.md`).
    ToggleOverhangs,
    /// Check the fit of the two selected parts, or clear the check (Tools menu;
    /// `docs/specs/m7-clearance-ui.md`).
    FitCheck,
    /// Grow the selected groups and components by the clearance (Tools menu;
    /// `docs/specs/m7-clearance-offset-ui.md`).
    GrowByClearance,
    /// Shrink the selected groups and components by the clearance (Tools menu).
    ShrinkByClearance,
    /// Ask for text, a size, a height and a mode, then place it on a face (Tools menu;
    /// `docs/specs/m7-text-3d-ui.md`).
    Text3d,
    /// Show the Print Setup window (Tools menu; `docs/specs/m7-print-setup.md`).
    PrintSetup,
    /// Show or hide the Shadows window (View menu; `docs/specs/m5-app-shadows-ui.md`).
    ToggleShadows,
    /// Show or hide one of the docking panels (View menu; `docs/specs/m6-dock-ui.md`).
    TogglePanel(PanelId),
    /// Put every panel back in its default place (View menu; `docs/specs/m6-dock-ui.md`).
    ResetPanels,
    /// Show the Model Info window (View menu; `docs/specs/m4-ui-status.md`).
    ModelInfo,
    /// Open the recent file at this index of the File menu's list (most recent first;
    /// `docs/specs/m4-ui-menus.md`).
    OpenRecent(usize),
    /// Quit, asking first when the document is modified (File > Exit).
    Exit,
}

/// A mouse drag of `delta` pixels (screen space, y down) with `button` held.
pub fn drag(
    tool: Tool,
    button: Button,
    mods: Modifiers,
    delta: DVec2,
    start_ndc: DVec2,
) -> Option<Action> {
    if delta == DVec2::ZERO {
        return None;
    }
    match button {
        Button::Middle => Some(if mods.shift {
            Action::Pan {
                dx: delta.x,
                dy: delta.y,
            }
        } else {
            Action::Orbit {
                dx: delta.x,
                dy: delta.y,
            }
        }),
        Button::Left => match tool {
            Tool::Orbit => Some(if mods.shift {
                Action::Pan {
                    dx: delta.x,
                    dy: delta.y,
                }
            } else {
                Action::Orbit {
                    dx: delta.x,
                    dy: delta.y,
                }
            }),
            Tool::Pan => Some(Action::Pan {
                dx: delta.x,
                dy: delta.y,
            }),
            Tool::Zoom => Some(Action::Zoom {
                steps: -delta.y / ZOOM_DRAG_PIXELS_PER_STEP,
                cursor_ndc: start_ndc,
            }),
            // The editor handles left drags for these (`docs/specs/m2-app-editor.md`).
            Tool::Select
            | Tool::Line
            | Tool::Rectangle
            | Tool::Circle
            | Tool::Arc
            | Tool::Arc3
            | Tool::PushPull
            | Tool::Rotate
            | Tool::Move
            | Tool::Scale
            | Tool::Tape
            | Tool::Erase
            | Tool::Paint => None,
        },
        Button::Right => None,
    }
}

/// The tool that gets the viewport's left button: none while the Materials eyedropper is
/// armed, else `tool`. The armed eyedropper is a one-shot tool of its own: its click goes
/// to the editor and a left drag does nothing, whatever tool was active.
pub fn left_button_tool(tool: Tool, eyedropper_armed: bool) -> Option<Tool> {
    (!eyedropper_armed).then_some(tool)
}

/// A scroll of `lines` notches (positive = away from the user) with the cursor at
/// `cursor_ndc`.
pub fn scroll(lines: f64, cursor_ndc: DVec2) -> Action {
    Action::Zoom {
        steps: lines,
        cursor_ndc,
    }
}

/// A key press. `key` is the egui key name (e.g. "O", "Z", "F2").
pub fn key(key: &str, mods: Modifiers) -> Option<Action> {
    let plain = !mods.shift && !mods.ctrl && !mods.alt;
    let shift = mods.shift && !mods.ctrl && !mods.alt;
    let ctrl = !mods.shift && mods.ctrl && !mods.alt;
    let ctrl_shift = mods.shift && mods.ctrl && !mods.alt;
    match key {
        "O" if plain => Some(Action::SelectTool(Tool::Orbit)),
        "H" if plain => Some(Action::SelectTool(Tool::Pan)),
        "Z" if plain => Some(Action::SelectTool(Tool::Zoom)),
        "Z" if shift => Some(Action::ZoomExtents),
        "P" if shift => Some(Action::ToggleProjection),
        "P" if plain => Some(Action::SelectTool(Tool::PushPull)),
        "M" if plain => Some(Action::SelectTool(Tool::Move)),
        "Q" if plain => Some(Action::SelectTool(Tool::Rotate)),
        "S" if plain => Some(Action::SelectTool(Tool::Scale)),
        "T" if plain => Some(Action::SelectTool(Tool::Tape)),
        "E" if plain => Some(Action::SelectTool(Tool::Erase)),
        "B" if plain => Some(Action::SelectTool(Tool::Paint)),
        "Delete" if plain => Some(Action::Erase),
        "N" if ctrl => Some(Action::New),
        "O" if ctrl => Some(Action::Open),
        "S" if ctrl => Some(Action::Save),
        "S" if ctrl_shift => Some(Action::SaveAs),
        "Space" if plain => Some(Action::SelectTool(Tool::Select)),
        "L" if plain => Some(Action::SelectTool(Tool::Line)),
        "R" if plain => Some(Action::SelectTool(Tool::Rectangle)),
        "C" if plain => Some(Action::SelectTool(Tool::Circle)),
        "A" if plain => Some(Action::SelectTool(Tool::Arc)),
        "A" if shift => Some(Action::SelectTool(Tool::Arc3)),
        "Escape" if plain => Some(Action::Edit(EditKey::Cancel)),
        "Enter" if plain => Some(Action::Edit(EditKey::Enter)),
        "Backspace" if plain => Some(Action::Edit(EditKey::Backspace)),
        "Z" if ctrl => Some(Action::Edit(EditKey::Undo)),
        "Y" if ctrl => Some(Action::Edit(EditKey::Redo)),
        "Z" if ctrl_shift => Some(Action::Edit(EditKey::Redo)),
        "Right" if plain => Some(Action::Edit(EditKey::Lock(0))),
        "Left" if plain => Some(Action::Edit(EditKey::Lock(1))),
        "Up" if plain => Some(Action::Edit(EditKey::Lock(2))),
        "Down" if plain => Some(Action::Edit(EditKey::Lock(2))),
        "F2" if plain => Some(Action::View(StandardView::Iso)),
        "F3" if plain => Some(Action::View(StandardView::Top)),
        "F4" if plain => Some(Action::View(StandardView::Front)),
        "F5" if plain => Some(Action::View(StandardView::Right)),
        "F6" if plain => Some(Action::View(StandardView::Back)),
        "F7" if plain => Some(Action::View(StandardView::Left)),
        "F8" if plain => Some(Action::View(StandardView::Bottom)),
        _ => None,
    }
}
