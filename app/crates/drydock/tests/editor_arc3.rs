// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Three-point arc tool in the editor (`docs/specs/m2-arc-3pt-app.md`).

use drydock::editor::*;
use drydock::input::{Action, Button, EditKey, Modifiers, Tool, drag, key};
use drydock_kernel::model::{BodyId, Model, UndoStack};
use drydock_kernel::tools::{Target, draw_segments};
use drydock_kernel::topology::Context;
use drydock_kernel::units::{Unit, format_length};
use drydock_view::{Bounds, Camera, Projection, StandardView};
use glam::{DVec2, DVec3};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

fn mods(m: u8) -> Modifiers {
    Modifiers {
        shift: m & 1 != 0,
        ctrl: m & 2 != 0,
        alt: m & 4 != 0,
    }
}

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn assert_close(a: DVec3, b: DVec3) {
    assert!(a.distance(b) < 1e-6, "{a:?} != {b:?}");
}

fn f(x: f64) -> String {
    format_length(x, Unit::Meter, 2)
}

/// Parallel camera framing -5..5 on every axis, 800x600 px.
fn view(standard: StandardView) -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(standard);
    camera.projection = Projection::Parallel;
    camera.zoom_extents(
        &Bounds {
            min: DVec3::splat(-5.0),
            max: DVec3::splat(5.0),
        },
        800.0 / 600.0,
    );
    Viewport {
        camera,
        size_px: (800.0, 600.0),
    }
}

struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
}

impl World {
    fn new(tool: Tool) -> World {
        World::with_view(tool, StandardView::Top)
    }

    fn with_view(tool: Tool, standard: StandardView) -> World {
        let mut editor = Editor::new();
        let mut model = Model::new();
        set_unit(&mut model, Unit::Meter);
        editor.set_tool(tool);
        World {
            model,
            editor,
            view: view(standard),
        }
    }

