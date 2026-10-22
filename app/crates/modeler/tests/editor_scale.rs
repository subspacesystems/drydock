// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Scale and Flip in the editor (`docs/specs/m3-app-scale-flip.md`).

use glam::{DVec2, DVec3};
use modeler::editor::*;
use modeler::input::{EditKey, Modifiers, Tool};
use modeler_kernel::model::{BodyId, Model, UndoStack};
use modeler_kernel::select::Entity;
use modeler_kernel::tools::scale::{Handle, ScaleStage};
use modeler_kernel::tools::{Target, draw_segments};
use modeler_kernel::topology::{Context, FaceId};
use modeler_kernel::units::Unit;
use modeler_view::{Bounds, Camera, Projection, StandardView};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

const CTRL: Modifiers = Modifiers {
    shift: false,
    ctrl: true,
    alt: false,
};

const SHIFT: Modifiers = Modifiers {
    shift: true,
    ctrl: false,
    alt: false,
};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn view(standard: StandardView) -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(standard);
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

/// A right triangle that is not symmetric either way.
const TRIANGLE: [DVec3; 3] = [
    DVec3::new(4.0, 0.0, 0.0),
    DVec3::new(6.0, 0.0, 0.0),
    DVec3::new(4.0, 1.0, 0.0),
];

fn has_segment(segs: &[(DVec3, DVec3)], a: DVec3, b: DVec3) -> bool {
    segs.iter().any(|&(p, q)| {
        (p.distance(a) < 1e-6 && q.distance(b) < 1e-6)
            || (p.distance(b) < 1e-6 && q.distance(a) < 1e-6)
    })
}

struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
}

impl World {
    /// Top view, `shape` drawn straight into the model, the Scale tool.
    fn with(shape: &[DVec3]) -> World {
        let mut w = World {
            model: Model::new(),
            editor: Editor::new(),
            view: view(StandardView::Top),
        };
        w.editor.unit = Unit::Meter;
        let n = shape.len();
        let segs: Vec<_> = (0..n).map(|i| (shape[i], shape[(i + 1) % n])).collect();
        let mut undo = UndoStack::new();
        let mut t = Target {
            model: &mut w.model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: Unit::Meter,
        };
        draw_segments(&mut t, &segs, "Draw").expect("draw");
        w.editor.set_tool(Tool::Scale);
        w
    }

    fn new() -> World {
        World::with(&SQUARE)
    }

    fn ctx(&self) -> &Context {
        self.model.root().geometry()
    }

    fn face(&self) -> FaceId {
        self.ctx().faces().next().map(|(id, _)| id).expect("a face")
    }

    /// Select the face by dragging a window box around everything, then back to Scale.
    fn select_all(&mut self) {
        self.editor.set_tool(Tool::Select);
        let a = self.px(v(-7.0, -7.0, 0.0));
        let b = self.px(v(7.0, 7.0, 0.0));
        self.editor.box_select(&self.model, &self.view, a, b, NONE);
        assert!(!self.editor.selection().is_empty());
        self.editor.set_tool(Tool::Scale);
    }

    fn px(&self, p: DVec3) -> DVec2 {
        self.view.project(p).expect("visible")
    }

    fn move_to_mods(&mut self, p: DVec3, mods: Modifiers) {
        let c = self.px(p);
        self.editor
            .pointer_move(&mut self.model, &self.view, c, mods);
    }

    fn move_to(&mut self, p: DVec3) {
        self.move_to_mods(p, NONE);
    }

    fn click_mods(&mut self, p: DVec3, mods: Modifiers) -> bool {
        self.move_to_mods(p, mods);
        let c = self.px(p);
        self.editor.click(&mut self.model, &self.view, c, mods, 1)
    }

    fn click(&mut self, p: DVec3) -> bool {
        self.click_mods(p, NONE)
    }

    fn key(&mut self, key: EditKey) -> bool {
        self.editor.key(key, &mut self.model)
    }

    fn enter(&mut self, s: &str) -> bool {
        assert!(self.editor.text(s), "text {s:?} refused");
        self.key(EditKey::Enter)
    }

    fn stage(&self) -> ScaleStage {
        self.editor.scale_tool().expect("scale tool").stage()
    }

    fn undo_count(&self) -> usize {
        self.editor.undo_stack().undo_count()
    }

    fn assert_positions(&self, expected: &[DVec3]) {
        let got: Vec<DVec3> = self.ctx().vertices().map(|(_, p)| p.position).collect();
        assert_eq!(got.len(), expected.len(), "{got:?} vs {expected:?}");
        for e in expected {
            assert!(
                got.iter().any(|g| g.distance(*e) < 1e-6),
                "{e:?} missing from {got:?}"
            );
        }
    }

