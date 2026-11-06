// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Tags panel: one row per tag with an eye toggle (show / hide), the name (double-click
//! renames) and a delete button, and an Add button. Kept apart from `App` so tests can drive
//! it with `egui_kittest`; the panel returns what the user asked for and the app applies it
//! through the editor (one undo step each).
//!
//! Behaviour: `docs/specs/m10-tags-panel.md`.

use drydock_kernel::model::Model;
use drydock_kernel::tags::TagId;
use eframe::egui;

/// What the panel remembers between frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagsPanel {
    /// The tag being renamed and the text typed so far.
    pub editing: Option<(TagId, String)>,
}

/// What the user asked for in the panel this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagAction {
    /// The Add button: the app makes a tag named `default_tag_name`.
    Add,
    /// A rename was confirmed (Enter, or focus left the box) with this text.
    Rename(TagId, String),
    /// The eye was clicked: show (`true`) or hide (`false`) the tag.
    SetVisible(TagId, bool),
    /// The delete button was clicked.
    Delete(TagId),
}

/// The name for a new tag: `Tag N` with the smallest N >= 1 no tag already has (compared
/// exactly).
pub fn default_tag_name(model: &Model) -> String {
    let _ = model;
    unimplemented!("m10-tags-panel")
}

/// The tags in the order the panel lists them: by name ignoring case, then by id.
pub fn listed_tags(model: &Model) -> Vec<TagId> {
    let _ = model;
    unimplemented!("m10-tags-panel")
}

/// Draw the panel. See the spec for the widgets and their accessible labels.
pub fn tags_panel(ui: &mut egui::Ui, panel: &mut TagsPanel, model: &Model) -> Option<TagAction> {
    let _ = (ui, panel, model);
    unimplemented!("m10-tags-panel")
}
