// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Cut, Copy and Paste in the editor (`docs/specs/m4-app-copy-paste.md`).

use glam::{DVec2, DVec3};
use modeler::editor::*;
use modeler::input::{EditKey, Modifiers, Tool};
use modeler_kernel::model::{BodyId, Model, UndoStack};
use modeler_kernel::select::Entity;
use modeler_kernel::tools::{Target, draw_segments};
use modeler_kernel::units::Unit;
use modeler_view::{Bounds, Camera, Projection, StandardView};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn view() -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(StandardView::Top);
    camera.projection = Projection::Parallel;
    camera.zoom_extents(
        &Bounds {
            min: DVec3::splat(-8.0),
            max: DVec3::splat(8.0),
        },
        800.0 / 600.0,
    );
    Viewport {
        camera,
        size_px: (800.0, 600.0),
    }
}

/// The unit square at x 1..2, y 0..1 on the ground.
const SQUARE: [DVec3; 4] = [
    DVec3::new(1.0, 0.0, 0.0),
    DVec3::new(2.0, 0.0, 0.0),
    DVec3::new(2.0, 1.0, 0.0),
    DVec3::new(1.0, 1.0, 0.0),
];

struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
}

impl World {
    /// The square drawn into the model and selected, the Select tool active.
    fn new() -> World {
        let mut w = World {
            model: Model::new(),
            editor: Editor::new(),
            view: view(),
        };
        w.editor.unit = Unit::Meter;
        let n = SQUARE.len();
        let segs: Vec<_> = (0..n).map(|i| (SQUARE[i], SQUARE[(i + 1) % n])).collect();
        let mut undo = UndoStack::new();
        let mut t = Target {
            model: &mut w.model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: Unit::Meter,
        };
        draw_segments(&mut t, &segs, "Draw").expect("draw");
        w.editor.set_tool(Tool::Select);
        w.click(v(1.5, 0.5, 0.0));
        assert_eq!(w.editor.selection().len(), 1);
        w
    }

    fn px(&self, p: DVec3) -> DVec2 {
        self.view.project(p).expect("visible")
    }

    fn move_to(&mut self, p: DVec3) {
        let c = self.px(p);
        self.editor
            .pointer_move(&mut self.model, &self.view, c, NONE);
    }

    fn click(&mut self, p: DVec3) -> bool {
        self.move_to(p);
        let c = self.px(p);
        self.editor.click(&mut self.model, &self.view, c, NONE, 1)
    }

    fn positions(&self) -> Vec<DVec3> {
        self.model
            .root()
            .geometry()
            .vertices()
            .map(|(_, p)| p.position)
            .collect()
    }

    fn faces(&self) -> usize {
        self.model.root().geometry().face_count()
    }

    fn undo_count(&self) -> usize {
        self.editor.undo_stack().undo_count()
    }

    fn has(&self, p: DVec3) -> bool {
        self.positions().iter().any(|q| q.distance(p) < 1e-6)
    }

    /// The square's four corners moved by `d` are all in the model.
    fn has_square_at(&self, d: DVec3) -> bool {
        SQUARE.iter().all(|p| self.has(*p + d))
    }
}

#[test]
fn nothing_on_the_clipboard_at_start() {
    let w = World::new();
    assert!(w.editor.clipboard().is_none());
    assert!(!w.editor.pasting());
    assert!(w.editor.paste_preview().is_empty());
}

#[test]
fn copy_fills_the_clipboard_and_changes_nothing() {
    let mut w = World::new();
    let base = w.undo_count();
    let selection = w.editor.selection().to_vec();
    assert!(w.editor.copy(&w.model));
    let clip = w.editor.clipboard().expect("clipboard");
    assert!(!clip.is_empty());
    assert_eq!(w.undo_count(), base);
    assert_eq!(w.faces(), 1);
    assert_eq!(w.editor.selection(), selection.as_slice());
    assert_eq!(w.editor.message(), None);
}

#[test]
fn copy_without_a_selection_keeps_the_old_clipboard() {
    let mut w = World::new();
    assert!(w.editor.copy(&w.model));
    let before = w.editor.clipboard().cloned();
    w.editor.set_tool(Tool::Select);
    w.click(v(5.0, 5.0, 0.0));
    assert!(w.editor.selection().is_empty());
    assert!(!w.editor.copy(&w.model));
    assert_eq!(w.editor.message(), Some("Select something to copy"));
    assert_eq!(w.editor.clipboard().cloned(), before);
}

#[test]
fn copy_without_a_selection_and_clipboard_leaves_it_empty() {
    let mut w = World::new();
    w.click(v(5.0, 5.0, 0.0));
    assert!(w.editor.selection().is_empty());
    assert!(!w.editor.copy(&w.model));
    assert!(w.editor.clipboard().is_none());
    assert_eq!(w.editor.message(), Some("Select something to copy"));
}