    /// Square selected, its corner (2, 1) grabbed: opposite corner (1, 0) is fixed.
    fn grab_corner(&mut self) {
        self.select_all();
        assert!(!self.click(v(2.0, 1.0, 0.0)));
        assert_eq!(self.stage(), ScaleStage::Drag);
    }
}

#[test]
fn scale_tool_only_with_scale() {
    let mut w = World::new();
    assert!(w.editor.scale_tool().is_some());
    assert_eq!(w.stage(), ScaleStage::Pick);
    assert_eq!(w.editor.draw_plane(), None);
    assert_eq!(w.editor.plane_lock(), None);
    for tool in [Tool::Move, Tool::Select, Tool::Line, Tool::Orbit] {
        w.editor.set_tool(tool);
        assert!(w.editor.scale_tool().is_none(), "{tool:?}");
        assert_eq!(w.editor.scale_hover(), None);
    }
}

#[test]
fn measurement_is_an_empty_scale_at_first() {
    let w = World::new();
    assert_eq!(w.editor.measurement(), Some(("Scale", String::new())));
    assert!(w.editor.preview().is_empty());
    assert!(w.editor.hint().is_none());
}

#[test]
fn hover_finds_the_handle_near_the_cursor() {
    let mut w = World::new();
    w.select_all();
    let near = w.px(v(2.0, 1.0, 0.0)) + DVec2::new(4.0, 3.0);
    w.editor.pointer_move(&mut w.model, &w.view, near, NONE);
    assert_eq!(w.editor.scale_hover(), Some(Handle::new(1, 1, 0)));
    let edge = w.px(v(1.5, 0.0, 0.0));
    w.editor.pointer_move(&mut w.model, &w.view, edge, NONE);
    assert_eq!(w.editor.scale_hover(), Some(Handle::new(0, -1, 0)));
    assert!(w.editor.hint().is_none());
}

#[test]
fn no_hover_away_from_the_handles_or_without_a_selection() {
    let mut w = World::new();
    w.move_to(v(2.0, 1.0, 0.0));
    assert_eq!(w.editor.scale_hover(), None);
    w.select_all();
    w.move_to(v(1.5, 0.5, 0.0));
    assert_eq!(w.editor.scale_hover(), None);
    w.move_to(v(-5.0, 5.0, 0.0));
    assert_eq!(w.editor.scale_hover(), None);
}

#[test]
fn click_without_a_selection_says_so() {
    let mut w = World::new();
    assert!(!w.click(v(2.0, 1.0, 0.0)));
    assert_eq!(w.editor.message(), Some("Select something to scale"));
    assert_eq!(w.stage(), ScaleStage::Pick);
}

#[test]
fn click_away_from_the_handles_says_so() {
    let mut w = World::new();
    w.select_all();
    assert!(!w.click(v(1.5, 0.5, 0.0)));
    assert_eq!(w.editor.message(), Some("Click a handle to scale"));
    assert_eq!(w.stage(), ScaleStage::Pick);
}

#[test]
fn grabbing_a_corner_starts_a_drag() {
    let mut w = World::new();
    w.grab_corner();
    assert_eq!(w.editor.message(), None);
    assert_eq!(w.editor.scale_hover(), None);
    w.move_to(v(2.0, 0.0, 0.0));
    assert_eq!(w.editor.scale_hover(), None);
    assert_eq!(
        w.editor.scale_tool().unwrap().handle_grabbed(),
        Some(Handle::new(1, 1, 0))
    );
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn a_corner_scales_uniformly() {
    let mut w = World::new();
    w.grab_corner();
    w.move_to(v(3.0, 2.0, 0.0));
    assert!(w.editor.hint().is_some());
    assert_eq!(w.editor.measurement(), Some(("Scale", "2.000".to_string())));
    assert!(w.click(v(3.0, 2.0, 0.0)));
    w.assert_positions(&[
        v(1.0, 0.0, 0.0),
        v(3.0, 0.0, 0.0),
        v(3.0, 2.0, 0.0),
        v(1.0, 2.0, 0.0),
    ]);
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.stage(), ScaleStage::Pick);
    assert_eq!(w.editor.message(), None);
    assert!(w.editor.selection().contains(&Entity::Face(w.face())));
    assert_eq!(w.editor.measurement(), Some(("Scale", "2.000".to_string())));
    assert!(w.key(EditKey::Undo));
    w.assert_positions(&SQUARE);
}

