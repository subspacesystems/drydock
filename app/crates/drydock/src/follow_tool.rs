// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Follow Me tool's state and steps: pick a profile face, pick the path (edges, or a face
//! whose outline is the path), preview the swept sections, apply as one undo step.
//! `Editor` owns one of these and calls it from its click, key and preview code
//! (`docs/specs/m9-followme-tool.md`); the geometry is `drydock_kernel::sweep` and
//! `drydock_kernel::sweepface`.

use drydock_kernel::model::{BodyId, Model, UndoStack};
use drydock_kernel::select::Entity;
use drydock_kernel::topology::{EdgeId, FaceId};
use glam::DVec3;

/// Status-bar hint before the profile is picked.
pub const HINT_PROFILE: &str = "Click the face to sweep (the profile).";
/// Status-bar hint once there is a profile and no path edges.
pub const HINT_PATH: &str = "Click edges, or a face to follow its outline, for the path.";
/// Status-bar hint once there is a path.
pub const HINT_APPLY: &str = "Press Enter to sweep along the path. Esc cancels.";

/// Undo-step label.
pub const LABEL: &str = "Follow Me";

/// What is under the cursor, worked out by the editor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Under {
    /// The face the ray meets (what Push/Pull would pick), edges ignored.
    pub face: Option<FaceId>,
    /// What a Select click would pick: an edge close to the cursor first, else a face.
    pub entity: Option<Entity>,
}

/// Follow Me's picks so far and the preview they give.
#[derive(Clone, Debug, Default)]
pub struct FollowTool {
    profile: Option<FaceId>,
    path: Vec<EdgeId>,
    /// The face whose outline is `path`, when the path came from a face click.
    #[allow(dead_code)] // used by the implementation
    path_face: Option<FaceId>,
    segments: Vec<(DVec3, DVec3)>,
    message: Option<String>,
}

impl FollowTool {
    pub fn new() -> FollowTool {
        FollowTool::default()
    }

    /// Forget every pick, the preview and the message.
    pub fn reset(&mut self) {
        *self = FollowTool::default();
    }

    /// The profile face once picked.
    pub fn profile(&self) -> Option<FaceId> {
        self.profile
    }

    /// The path edges, in the order picked (a path face gives its outline in loop order).
    pub fn path(&self) -> &[EdgeId] {
        &self.path
    }

    /// The step the user is on: [`HINT_PROFILE`], [`HINT_PATH`] or [`HINT_APPLY`].
    pub fn hint(&self) -> &'static str {
        unimplemented!("m9-followme-tool")
    }

    /// The preview: the world segments of every section's loops and the lines joining
    /// corresponding points of consecutive sections. Empty without a profile and a path that
    /// sweeps (`sweep_sections` succeeds).
    pub fn preview(&self) -> &[(DVec3, DVec3)] {
        &self.segments
    }

    /// Why the last click or Enter did nothing, for the status bar; cleared by the next one.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// A left click. Returns true when the model changed (a face-path click that applies).
    /// Recomputes the preview.
    pub fn click(
        &mut self,
        model: &mut Model,
        body: BodyId,
        under: Under,
        undo: &mut UndoStack,
    ) -> bool {
        let _ = (model, body, under, undo);
        unimplemented!("m9-followme-tool")
    }

    /// Enter: sweep the profile along the path as one undo step labelled [`LABEL`]. Returns
    /// true when the model changed. On a refusal the picks stay and `message` says why.
    pub fn apply(&mut self, model: &mut Model, body: BodyId, undo: &mut UndoStack) -> bool {
        let _ = (model, body, undo);
        unimplemented!("m9-followme-tool")
    }
}