    /// The 2 m square wall (0,0,0)-(2,0,2) in the plane y = 0, drawn straight into the model.
    fn wall(&mut self) {
        let pts = [
            v(0.0, 0.0, 0.0),
            v(2.0, 0.0, 0.0),
            v(2.0, 0.0, 2.0),
            v(0.0, 0.0, 2.0),
        ];
        let segs: Vec<_> = (0..4).map(|i| (pts[i], pts[(i + 1) % 4])).collect();
        let mut undo = UndoStack::new();
        let mut t = Target {
            model: &mut self.model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: Unit::Meter,
        };
        draw_segments(&mut t, &segs, "Wall").expect("wall");
        assert_eq!(self.ctx().face_count(), 1);
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

    fn key(&mut self, key: EditKey) -> bool {
        self.editor.key(key, &mut self.model)
    }

    /// Types `s` and presses Enter; returns what Enter returned.
    fn enter(&mut self, s: &str) -> bool {
        assert!(self.editor.text(s), "text {s:?} refused");
        self.key(EditKey::Enter)
    }

    fn ctx(&self) -> &Context {
        self.model.root().geometry()
    }

    fn vertices(&self) -> Vec<DVec3> {
        self.ctx().vertices().map(|(_, v)| v.position).collect()
    }

    fn has_vertex(&self, p: DVec3) -> bool {
        self.vertices().iter().any(|q| q.distance(p) < 1e-6)
    }

    fn measurement(&self) -> (&'static str, String) {
        self.editor.measurement(&self.model).expect("measurement")
    }
}

// ---------------------------------------------------------------- keys and drags

#[test]
fn shift_a_picks_the_three_point_arc() {
    assert_eq!(key("A", mods(1)), Some(Action::SelectTool(Tool::Arc3)));
    assert_eq!(key("A", mods(0)), Some(Action::SelectTool(Tool::Arc)));
    for m in 2..8 {
        assert_eq!(key("A", mods(m)), None, "{:?}", mods(m));
    }
}

#[test]
fn arc3_leaves_left_drags_to_the_editor() {
    for m in 0..8 {
        let d = DVec2::new(5.0, 6.0);
        assert_eq!(
            drag(Tool::Arc3, Button::Left, mods(m), d, DVec2::ZERO),
            None
        );
    }
    assert!(matches!(
        drag(
            Tool::Arc3,
            Button::Middle,
            mods(0),
            DVec2::new(5.0, 6.0),
            DVec2::ZERO
        ),
        Some(Action::Orbit { .. })
    ));
}

// ---------------------------------------------------------------- editor

#[test]
fn arc3_starts_on_the_ground_plane() {
    let w = World::new(Tool::Arc3);
    assert_eq!(w.editor.tool(), Tool::Arc3);
    let p = w.editor.draw_plane().expect("draw plane");
    assert_close(p.u, DVec3::X);
    assert_close(p.v, DVec3::Y);
    assert_eq!(w.editor.plane_lock(), None);
    assert_eq!(w.editor.lock(), None);
    assert!(w.editor.preview().is_empty());
    assert_eq!(w.measurement(), ("Segments", "12".to_string()));
}

#[test]
fn arc3_three_clicks_draw_a_half_circle() {
    let mut w = World::new(Tool::Arc3);
    assert!(!w.click(v(0.0, 0.0, 0.0)));
    w.move_to(v(2.0, 0.0, 0.0));
    assert_eq!(w.measurement(), ("Length", f(2.0)));
    let preview = w.editor.preview();
    assert_eq!(preview.len(), 1);
    assert_close(preview[0].0, v(0.0, 0.0, 0.0));
    assert_close(preview[0].1, v(2.0, 0.0, 0.0));
    assert!(!w.click(v(2.0, 0.0, 0.0)));
    assert_eq!(w.ctx().edge_count(), 0);
    w.move_to(v(1.0, 1.0, 0.0));
    assert_eq!(w.measurement(), ("Bulge", f(1.0)));
    let preview = w.editor.preview();
    assert_eq!(preview.len(), 12);
    assert_close(preview[0].0, v(0.0, 0.0, 0.0));
    assert_close(preview[11].1, v(2.0, 0.0, 0.0));
    assert!(w.click(v(1.0, 1.0, 0.0)));
    assert_eq!(w.ctx().edge_count(), 12);
    assert_eq!(w.ctx().face_count(), 0);
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Arc"));
    for p in [v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0), v(1.0, 1.0, 0.0)] {
        assert!(w.has_vertex(p), "{p:?}");
    }
    assert_eq!(w.measurement(), ("Segments", "12".to_string()));
    assert!(w.editor.preview().is_empty());
}

#[test]
fn arc3_bulge_of_more_than_half_a_turn() {
    // The circle through (0,0), (2,0) and (1,1.5) has its center at (1, 5/12): the arc
    // through (1,1.5) is the long way round, and (1,1.5) is its highest point.
    let mut w = World::new(Tool::Arc3);
    w.click(v(0.0, 0.0, 0.0));
    w.click(v(2.0, 0.0, 0.0));
    w.move_to(v(1.0, 1.5, 0.0));
    assert_eq!(w.measurement(), ("Bulge", f(1.5)));
    w.move_to(v(1.0, -0.5, 0.0));
    assert_eq!(w.measurement(), ("Bulge", f(0.5)));
}

#[test]
fn arc3_typed_chord_then_bulge_follows_the_cursor_side() {
    let mut w = World::new(Tool::Arc3);
    w.click(v(0.0, 0.0, 0.0));
    w.move_to(v(1.0, 0.0, 0.0));
    assert!(!w.enter("2"));
    assert_eq!(w.ctx().edge_count(), 0);
    assert_eq!(w.measurement(), ("Bulge", String::new()));
    w.move_to(v(1.0, -0.5, 0.0));
    assert!(w.enter("1"));
    assert_eq!(w.ctx().edge_count(), 12);
    for p in [v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0), v(1.0, -1.0, 0.0)] {
        assert!(w.has_vertex(p), "{p:?}");
    }
    assert!(!w.has_vertex(v(1.0, 1.0, 0.0)));
}

