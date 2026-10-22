// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Drawing tools: state machines that turn the cursor's inferred points, clicks and typed
//! lengths into edits of one body. No UI: the app runs `infer`, feeds the result in as
//! `Input`, and draws `preview`.
//!
//! Behaviour: `docs/specs/m2-tool-line.md`.

use glam::DVec3;

use crate::model::{BodyId, FaceMaterials, Model, ModelEdit, ModelError, UndoStack};
use crate::sticky::{EdgeChain, StickyError};
use crate::units::{ParseError, Unit};

pub mod arc;
pub mod arc3;
pub mod circle;
pub mod line;
pub mod plane;
pub mod rectangle;
pub mod rotate;
pub mod scale;
pub mod tape;
pub mod translate;

/// One event for a tool. Points are in the body's coordinates.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    /// The cursor moved; the inferred point under it.
    Move(DVec3),
    /// A click at the inferred point.
    Click(DVec3),
    /// Text from the measurement box, on Enter.
    Typed(String),
    /// Esc: drop the shape in progress.
    Cancel,
}

/// What a tool edits: one body of `model`. Every finished step goes onto `undo`.
pub struct Target<'a> {
    pub model: &'a mut Model,
    pub body: BodyId,
    pub undo: &'a mut UndoStack,
    /// The unit a bare typed number is read in.
    pub unit: Unit,
}

/// Why a tool refused an input. The tool, the model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolError {
    /// The typed text is not a length.
    Parse(ParseError),
    /// A typed length needs a shape in progress.
    NotStarted,
    /// A typed length needs a direction, and the cursor gives none.
    NoDirection,
    Geometry(StickyError),
    Model(ModelError),
}

/// Draw `segments` into `target.body` as one undo step labelled `label`, and give every face
/// split from an existing face that face's materials. On error, nothing changes.
pub fn draw_segments(
    target: &mut Target,
    segments: &[(DVec3, DVec3)],
    label: &str,
) -> Result<Vec<EdgeChain>, ToolError> {
    if segments.is_empty() {
        return Ok(Vec::new());
    }

    let mut edit = ModelEdit::new();
    let mut chains = Vec::with_capacity(segments.len());
    let mut failure = None;
    let result = target
        .model
        .edit_geometry(target.body, &mut edit, |ctx, log| {
            for &(a, b) in segments {
                match ctx.add_edge(a, b, log) {
                    Ok(chain) => chains.push(chain),
                    Err(error) => {
                        failure = Some(error);
                        break;
                    }
                }
            }
        });
    if let Err(error) = result {
        return Err(ToolError::Model(error));
    }
    if let Some(error) = failure {
        let _ = target.model.apply_edit(&edit.inverse());
        return Err(ToolError::Geometry(error));
    }

    for chain in &chains {
        for &(new, old) in &chain.faces {
            let Some(old) = old else {
                continue;
            };
            let Some(body) = target.model.body(target.body) else {
                let _ = target.model.apply_edit(&edit.inverse());
                return Err(ToolError::Model(ModelError::MissingBody(target.body)));
            };
            let materials = body.face_materials(old);
            if materials == FaceMaterials::default() {
                continue;
            }
            if let Err(error) =
                target
                    .model
                    .set_face_materials(target.body, new, materials, &mut edit)
            {
                let _ = target.model.apply_edit(&edit.inverse());
                return Err(ToolError::Model(error));
            }
        }
    }

    target.undo.push(label, edit);
    Ok(chains)
}
