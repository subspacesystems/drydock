// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Tools > 3D Text...: the options dialog and the plain functions that turn a piece of text
//! into loops lying on a face. The click, the preview and the undo step are
//! `Editor::begin_text` and friends; the kernel operation is `modeler_kernel::text3d`.
//! Behaviour: `docs/specs/m7-text-3d-ui.md`.

use eframe::egui;
use glam::DVec3;
use modeler_dae::TextOutline;
use modeler_kernel::text3d::TextMode;
use modeler_kernel::topology::{Context, FaceId};

/// Smallest length the size and height fields accept, in millimeters.
pub const MIN_MM: f64 = 0.01;
/// Largest length the size and height fields accept, in millimeters.
pub const MAX_MM: f64 = 1000.0;

/// What to place. Lengths are meters in the world (the fields are millimeters).
#[derive(Clone, Debug, PartialEq)]
pub struct TextOptions {
    /// One line, not empty.
    pub text: String,
    /// Font size (the em square) in meters.
    pub size_m: f64,
    /// How far the letters stand out of the face, or sink into it, in meters.
    pub height_m: f64,
    pub mode: TextMode,
}

/// The length typed in a field, in millimeters: a number with an optional trailing `mm`
/// (spaces around either are fine), finite and within `MIN_MM..=MAX_MM`. `None` otherwise.
pub fn parse_mm(text: &str) -> Option<f64> {
    let _ = text;
    unimplemented!("m7-text-3d-ui")
}

/// How a piece of text lies on a face, in the open body's coordinates. The three are unit
/// vectors at right angles with `right x up = out`: someone looking at the face from outside
/// reads the text left to right along `right`, bottom to top along `up`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextFrame {
    pub right: DVec3,
    pub up: DVec3,
    /// Away from the material (the face's front normal, or its opposite when the face is
    /// reversed: `check_geometry(ctx).reversed_faces`).
    pub out: DVec3,
}

/// The frame for text on `face`; `None` when the face is missing or has no plane.
pub fn text_frame(ctx: &Context, face: FaceId) -> Option<TextFrame> {
    let _ = (ctx, face);
    unimplemented!("m7-text-3d-ui")
}

/// The outline's loops as 3D points on the face's plane: outline point `(x, y)` goes to
/// `center + right * (x - cx) + up * (y - cy)`, where `(cx, cy)` is the middle of the bounding
/// box of all loop points. One entry per `outline.loops`, same order and same points.
pub fn place_loops(outline: &TextOutline, frame: &TextFrame, center: DVec3) -> Vec<Vec<DVec3>> {
    let _ = (outline, frame, center);
    unimplemented!("m7-text-3d-ui")
}

/// The status-bar text for a refused placement: `Can't place text: ` and the error's
/// `Display`, e.g. `Can't place text: the face is too small for the text`.
pub fn refusal(error: &modeler_kernel::text3d::Text3dError) -> String {
    let _ = error;
    unimplemented!("m7-text-3d-ui")
}

/// How the dialog was left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextOutcome {
    Ok,
    Cancel,
}

/// The dialog's state: the text boxes' contents and the mode.
#[derive(Clone, Debug, PartialEq)]
pub struct TextDialog {
    pub text: String,
    pub size_text: String,
    pub height_text: String,
    pub mode: TextMode,
}

impl TextDialog {
    /// First use: empty text, `10`, `1`, `Emboss`.
    pub fn new() -> TextDialog {
        unimplemented!("m7-text-3d-ui")
    }

    /// Shows `options` again (the last ones used): the numbers as millimeters printed the way
    /// Rust prints a float (`0.01` meters is `10`).
    pub fn from_options(options: &TextOptions) -> TextDialog {
        let _ = options;
        unimplemented!("m7-text-3d-ui")
    }

    /// The options the fields hold; `None` while the text is empty or a number does not parse.
    pub fn options(&self) -> Option<TextOptions> {
        unimplemented!("m7-text-3d-ui")
    }
}

impl Default for TextDialog {
    fn default() -> TextDialog {
        TextDialog::new()
    }
}

/// The dialog's contents. `None` until `OK` or `Cancel` is clicked.
pub fn text_dialog(ui: &mut egui::Ui, dialog: &mut TextDialog) -> Option<TextOutcome> {
    let _ = (ui, dialog);
    unimplemented!("m7-text-3d-ui")
}