#[test]
fn copy_drops_a_tool_in_progress() {
    let mut w = World::new();
    w.editor.set_tool(Tool::Line);
    w.click(v(-3.0, -3.0, 0.0));
    assert!(!w.editor.preview().is_empty() || w.editor.hint().is_some());
    assert!(w.editor.copy(&w.model));
    w.move_to(v(-5.0, -3.0, 0.0));
    assert!(w.editor.preview().is_empty());
}

#[test]
fn cut_erases_with_one_undo_step_labelled_cut() {
    let mut w = World::new();
    let base = w.undo_count();
    assert!(w.editor.cut(&mut w.model));
    // Like Erase on a face: the face goes, its edges stay.
    assert_eq!(w.faces(), 0);
    assert!(w.editor.selection().is_empty());
    assert!(w.editor.clipboard().is_some());
    assert_eq!(w.undo_count(), base + 1);
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Cut"));
    assert_eq!(w.editor.message(), None);
}

#[test]
fn undoing_a_cut_brings_the_square_back_and_keeps_the_clipboard() {
    let mut w = World::new();
    assert!(w.editor.cut(&mut w.model));
    assert!(w.editor.key(EditKey::Undo, &mut w.model));
    assert_eq!(w.faces(), 1);
    assert!(w.has_square_at(DVec3::ZERO));
    assert!(w.editor.clipboard().is_some());
}

#[test]
fn cut_without_a_selection_changes_nothing() {
    let mut w = World::new();
    w.click(v(5.0, 5.0, 0.0));
    let base = w.undo_count();
    assert!(!w.editor.cut(&mut w.model));
    assert_eq!(w.undo_count(), base);
    assert_eq!(w.faces(), 1);
    assert!(w.editor.clipboard().is_none());
    assert_eq!(w.editor.message(), Some("Select something to cut"));
}

#[test]
fn paste_with_nothing_copied_is_refused() {
    let mut w = World::new();
    assert!(!w.editor.paste(&w.model));
    assert!(!w.editor.pasting());
    assert_eq!(w.editor.message(), Some("Nothing to paste"));
}

#[test]
fn paste_starts_with_the_select_tool_and_no_change() {
    let mut w = World::new();
    assert!(w.editor.copy(&w.model));
    w.editor.set_tool(Tool::Move);
    let base = w.undo_count();
    assert!(w.editor.paste(&w.model));
    assert!(w.editor.pasting());
    assert_eq!(w.editor.tool(), Tool::Select);
    assert_eq!(w.undo_count(), base);
    assert_eq!(w.faces(), 1);
    assert!(w.editor.paste_preview().is_empty());
}

#[test]
fn preview_follows_the_cursor_anchored_at_the_bottom_middle() {
    let mut w = World::new();
    assert!(w.editor.copy(&w.model));
    assert!(w.editor.paste(&w.model));
    w.move_to(v(5.5, 4.5, 0.0));
    let preview = w.editor.paste_preview();
    assert_eq!(preview.len(), 4);
    // The square's middle (1.5, 0.5) lands on the cursor: a shift of (4, 4).
    let d = v(4.0, 4.0, 0.0);
    for p in SQUARE {
        assert!(
            preview
                .iter()
                .any(|(a, b)| a.distance(p + d) < 1e-6 || b.distance(p + d) < 1e-6),
            "corner {p:?} missing in the preview"
        );
    }
    w.move_to(v(-4.5, -3.5, 0.0));
    let preview = w.editor.paste_preview();
    let d = v(-6.0, -4.0, 0.0);
    assert!(
        preview
            .iter()
            .any(|(a, b)| a.distance(SQUARE[0] + d) < 1e-6 || b.distance(SQUARE[0] + d) < 1e-6)
    );
}

#[test]
fn click_places_the_paste() {
    let mut w = World::new();
    assert!(w.editor.copy(&w.model));
    assert!(w.editor.paste(&w.model));
    let base = w.undo_count();
    assert!(w.click(v(5.5, 4.5, 0.0)));
    assert_eq!(w.faces(), 2);
    assert!(w.has_square_at(DVec3::ZERO), "the original moved");
    assert!(w.has_square_at(v(4.0, 4.0, 0.0)));
    assert_eq!(w.undo_count(), base + 1);
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Paste"));
    assert_eq!(w.editor.message(), None);
    assert!(!w.editor.pasting());
    assert!(w.editor.paste_preview().is_empty());
    assert_eq!(w.editor.tool(), Tool::Select);
}

#[test]
fn selection_becomes_the_pasted_entities() {
    let mut w = World::new();
    let original = w.editor.selection().to_vec();
    assert!(w.editor.copy(&w.model));
    assert!(w.editor.paste(&w.model));
    assert!(w.click(v(5.5, 4.5, 0.0)));
    // The pasted face and its four edges, none of them the original's.
    let now = w.editor.selection().to_vec();
    assert_eq!(now.len(), 5);
    assert_eq!(
        now.iter().filter(|e| matches!(e, Entity::Face(_))).count(),
        1
    );
    assert!(now.iter().all(|e| !original.contains(e)));
}