#[test]
fn an_edge_handle_scales_along_one_axis() {
    let mut w = World::new();
    w.select_all();
    // The middle of the bottom edge: the top edge (y = 1) stays.
    assert!(!w.click(v(1.5, 0.0, 0.0)));
    assert_eq!(w.stage(), ScaleStage::Drag);
    w.move_to(v(1.5, -1.0, 0.0));
    assert!(w.click(v(1.5, -1.0, 0.0)));
    w.assert_positions(&[
        v(1.0, 1.0, 0.0),
        v(2.0, 1.0, 0.0),
        v(2.0, -1.0, 0.0),
        v(1.0, -1.0, 0.0),
    ]);
    assert_eq!(
        w.editor.measurement(),
        Some(("Scale", "1.000, 2.000, 1.000".to_string()))
    );
}

#[test]
fn ctrl_scales_about_the_center() {
    let mut w = World::new();
    w.grab_corner();
    assert!(w.click_mods(v(2.5, 1.5, 0.0), CTRL));
    w.assert_positions(&[
        v(0.5, -0.5, 0.0),
        v(2.5, -0.5, 0.0),
        v(2.5, 1.5, 0.0),
        v(0.5, 1.5, 0.0),
    ]);
}

#[test]
fn shift_swaps_uniform_and_per_axis() {
    let mut w = World::new();
    w.grab_corner();
    w.move_to_mods(v(3.0, 1.0, 0.0), SHIFT);
    assert!(w.click_mods(v(3.0, 1.0, 0.0), SHIFT));
    w.assert_positions(&[
        v(1.0, 0.0, 0.0),
        v(3.0, 0.0, 0.0),
        v(3.0, 1.0, 0.0),
        v(1.0, 1.0, 0.0),
    ]);
}

#[test]
fn preview_shows_the_box_and_the_scaled_selection() {
    let mut w = World::new();
    w.grab_corner();
    w.move_to(v(3.0, 2.0, 0.0));
    let preview = w.editor.preview();
    assert_eq!(preview.len(), 16);
    // Box edges first, in the order of the spec.
    assert!(has_segment(
        &preview[..12],
        v(1.0, 0.0, 0.0),
        v(3.0, 0.0, 0.0)
    ));
    assert!(has_segment(
        &preview[..12],
        v(3.0, 0.0, 0.0),
        v(3.0, 2.0, 0.0)
    ));
    assert!(preview[0].0.distance(v(1.0, 0.0, 0.0)) < 1e-6);
    assert!(preview[0].1.distance(v(3.0, 0.0, 0.0)) < 1e-6);
    assert!(preview[1].0.distance(v(1.0, 0.0, 0.0)) < 1e-6);
    assert!(preview[1].1.distance(v(1.0, 2.0, 0.0)) < 1e-6);
    // Then the selected edges, scaled.
    assert!(has_segment(
        &preview[12..],
        v(3.0, 0.0, 0.0),
        v(3.0, 2.0, 0.0)
    ));
    assert!(has_segment(
        &preview[12..],
        v(1.0, 2.0, 0.0),
        v(3.0, 2.0, 0.0)
    ));
}

#[test]
fn text_is_taken() {
    let mut w = World::new();
    assert!(!w.editor.text("x"));
    assert!(w.editor.text("2"));
    assert_eq!(w.editor.typed(), "2");
}

#[test]
fn typed_factor_while_scaling() {
    let mut w = World::new();
    w.grab_corner();
    assert!(w.enter("2"));
    w.assert_positions(&[
        v(1.0, 0.0, 0.0),
        v(3.0, 0.0, 0.0),
        v(3.0, 2.0, 0.0),
        v(1.0, 2.0, 0.0),
    ]);
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.stage(), ScaleStage::Pick);
    assert_eq!(w.editor.typed(), "");
    assert_eq!(w.editor.message(), None);
}

#[test]
fn typed_factors_per_axis_on_an_edge_handle() {
    let mut w = World::new();
    w.select_all();
    assert!(!w.click(v(2.0, 0.5, 0.0)));
    assert_eq!(w.stage(), ScaleStage::Drag);
    // The right edge middle only has the x axis: one value.
    assert!(w.enter("3"));
    w.assert_positions(&[
        v(1.0, 0.0, 0.0),
        v(4.0, 0.0, 0.0),
        v(4.0, 1.0, 0.0),
        v(1.0, 1.0, 0.0),
    ]);
}

#[test]
fn a_negative_factor_mirrors() {
    let mut w = World::new();
    w.grab_corner();
    assert!(w.enter("-1"));
    w.assert_positions(&[
        v(1.0, 0.0, 0.0),
        v(0.0, 0.0, 0.0),
        v(0.0, -1.0, 0.0),
        v(1.0, -1.0, 0.0),
    ]);
}