#[test]
fn arc3_zero_bulge_is_refused() {
    let mut w = World::new(Tool::Arc3);
    w.click(v(0.0, 0.0, 0.0));
    w.click(v(2.0, 0.0, 0.0));
    assert!(!w.enter("0"));
    assert!(w.editor.message().is_some());
    assert_eq!(w.measurement().0, "Bulge");
    assert_eq!(w.ctx().edge_count(), 0);
    assert!(w.enter("1"));
    assert!(w.has_vertex(v(1.0, 1.0, 0.0)));
    assert_eq!(w.editor.message(), None);
}

#[test]
fn arc3_typed_while_idle_sets_a_message() {
    let mut w = World::new(Tool::Arc3);
    assert!(!w.enter("2"));
    assert!(w.editor.message().is_some());
    assert_eq!(w.ctx().edge_count(), 0);
}

#[test]
fn arc3_segment_count_is_its_own() {
    let mut w = World::new(Tool::Arc3);
    assert!(!w.enter("6s"));
    assert_eq!(w.measurement(), ("Segments", "6".to_string()));
    w.editor.set_tool(Tool::Arc);
    assert_eq!(w.measurement(), ("Segments", "12".to_string()));
    w.enter("4s");
    w.editor.set_tool(Tool::Arc3);
    assert_eq!(w.measurement(), ("Segments", "6".to_string()));
    w.click(v(0.0, 0.0, 0.0));
    w.click(v(2.0, 0.0, 0.0));
    assert!(w.click(v(1.0, 1.0, 0.0)));
    assert_eq!(w.ctx().edge_count(), 6);
}

#[test]
fn arc3_escape_and_tool_change_drop_the_arc_in_progress() {
    let mut w = World::new(Tool::Arc3);
    w.click(v(0.0, 0.0, 0.0));
    w.click(v(2.0, 0.0, 0.0));
    w.move_to(v(1.0, 1.0, 0.0));
    w.key(EditKey::Cancel);
    assert_eq!(w.measurement(), ("Segments", "12".to_string()));
    assert!(w.editor.preview().is_empty());

    w.click(v(0.0, 0.0, 0.0));
    w.editor.set_tool(Tool::Line);
    w.editor.set_tool(Tool::Arc3);
    assert_eq!(w.measurement(), ("Segments", "12".to_string()));
    assert_eq!(w.ctx().edge_count(), 0);
}

#[test]
fn arc3_undo_and_redo() {
    let mut w = World::new(Tool::Arc3);
    w.click(v(0.0, 0.0, 0.0));
    w.click(v(2.0, 0.0, 0.0));
    w.click(v(1.0, 1.0, 0.0));
    assert!(w.key(EditKey::Undo));
    assert_eq!(w.ctx().edge_count(), 0);
    assert!(w.key(EditKey::Redo));
    assert_eq!(w.ctx().edge_count(), 12);
}

#[test]
fn arc3_takes_typed_text() {
    let mut w = World::new(Tool::Arc3);
    assert!(w.editor.text("1"));
    assert_eq!(w.editor.typed(), "1");
}

#[test]
fn arc3_on_a_wall_stays_in_its_plane() {
    let mut w = World::with_view(Tool::Arc3, StandardView::Front);
    w.wall();
    assert!(!w.click(v(0.5, 0.0, 0.5)));
    let plane = w.editor.draw_plane().unwrap();
    assert!(plane.normal().y.abs() > 1.0 - 1e-6, "{plane:?}");
    assert!(!w.click(v(1.5, 0.0, 0.5)));
    assert_eq!(w.editor.draw_plane(), Some(plane));
    assert!(w.click(v(1.0, 0.0, 1.0)));
    assert!(w.has_vertex(v(1.0, 0.0, 1.0)));
    for p in w.vertices() {
        assert!(p.y.abs() < 1e-6, "{p:?}");
    }
}

/// Typed numbers and measurements in `unit`, 2 places.
fn set_unit(model: &mut Model, unit: Unit) {
    model
        .set_length_format(
            drydock_kernel::units::LengthFormat::Decimal { unit, decimals: 2 },
            &mut drydock_kernel::model::ModelEdit::new(),
        )
        .unwrap();
}
