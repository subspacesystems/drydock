// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Move tool: click a from point, then a to point (or type a distance) to move the selection
//! by the difference, healing and splitting against the rest of the body.
//!
//! Behaviour: `docs/specs/m3-tool-move.md`.

use glam::{DMat4, DVec3};

use crate::model::UndoStack;
use crate::select::Entity;
use crate::transform::TransformError;
use crate::units::ParseError;

use super::{Input, Target};

/// Why the tool refused an input. The tool, the model and the undo stack are unchanged.
#[derive(Clone, Debug, PartialEq)]
pub enum MoveError {
    /// The typed text is not a length.
    Parse(ParseError),
    /// A typed distance needs a move in progress, or one just finished.
    NotStarted,
    /// A typed distance needs a direction, and the cursor gives none.
    NoDirection,
    /// Nothing is selected.
    Empty,
    /// A clicked point is not finite.
    NotFinite,
    /// The kernel refused the move.
    Transform(TransformError),
}

/// What the next click does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveStage {
    /// Place the point to move from.
    From,
    /// Move: the next click finishes.
    To,
}

#[derive(Clone, Debug)]
struct LastMove {
    #[allow(dead_code)]
    offset: DVec3,
    #[allow(dead_code)]
    count: usize,
}

// Fields are used once the bodies are filled in.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct MoveTool {
    stage: MoveStage,
    from: Option<DVec3>,
    cursor: Option<DVec3>,
    last: Option<LastMove>,
}

impl Default for MoveTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MoveTool {
    pub fn new() -> Self {
        Self {
            stage: MoveStage::From,
            from: None,
            cursor: None,
            last: None,
        }
    }

    pub fn stage(&self) -> MoveStage {
        self.stage
    }

    /// The point to move from, once placed. Use it as `Probe::anchor`.
    pub fn from(&self) -> Option<DVec3> {
        unimplemented!("m3-tool-move")
    }

    /// The offset the next click would move by, while moving and once the cursor has moved.
    pub fn offset(&self) -> Option<DVec3> {
        unimplemented!("m3-tool-move")
    }

    /// The offset of the last finished move, while a typed distance can still change it
    /// (`undo` still shows it on top).
    pub fn last_offset(&self, _undo: &UndoStack) -> Option<DVec3> {
        unimplemented!("m3-tool-move")
    }

    /// The translation the next click would apply, while moving.
    pub fn matrix(&self) -> Option<DMat4> {
        unimplemented!("m3-tool-move")
    }

    /// From point to cursor while moving.
    pub fn preview(&self) -> Vec<(DVec3, DVec3)> {
        unimplemented!("m3-tool-move")
    }

    /// Moves go through `transform_sticky` on `selection`, one undo step each.
    pub fn handle(
        &mut self,
        _input: Input,
        _selection: &[Entity],
        _target: &mut Target,
    ) -> Result<(), MoveError> {
        unimplemented!("m3-tool-move")
    }
}