#[test]
fn typed_factor_right_after_changes_the_scale() {
    let mut w = World::new();
    w.grab_corner();
    assert!(w.click(v(3.0, 2.0, 0.0)));
    assert!(w.enter("3"));
    w.assert_positions(&[
        v(1.0, 0.0, 0.0),
        v(4.0, 0.0, 0.0),
        v(4.0, 3.0, 0.0),
        v(1.0, 3.0, 0.0),
    ]);
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.editor.measurement(), Some(("Scale", "3.000".to_string())));
    // The same value again changes nothing.
    assert!(!w.enter("3"));
    assert_eq!(w.undo_count(), 1);
}

#[test]
fn typed_junk_is_refused() {
    let mut w = World::new();
    assert!(!w.enter("2"));
    assert_eq!(w.editor.message(), Some("Click a handle first"));
    w.grab_corner();
    assert!(!w.enter("1,,2"));
    assert_eq!(w.editor.message(), Some("Not a scale factor"));
    assert!(!w.enter("2,3"));
    assert_eq!(w.editor.message(), Some("Wrong number of factors"));
    assert!(!w.enter("0"));
    assert_eq!(w.editor.message(), Some("Can't scale to nothing"));
    assert_eq!(w.stage(), ScaleStage::Drag);
    assert_eq!(w.undo_count(), 0);
    w.assert_positions(&SQUARE);
}

#[test]
fn escape_drops_the_scale() {
    let mut w = World::new();
    w.grab_corner();
    w.move_to(v(3.0, 2.0, 0.0));
    assert!(w.editor.text("2"));
    assert!(!w.key(EditKey::Cancel));
    assert_eq!(w.stage(), ScaleStage::Pick);
    assert_eq!(w.editor.typed(), "");
    assert!(w.editor.preview().is_empty());
    assert_eq!(w.undo_count(), 0);
    w.assert_positions(&SQUARE);
}

#[test]
fn undo_and_redo_drop_a_scale_in_progress() {
    let mut w = World::new();
    w.grab_corner();
    w.key(EditKey::Undo);
    assert_eq!(w.stage(), ScaleStage::Pick);
    w.grab_corner();
    w.key(EditKey::Redo);
    assert_eq!(w.stage(), ScaleStage::Pick);
}

#[test]
fn switching_tools_forgets_the_scale() {
    let mut w = World::new();
    w.grab_corner();
    assert!(w.click(v(3.0, 2.0, 0.0)));
    w.editor.set_tool(Tool::Move);
    w.editor.set_tool(Tool::Scale);
    assert_eq!(w.stage(), ScaleStage::Pick);
    assert_eq!(w.editor.measurement(), Some(("Scale", String::new())));
    assert!(!w.enter("2"));
    assert_eq!(w.editor.message(), Some("Click a handle first"));
}

#[test]
fn flip_mirrors_the_selection_in_place() {
    let mut w = World::with(&TRIANGLE);
    w.select_all();
    assert!(w.editor.flip(0, &mut w.model));
    w.assert_positions(&[v(6.0, 0.0, 0.0), v(4.0, 0.0, 0.0), v(6.0, 1.0, 0.0)]);
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.editor.message(), None);
    assert!(!w.editor.selection().is_empty());
    assert!(w.key(EditKey::Undo));
    w.assert_positions(&TRIANGLE);
}

#[test]
fn flip_about_green() {
    let mut w = World::with(&TRIANGLE);
    w.select_all();
    assert!(w.editor.flip(1, &mut w.model));
    w.assert_positions(&[v(4.0, 1.0, 0.0), v(6.0, 1.0, 0.0), v(4.0, 0.0, 0.0)]);
}

#[test]
fn flip_works_with_other_tools() {
    let mut w = World::with(&TRIANGLE);
    w.select_all();
    w.editor.set_tool(Tool::Select);
    assert!(w.editor.flip(0, &mut w.model));
}

#[test]
fn flip_refusals() {
    let mut w = World::with(&TRIANGLE);
    assert!(!w.editor.flip(0, &mut w.model));
    assert_eq!(w.editor.message(), Some("Select something to flip"));
    w.select_all();
    assert!(!w.editor.flip(3, &mut w.model));
    assert_eq!(w.undo_count(), 0);
    w.assert_positions(&TRIANGLE);
}

#[test]
fn flip_is_ignored_while_scaling() {
    let mut w = World::with(&TRIANGLE);
    w.select_all();
    assert!(!w.click(v(6.0, 1.0, 0.0)));
    assert_eq!(w.stage(), ScaleStage::Drag);
    assert!(!w.editor.flip(0, &mut w.model));
    assert_eq!(w.undo_count(), 0);
    w.assert_positions(&TRIANGLE);
}