#[test]
fn paste_is_undone_in_one_step() {
    let mut w = World::new();
    assert!(w.editor.copy(&w.model));
    assert!(w.editor.paste(&w.model));
    assert!(w.click(v(5.5, 4.5, 0.0)));
    assert!(w.editor.key(EditKey::Undo, &mut w.model));
    assert_eq!(w.faces(), 1);
    assert_eq!(w.positions().len(), 4);
    assert!(w.has_square_at(DVec3::ZERO));
}

#[test]
fn the_same_clipboard_pastes_again() {
    let mut w = World::new();
    assert!(w.editor.copy(&w.model));
    assert!(w.editor.paste(&w.model));
    assert!(w.click(v(5.5, 4.5, 0.0)));
    assert!(w.editor.clipboard().is_some());
    assert!(w.editor.paste(&w.model));
    assert!(w.click(v(-4.5, -4.5, 0.0)));
    assert_eq!(w.faces(), 3);
    assert!(w.has_square_at(v(4.0, 4.0, 0.0)));
    assert!(w.has_square_at(v(-6.0, -5.0, 0.0)));
}

#[test]
fn cut_then_paste_moves_the_square() {
    let mut w = World::new();
    assert!(w.editor.cut(&mut w.model));
    assert!(w.editor.paste(&w.model));
    assert!(w.click(v(5.5, 4.5, 0.0)));
    assert_eq!(w.faces(), 1);
    assert!(w.has_square_at(v(4.0, 4.0, 0.0)));
}

#[test]
fn a_click_with_other_modifiers_or_counts_still_places() {
    let mut w = World::new();
    assert!(w.editor.copy(&w.model));
    assert!(w.editor.paste(&w.model));
    w.move_to(v(5.5, 4.5, 0.0));
    let c = w.px(v(5.5, 4.5, 0.0));
    let shift = Modifiers {
        shift: true,
        ..NONE
    };
    assert!(w.editor.click(&mut w.model, &w.view, c, shift, 2));
    assert_eq!(w.faces(), 2);
    assert!(!w.editor.pasting());
}

#[test]
fn escape_cancels_the_paste() {
    let mut w = World::new();
    assert!(w.editor.copy(&w.model));
    assert!(w.editor.paste(&w.model));
    w.move_to(v(5.5, 4.5, 0.0));
    let base = w.undo_count();
    w.editor.key(EditKey::Cancel, &mut w.model);
    assert!(!w.editor.pasting());
    assert!(w.editor.paste_preview().is_empty());
    assert_eq!(w.undo_count(), base);
    assert_eq!(w.faces(), 1);
    assert!(w.editor.clipboard().is_some());
    // A click is now just a selection click.
    assert!(!w.click(v(5.5, 4.5, 0.0)));
    assert_eq!(w.faces(), 1);
}

#[test]
fn switching_tools_ends_the_paste() {
    let mut w = World::new();
    assert!(w.editor.copy(&w.model));
    assert!(w.editor.paste(&w.model));
    w.editor.set_tool(Tool::Move);
    assert!(!w.editor.pasting());
    assert!(w.editor.paste_preview().is_empty());
    w.editor.set_tool(Tool::Select);
    assert!(!w.click(v(5.5, 4.5, 0.0)));
    assert_eq!(w.faces(), 1);
}

#[test]
fn erasing_ends_the_paste() {
    let mut w = World::new();
    assert!(w.editor.copy(&w.model));
    assert!(w.editor.paste(&w.model));
    assert!(w.editor.erase_selection(&mut w.model));
    assert!(!w.editor.pasting());
}

#[test]
fn a_restarted_paste_has_no_preview_until_the_cursor_moves() {
    let mut w = World::new();
    assert!(w.editor.copy(&w.model));
    assert!(w.editor.paste(&w.model));
    w.move_to(v(5.5, 4.5, 0.0));
    assert!(!w.editor.paste_preview().is_empty());
    w.editor.key(EditKey::Cancel, &mut w.model);
    assert!(w.editor.paste(&w.model));
    assert!(w.editor.paste_preview().is_empty());
}

#[test]
fn pasting_a_selection_of_edges_keeps_them_edges() {
    let mut w = World::new();
    // Select one edge only: click on the bottom edge's middle.
    w.click(v(5.0, 5.0, 0.0));
    w.click(v(1.5, 0.0, 0.0));
    assert_eq!(w.editor.selection().len(), 1);
    assert!(matches!(w.editor.selection()[0], Entity::Edge(_)));
    assert!(w.editor.copy(&w.model));
    assert!(w.editor.paste(&w.model));
    w.move_to(v(-4.0, -4.0, 0.0));
    let preview = w.editor.paste_preview();
    assert_eq!(preview.len(), 1);
    assert!(w.click(v(-4.0, -4.0, 0.0)));
    assert_eq!(w.faces(), 1);
    // Anchor: middle of the edge (1.5, 0), z lowest: so the edge now spans x -4.5..-3.5 at y -4.
    assert!(w.has(v(-4.5, -4.0, 0.0)));
    assert!(w.has(v(-3.5, -4.0, 0.0)));
}
