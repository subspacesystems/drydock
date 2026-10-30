// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! 3D text: flat letter loops drawn on a face, then raised out of it (emboss) or sunk into it
//! (engrave) with push/pull, in one undo step.
//!
//! Behaviour: `docs/specs/m7-text-3d.md`.

use std::fmt;

use glam::DVec3;

use crate::model::{BodyId, Model, ModelError, UndoStack};
use crate::pushpull::PushPullError;
use crate::repair::RepairReport;
use crate::solid::SolidReport;
use crate::sticky::StickyError;
use crate::topology::{Context, FaceId};

/// Which way the letters go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextMode {
    /// Raised out of the face, away from the material.
    Emboss,
    /// Sunk into the material behind the face.
    Engrave,
}

/// Why [`text_3d`] or [`check_text`] refused. The model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum Text3dError {
    MissingBody(BodyId),
    /// No such face, or the face has no plane.
    MissingFace(FaceId),
    /// The height is not finite or not longer than `LINEAR_TOL`.
    BadHeight,
    /// No loops.
    NoText,
    /// Loop `i` has fewer than 3 points or is not a simple polygon.
    BadLoop(usize),
    /// A point of loop `i` is off the face's plane.
    OffPlane(usize),
    /// Loops `i < j` cross or touch.
    LoopsCross(usize, usize),
    /// The text is longer than the face however it is turned.
    TooSmall,
    /// Loop `i` leaves the face or meets its border.
    CrossesBorder(usize),
    /// The engrave is at least as deep as the part is thick under the text.
    TooDeep {
        thickness: f64,
    },
    /// The letters would cut into other geometry of the body.
    Intersects,
    /// Drawing the loops on the face failed.
    Draw(StickyError),
    /// Push/pull refused a letter face.
    PushPull(PushPullError),
    /// The body was a solid before and repair couldn't make the result one.
    NotSolid(Box<SolidReport>),
    Model(ModelError),
}

impl fmt::Display for Text3dError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Text3dError::MissingBody(_) => write!(f, "the part is gone"),
            Text3dError::MissingFace(_) => write!(f, "pick a flat face to put the text on"),
            Text3dError::BadHeight => write!(f, "the height must be a length above zero"),
            Text3dError::NoText => write!(f, "there is no text to place"),
            Text3dError::BadLoop(_) => write!(f, "a letter outline crosses itself"),
            Text3dError::OffPlane(_) => write!(f, "the text is not flat on the face"),
            Text3dError::LoopsCross(_, _) => write!(f, "two letters touch or overlap"),
            Text3dError::TooSmall => write!(f, "the face is too small for the text"),
            Text3dError::CrossesBorder(_) => write!(f, "the text runs over the edge of the face"),
            Text3dError::TooDeep { .. } => {
                write!(f, "the engraving is as deep as the part is thick")
            }
            Text3dError::Intersects => write!(f, "the letters would cut into other geometry"),
            Text3dError::Draw(_) => write!(f, "the letters could not be drawn on the face"),
            Text3dError::PushPull(_) => write!(f, "push/pull refused a letter"),
            Text3dError::NotSolid(_) => write!(f, "the part would no longer be a solid"),
            Text3dError::Model(_) => write!(f, "the model could not be changed"),
        }
    }
}

impl std::error::Error for Text3dError {}

/// What [`text_3d`] made.
#[derive(Clone, Debug, PartialEq)]
pub struct TextFaces {
    /// Each letter face at its new place (top of a raised letter, floor of a sunk one), in
    /// loop order.
    pub caps: Vec<FaceId>,
    /// The new side faces, letter after letter, each in push/pull's order.
    pub sides: Vec<FaceId>,
    /// What repair did, when the result needed it.
    pub repaired: Option<RepairReport>,
}

/// Checks that `loops` can be embossed on or engraved into `face` of `ctx` by `height`,
/// without changing anything.
pub fn check_text(
    ctx: &Context,
    face: FaceId,
    loops: &[Vec<DVec3>],
    mode: TextMode,
    height: f64,
) -> Result<(), Text3dError> {
    let _ = (ctx, face, loops, mode, height);
    unimplemented!("m7-text-3d")
}

/// Draws `loops` (closed polygons on `face`'s plane, outer loops and holes told apart by
/// nesting) on `face` of `body` and pushes the letter faces out by `height` (emboss) or in
/// (engrave), as one undo step labelled `label`.
#[allow(clippy::too_many_arguments)]
pub fn text_3d(
    model: &mut Model,
    body: BodyId,
    face: FaceId,
    loops: &[Vec<DVec3>],
    mode: TextMode,
    height: f64,
    undo: &mut UndoStack,
    label: &str,
) -> Result<TextFaces, Text3dError> {
    let _ = (model, body, face, loops, mode, height, undo, label);
    unimplemented!("m7-text-3d")
}
