// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The state of a new, empty document: what the app starts in and what File > New gives
//! (`docs/specs/m10-start-state.md`).

use crate::document::Document;
use crate::editor::Editor;
use crate::print_setup::PrintSettings;
use crate::viewstate::ViewState;

/// A new empty document with its editor and view, ready to draw in.
pub struct Fresh {
    pub document: Document,
    pub editor: Editor,
    pub view: ViewState,
    /// The window title for `document`.
    pub title: String,
}

impl Fresh {
    /// `Document::new_empty()`, `Editor::new()` and a `ViewState::new()` whose bed, bed
    /// visibility and overhang angle come from `print`.
    pub fn new(print: &PrintSettings) -> Fresh {
        let document = Document::new_empty();
        let title = document.title();
        let mut view = ViewState::new();
        view.bed = print.bed;
        view.bed_visible = print.show_bed;
        view.overhang_degrees = print.overhang_degrees;
        Fresh {
            document,
            editor: Editor::new(),
            view,
            title,
        }
    }
}
