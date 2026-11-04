// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Follow Me tool's state and steps: pick a profile face, pick the path (edges, or a face
//! whose outline is the path), preview the swept sections, apply as one undo step.
//! `Editor` owns one of these and calls it from its click, key and preview code
//! (`docs/specs/m9-followme-tool.md`); the geometry is `drydock_kernel::sweep` and
//! `drydock_kernel::sweepface`.

use drydock_kernel::model::{BodyId, Model, UndoStack};
use drydock_kernel::select::Entity;
use drydock_kernel::sweep::{self, PathError};
use drydock_kernel::sweepface::{self, SweepError};
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
        if self.profile.is_none() {
            HINT_PROFILE
        } else if self.path.is_empty() {
            HINT_PATH
        } else {
            HINT_APPLY
        }
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
        self.message = None;
        let Some(profile) = self.profile else {
            if let Some(face) = under.face {
                self.profile = Some(face);
            }
            self.recompute_preview(model, body);
            return false;
        };

        match under.entity {
            Some(Entity::Edge(edge)) => {
                if self.on_profile_outline(model, body, profile, edge) {
                    self.message = Some("That edge is part of the profile.".to_string());
                } else {
                    self.path_face = None;
                    if let Some(index) = self.path.iter().position(|&e| e == edge) {
                        self.path.remove(index);
                    } else {
                        self.path.push(edge);
                    }
                }
            }
            Some(Entity::Face(face)) => {
                if face == profile {
                    self.message = Some("That face is the profile.".to_string());
                } else if self.path_face == Some(face) {
                    return self.apply(model, body, undo);
                } else {
                    match Self::perimeter(model, body, face) {
                        Some(path) => {
                            self.path = path.edges;
                            self.path_face = Some(face);
                        }
                        None => {
                            self.message = Some("That face has no outline to follow.".to_string());
                        }
                    }
                }
            }
            Some(Entity::Instance(_)) | None => {}
        }

        self.recompute_preview(model, body);
        false
    }

    /// Enter: sweep the profile along the path as one undo step labelled [`LABEL`]. Returns
    /// true when the model changed. On a refusal the picks stay and `message` says why.
    pub fn apply(&mut self, model: &mut Model, body: BodyId, undo: &mut UndoStack) -> bool {
        self.message = None;
        let Some(profile) = self.profile else {
            return false;
        };
        if self.path.is_empty() {
            return false;
        }

        let path = {
            let Some(body_ref) = model.body(body) else {
                self.message = Some("The body is gone.".to_string());
                return false;
            };
            match sweep::path_from_edges(body_ref.geometry(), &self.path) {
                Ok(path) => path,
                Err(error) => {
                    self.message = Some(path_error_message(&error).to_string());
                    return false;
                }
            }
        };

        match sweepface::sweep_face(model, body, profile, &path, undo, LABEL) {
            Ok(_) => {
                self.reset();
                true
            }
            Err(error) => {
                self.message = Some(sweep_error_message(&error).to_string());
                false
            }
        }
    }

    fn perimeter(model: &Model, body: BodyId, face: FaceId) -> Option<sweep::SweepPath> {
        let body_ref = model.body(body)?;
        sweep::face_perimeter(body_ref.geometry(), face).ok()
    }

    fn on_profile_outline(
        &self,
        model: &Model,
        body: BodyId,
        profile: FaceId,
        edge: EdgeId,
    ) -> bool {
        let Some(body_ref) = model.body(body) else {
            return false;
        };
        let ctx = body_ref.geometry();
        ctx.face(profile)
            .is_some_and(|face| face.outer.uses.iter().any(|use_| use_.edge == edge))
    }

    fn recompute_preview(&mut self, model: &Model, body: BodyId) {
        self.segments.clear();
        let Some(profile) = self.profile else {
            return;
        };
        if self.path.is_empty() {
            return;
        }
        let Some(body_ref) = model.body(body) else {
            return;
        };
        let ctx = body_ref.geometry();
        let Ok(path) = sweep::path_from_edges(ctx, &self.path) else {
            return;
        };
        let Ok(sections) = sweepface::sweep_sections(ctx, profile, &path) else {
            return;
        };
        for section in &sections {
            for points in section {
                for i in 0..points.len() {
                    self.segments
                        .push((points[i], points[(i + 1) % points.len()]));
                }
            }
        }
        for pair in sections.windows(2) {
            for (first, second) in pair[0].iter().zip(&pair[1]) {
                for (&a, &b) in first.iter().zip(second) {
                    self.segments.push((a, b));
                }
            }
        }
    }
}

fn path_error_message(error: &PathError) -> &'static str {
    match error {
        PathError::Empty => "Pick some edges for the path.",
        PathError::UnknownEdge(_) => "One of the path edges is gone.",
        PathError::UnknownFace(_) => "That face is gone.",
        PathError::Branch(_) => "The path branches where three or more edges meet.",
        PathError::Gap { .. } => "The path has a gap: its edges are not all connected.",
    }
}

fn sweep_error_message(error: &SweepError) -> &'static str {
    match error {
        SweepError::MissingBody(_) => "The body is gone.",
        SweepError::MissingFace(_) => "The profile face is gone.",
        SweepError::BadPath => "The path is too short or has a zero-length segment.",
        SweepError::ProfileShared(_) => "The profile touches another face.",
        SweepError::DegenerateMitre(_) => "The path turns too sharply for the profile.",
        SweepError::NotOnPath => "The profile's plane does not meet the path.",
        SweepError::MitreMismatch(_) => "The profile does not lie in the path's mitre plane.",
        SweepError::ProfileParallel => "The profile is parallel to the path.",
        SweepError::Folded(_) => "The profile is too big for a turn in the path.",
        SweepError::Twisted => "The path twists the profile and does not close it.",
        SweepError::Invalid(_) => "The sweep would make the model invalid.",
        SweepError::Overlap(_, _) => "The sweep would overlap another face.",
        SweepError::Model(_) => "The model could not be changed.",
    }
}
