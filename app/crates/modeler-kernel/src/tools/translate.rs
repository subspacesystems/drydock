// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Move tool: click a from point, then a to point (or type a distance) to move the selection
//! by the difference, healing and splitting against the rest of the body.
//!
//! Behaviour: `docs/specs/m3-tool-move.md`.

use glam::{DMat4, DVec3};

use crate::geom::LINEAR_TOL;
use crate::model::UndoStack;
use crate::movemerge::transform_sticky;
use crate::select::Entity;
use crate::transform::TransformError;
use crate::units::{ParseError, parse_length};

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
    offset: DVec3,
    count: usize,
}

#[derive(Clone, Debug)]
pub struct MoveTool {
    stage: MoveStage,
    from: Option<DVec3>,
    cursor: Option<DVec3>,
    last: Option<LastMove>,
}

fn finite(p: DVec3) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.z.is_finite()
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
        if self.stage == MoveStage::To {
            self.from
        } else {
            None
        }
    }

    /// The offset the next click would move by, while moving and once the cursor has moved.
    pub fn offset(&self) -> Option<DVec3> {
        if self.stage != MoveStage::To {
            return None;
        }
        Some(self.cursor? - self.from?)
    }

    /// The offset of the last finished move, while a typed distance can still change it
    /// (`undo` still shows it on top).
    pub fn last_offset(&self, undo: &UndoStack) -> Option<DVec3> {
        let last = self.last.as_ref()?;
        self.editable(undo).then_some(last.offset)
    }

    /// The translation the next click would apply, while moving.
    pub fn matrix(&self) -> Option<DMat4> {
        self.offset().map(DMat4::from_translation)
    }

    /// From point to cursor while moving.
    pub fn preview(&self) -> Vec<(DVec3, DVec3)> {
        if self.stage != MoveStage::To {
            return Vec::new();
        }
        let (Some(from), Some(cursor)) = (self.from, self.cursor) else {
            return Vec::new();
        };
        if cursor.distance(from) <= LINEAR_TOL {
            Vec::new()
        } else {
            vec![(from, cursor)]
        }
    }

    /// Moves go through `transform_sticky` on `selection`, one undo step each.
    pub fn handle(
        &mut self,
        input: Input,
        selection: &[Entity],
        target: &mut Target,
    ) -> Result<(), MoveError> {
        match input {
            Input::Move(p) => self.move_cursor(p),
            Input::Click(p) => self.click(p, selection, target),
            Input::Typed(text) => self.typed(&text, selection, target),
            Input::Cancel => self.cancel(),
        }
    }

    fn editable(&self, undo: &UndoStack) -> bool {
        let Some(last) = self.last.as_ref() else {
            return false;
        };
        self.stage == MoveStage::From && undo.undo_count() == last.count && undo.redo_count() == 0
    }

    fn move_cursor(&mut self, p: DVec3) -> Result<(), MoveError> {
        if finite(p) && self.stage == MoveStage::To {
            self.cursor = Some(p);
        }
        Ok(())
    }

    fn click(
        &mut self,
        p: DVec3,
        selection: &[Entity],
        target: &mut Target,
    ) -> Result<(), MoveError> {
        if !finite(p) {
            return Err(MoveError::NotFinite);
        }
        match self.stage {
            MoveStage::From => {
                if selection.is_empty() {
                    return Err(MoveError::Empty);
                }
                self.stage = MoveStage::To;
                self.from = Some(p);
                self.cursor = None;
                self.last = None;
                Ok(())
            }
            MoveStage::To => {
                self.cursor = Some(p);
                let Some(offset) = self.offset() else {
                    return Ok(());
                };
                if offset.length() <= LINEAR_TOL {
                    return Ok(());
                }
                self.commit(offset, selection, target)
            }
        }
    }

    fn typed(
        &mut self,
        text: &str,
        selection: &[Entity],
        target: &mut Target,
    ) -> Result<(), MoveError> {
        match self.stage {
            MoveStage::From => self.retype_last(text, selection, target),
            MoveStage::To => {
                let value = parse_length(text, target.unit).map_err(MoveError::Parse)?;
                let Some(offset) = self.offset() else {
                    return Err(MoveError::NoDirection);
                };
                if offset.length() <= LINEAR_TOL {
                    return Err(MoveError::NoDirection);
                }
                if value.abs() <= LINEAR_TOL {
                    return Ok(());
                }
                self.commit(offset.normalize() * value, selection, target)
            }
        }
    }

    fn retype_last(
        &mut self,
        text: &str,
        selection: &[Entity],
        target: &mut Target,
    ) -> Result<(), MoveError> {
        if !self.editable(target.undo) {
            return Err(MoveError::NotStarted);
        }
        let Some(last) = self.last.clone() else {
            return Err(MoveError::NotStarted);
        };
        let value = parse_length(text, target.unit).map_err(MoveError::Parse)?;
        if value.abs() <= LINEAR_TOL {
            return Ok(());
        }
        if selection.is_empty() {
            return Err(MoveError::Empty);
        }
        target
            .undo
            .undo(target.model)
            .map_err(|e| MoveError::Transform(TransformError::Model(e)))?;
        let offset = last.offset.normalize() * value;
        if let Err(error) = transform_sticky(
            target.model,
            target.body,
            selection,
            DMat4::from_translation(offset),
            target.undo,
            "Move",
        ) {
            let _ = target.undo.redo(target.model);
            return Err(MoveError::Transform(error));
        }
        self.last = Some(LastMove {
            offset,
            count: target.undo.undo_count(),
        });
        Ok(())
    }

    fn cancel(&mut self) -> Result<(), MoveError> {
        self.stage = MoveStage::From;
        self.from = None;
        self.cursor = None;
        Ok(())
    }

    fn commit(
        &mut self,
        offset: DVec3,
        selection: &[Entity],
        target: &mut Target,
    ) -> Result<(), MoveError> {
        transform_sticky(
            target.model,
            target.body,
            selection,
            DMat4::from_translation(offset),
            target.undo,
            "Move",
        )
        .map_err(MoveError::Transform)?;
        self.stage = MoveStage::From;
        self.from = None;
        self.cursor = None;
        self.last = Some(LastMove {
            offset,
            count: target.undo.undo_count(),
        });
        Ok(())
    }
}
