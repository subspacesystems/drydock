// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Undo and Redo drop Follow Me's picks and preview (`docs/specs/m9-followme-undo.md`),
//! as they drop Offset and Extrude in progress.

use drydock::editor::*;
use drydock::follow_tool::HINT_PROFILE;
use drydock::input::{EditKey, Modifiers, Tool};
use drydock_kernel::model::{BodyId, Model, UndoStack};
use drydock_kernel::tools::{Target, draw_segments};
use drydock_kernel::units::Unit;
use drydock_view::{Bounds, Camera, Projection, StandardView};
use glam::{DVec2, DVec3};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

const PROFILE_AT: DVec3 = DVec3::new(0.0, 0.4, 0.1);
const A_AT: DVec3 = DVec3::new(1.2, 0.0, 0.0);
const B_AT: DVec3 = DVec3::new(2.0, 1.2, 0.0);

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
}

impl World {
    fn draw(&mut self, points: &[DVec3], closed: bool) {
        let n = points.len();
        let count = if closed { n } else { n - 1 };
        let segs: Vec<_> = (0..count)
            .map(|i| (points[i], points[(i + 1) % n]))
            .collect();
        let mut undo = UndoStack::new();
        let mut t = Target {
            model: &mut self.model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: Unit::Meter,
        };
        draw_segments(&mut t, &segs, "Scene").expect("draw");
    }

    /// A 0.2 m square profile in x = 0 and an open two-edge path on the ground.
    fn open() -> World {
        let mut camera = Camera::default();
        camera.set_standard_view(StandardView::Iso);
        camera.projection = Projection::Parallel;
        camera.zoom_extents(
            &Bounds {
                min: DVec3::splat(-5.0),
                max: DVec3::splat(5.0),
            },
            800.0 / 600.0,
        );
        let mut editor = Editor::new();
        let mut model = Model::new();
        set_unit(&mut model, Unit::Meter);
        editor.set_tool(Tool::FollowMe);
        let mut w = World {
            model,
            editor,
            view: Viewport {
                camera,
                size_px: (800.0, 600.0),
            },
        };
        w.draw(
            &[
                v(0.0, 0.3, 0.0),
                v(0.0, 0.5, 0.0),
                v(0.0, 0.5, 0.2),
                v(0.0, 0.3, 0.2),
            ],
            true,
        );
        w.draw(
            &[v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0), v(2.0, 2.0, 0.0)],
            false,
        );
        w
    }

    fn px(&self, p: DVec3) -> DVec2 {
        self.view.project(p).expect("visible")
    }

    fn click(&mut self, p: DVec3) -> bool {
        let c = self.px(p);
        self.editor
            .pointer_move(&mut self.model, &self.view, c, NONE);
        self.editor.click(&mut self.model, &self.view, c, NONE, 1)
    }

    fn key(&mut self, key: EditKey) -> bool {
        self.editor.key(key, &mut self.model)
    }

    /// Profile and both path edges picked, preview showing.
    fn picked() -> World {
        let mut w = World::open();
        w.click(PROFILE_AT);
        w.click(A_AT);
        w.click(B_AT);
        assert!(w.editor.follow_profile().is_some());
        assert_eq!(w.editor.follow_path().len(), 2);
        assert!(!w.editor.preview().is_empty());
        w
    }

    fn counts(&self) -> (usize, usize, usize) {
        let c = self.model.root().geometry();
        (c.face_count(), c.edge_count(), c.vertex_count())
    }

    fn assert_idle(&self) {
        assert_eq!(self.editor.follow_profile(), None);
        assert!(self.editor.follow_path().is_empty());
        assert!(self.editor.preview().is_empty());
        assert_eq!(self.editor.follow_hint(), Some(HINT_PROFILE));
    }
}

#[test]
fn undo_drops_the_picks_and_the_preview() {
    let mut w = World::picked();
    w.key(EditKey::Undo);
    w.assert_idle();
}

#[test]
fn redo_drops_the_picks_and_the_preview() {
    let mut w = World::picked();
    w.key(EditKey::Redo);
    w.assert_idle();
}

#[test]
fn undo_drops_the_picks_when_something_was_undone() {
    let mut w = World::open();
    // One real undo step: a rectangle drawn with the editor's own tool.
    w.editor.set_tool(Tool::Line);
    w.click(v(-3.0, -3.0, 0.0));
    w.click(v(-2.0, -3.0, 0.0));
    w.editor.set_tool(Tool::FollowMe);
    w.click(PROFILE_AT);
    w.click(A_AT);
    w.key(EditKey::Undo);
    w.assert_idle();
}

#[test]
fn enter_after_undo_or_redo_changes_nothing() {
    for key in [EditKey::Undo, EditKey::Redo] {
        let mut w = World::picked();
        w.key(key);
        let before = w.counts();
        assert!(!w.key(EditKey::Enter));
        assert_eq!(w.counts(), before);
        w.assert_idle();
    }
}

#[test]
fn the_tool_works_again_after_an_undo() {
    let mut w = World::picked();
    w.key(EditKey::Undo);
    w.click(PROFILE_AT);
    w.click(A_AT);
    assert!(w.editor.follow_profile().is_some());
    assert_eq!(w.editor.follow_path().len(), 1);
    assert!(!w.editor.preview().is_empty());
}

fn set_unit(model: &mut Model, unit: Unit) {
    model
        .set_length_format(
            drydock_kernel::units::LengthFormat::Decimal { unit, decimals: 2 },
            &mut drydock_kernel::model::ModelEdit::new(),
        )
        .unwrap();
}
