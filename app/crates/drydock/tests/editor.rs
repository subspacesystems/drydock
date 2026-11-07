// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Select and line tools in the editor (`docs/specs/m2-app-editor.md`).

use drydock::editor::*;
use drydock::input::{EditKey, Modifiers, Tool};
use drydock_kernel::infer::Snap;
use drydock_kernel::model::Model;
use drydock_kernel::select::Entity;
use drydock_kernel::topology::Context;
use drydock_kernel::units::Unit;
use drydock_view::{Bounds, Camera, Projection, StandardView};
use glam::{DVec2, DVec3};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};
const SHIFT: Modifiers = Modifiers {
    shift: true,
    ctrl: false,
    alt: false,
};
const CTRL: Modifiers = Modifiers {
    shift: false,
    ctrl: true,
    alt: false,
};
const ALT: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: true,
};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn assert_close(a: DVec3, b: DVec3) {
    assert!(a.distance(b) < 1e-6, "{a:?} != {b:?}");
}

/// Top view, parallel, framing -5..5 on every axis, 800x600 px.
fn top_view() -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(StandardView::Top);
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
        let mut editor = Editor::new();
        let mut model = Model::new();
        set_unit(&mut model, Unit::Meter);
        editor.set_tool(tool);
        World {
            model,
            editor,
            view: top_view(),
        }
    }

    fn px(&self, p: DVec3) -> DVec2 {
        self.view.project(p).expect("visible")
    }

    fn move_to(&mut self, p: DVec3, mods: Modifiers) {
        let c = self.px(p);
        self.editor
            .pointer_move(&mut self.model, &self.view, c, mods);
    }

    fn click_n(&mut self, p: DVec3, mods: Modifiers, count: u32) -> bool {
        let c = self.px(p);
        self.editor
            .click(&mut self.model, &self.view, c, mods, count)
    }

    fn click(&mut self, p: DVec3) -> bool {
        self.move_to(p, NONE);
        self.click_n(p, NONE, 1)
    }

    fn key(&mut self, key: EditKey) -> bool {
        self.editor.key(key, &mut self.model)
    }

    fn ctx(&self) -> &Context {
        self.model.root().geometry()
    }

    fn edge_ends(&self) -> Vec<(DVec3, DVec3)> {
        let ctx = self.ctx();
        ctx.edges()
            .map(|(_, e)| {
                let p = |id| ctx.vertex(id).expect("vertex").position;
                (p(e.ends[0]), p(e.ends[1]))
            })
            .collect()
    }

    /// The 2 m square (0,0)-(2,2) on the ground, drawn with the line tool. Leaves the tool
    /// as it was.
    fn square(&mut self) {
        let tool = self.editor.tool();
        self.editor.set_tool(Tool::Line);
        for p in [
            v(0.0, 0.0, 0.0),
            v(2.0, 0.0, 0.0),
            v(2.0, 2.0, 0.0),
            v(0.0, 2.0, 0.0),
            v(0.0, 0.0, 0.0),
        ] {
            self.click(p);
        }
        assert_eq!(self.ctx().face_count(), 1);
        assert_eq!(self.ctx().edge_count(), 4);
        self.editor.set_tool(tool);
    }

    fn face(&self) -> Entity {
        let (id, _) = self.ctx().faces().next().expect("a face");
        Entity::Face(id)
    }

    fn edges(&self) -> Vec<Entity> {
        self.ctx().edges().map(|(id, _)| Entity::Edge(id)).collect()
    }

    /// The edge whose midpoint is `m`.
    fn edge_at(&self, m: DVec3) -> Entity {
        let ctx = self.ctx();
        let (id, _) = ctx
            .edges()
            .find(|(_, e)| {
                let a = ctx.vertex(e.ends[0]).expect("vertex").position;
                let b = ctx.vertex(e.ends[1]).expect("vertex").position;
                ((a + b) / 2.0).distance(m) < 1e-6
            })
            .expect("edge with that midpoint");
        Entity::Edge(id)
    }
}

fn sorted(mut v: Vec<Entity>) -> Vec<Entity> {
    v.sort();
    v.dedup();
    v
}

// ---------------------------------------------------------------- viewport

#[test]
fn project_puts_the_target_in_the_middle() {
    let view = top_view();
    let c = view.project(view.camera.target).expect("visible");
    assert!(c.distance(DVec2::new(400.0, 300.0)) < 1e-6, "{c:?}");
}

#[test]
fn project_top_view_x_right_y_up() {
    let view = top_view();
    let o = view.project(DVec3::ZERO).unwrap();
    let x = view.project(DVec3::X).unwrap();
    let y = view.project(DVec3::Y).unwrap();
    // Some yaw: just check screen distances are equal and the axes are perpendicular.
    let dx = x - o;
    let dy = y - o;
    assert!((dx.length() - dy.length()).abs() < 1e-6);
    assert!(dx.dot(dy).abs() < 1e-6);
    assert!(dx.length() > 30.0, "1 m is {} px", dx.length());
}

#[test]
fn ray_top_view_parallel_points_down_from_above() {
    let view = top_view();
    let (origin, dir) = view.ray(DVec2::new(400.0, 300.0));
    assert!((dir.length() - 1.0).abs() < 1e-9);
    assert!(dir.distance(-DVec3::Z) < 1e-9, "{dir:?}");
    assert!(origin.z > 1000.0, "{origin:?}");
    assert!(origin.x.abs() < 1e-6 && origin.y.abs() < 1e-6, "{origin:?}");
}

#[test]
fn ray_parallel_rays_are_parallel() {
    let view = top_view();
    let (o1, d1) = view.ray(DVec2::new(10.0, 20.0));
    let (o2, d2) = view.ray(DVec2::new(700.0, 500.0));
    assert!(d1.distance(d2) < 1e-12);
    assert!(o1.distance(o2) > 1.0);
}

#[test]
fn ray_perspective_starts_at_the_eye() {
    let mut view = top_view();
    view.camera.projection = Projection::Perspective;
    let (o1, d1) = view.ray(DVec2::new(10.0, 20.0));
    let (o2, d2) = view.ray(DVec2::new(700.0, 500.0));
    assert_close(o1, view.camera.eye());
    assert_close(o2, view.camera.eye());
    assert!((d1.length() - 1.0).abs() < 1e-9);
    assert!(d1.distance(d2) > 1e-3);
}

#[test]
fn ray_points_project_back_to_the_cursor() {
    for projection in [Projection::Parallel, Projection::Perspective] {
        let mut view = top_view();
        view.camera.projection = projection;
        view.camera.set_standard_view(StandardView::Iso);
        for c in [
            DVec2::new(0.0, 0.0),
            DVec2::new(400.0, 300.0),
            DVec2::new(123.0, 456.0),
        ] {
            let (o, d) = view.ray(c);
            let t = (view.camera.eye() - o).length() + view.camera.distance;
            let back = view.project(o + d * t).expect("in front");
            assert!(back.distance(c) < 1e-6, "{projection:?} {c:?} -> {back:?}");
        }
    }
}

#[test]
fn bad_viewport_size_is_one_pixel() {
    let mut view = top_view();
    for size in [
        (0.0, 0.0),
        (-5.0, 10.0),
        (f64::NAN, 10.0),
        (10.0, f64::INFINITY),
    ] {
        view.size_px = size;
        let (o, d) = view.ray(DVec2::new(0.5, 0.5));
        assert!(o.is_finite() && d.is_finite(), "{size:?}");
        let c = view.project(view.camera.target).expect("visible");
        assert!(c.distance(DVec2::new(0.5, 0.5)) < 1e-9, "{size:?} {c:?}");
    }
}

#[test]
fn project_behind_a_perspective_camera_is_none() {
    let mut view = top_view();
    view.camera.projection = Projection::Perspective;
    let behind = view.camera.eye() + DVec3::Z * 10.0;
    assert_eq!(view.project(behind), None);
}

// ---------------------------------------------------------------- state

#[test]
fn new_editor_defaults() {
    let e = Editor::new();
    assert_eq!(e.tool(), Tool::Select);
    assert_eq!(Model::new().length_format().input_unit(), Unit::Inch);
    assert!(e.selection().is_empty());
    assert_eq!(e.undo_stack().undo_count(), 0);
    assert_eq!(e.typed(), "");
    assert_eq!(e.hint(), None);
    assert_eq!(e.lock(), None);
    assert!(e.preview().is_empty());
    assert_eq!(e.measurement(&Model::new()), None);
    assert_eq!(e.message(), None);
    assert_eq!(Editor::default().tool(), Tool::Select);
}

#[test]
fn view_tools_ignore_clicks_and_moves() {
    for tool in [Tool::Orbit, Tool::Pan, Tool::Zoom] {
        let mut w = World::new(tool);
        w.move_to(v(1.0, 1.0, 0.0), NONE);
        assert_eq!(w.editor.hint(), None);
        assert!(!w.click_n(v(1.0, 1.0, 0.0), NONE, 1));
        assert_eq!(w.ctx().edge_count(), 0);
        assert_eq!(w.editor.measurement(&w.model), None);
        assert!(!w.editor.text("5"));
        assert_eq!(w.editor.typed(), "");
    }
}

// ---------------------------------------------------------------- line tool

#[test]
fn line_tool_hint_follows_the_cursor() {
    let mut w = World::new(Tool::Line);
    w.move_to(v(1.0, 0.5, 0.0), NONE);
    let hint = w.editor.hint().expect("hint");
    assert_close(hint.point, v(1.0, 0.5, 0.0));
    assert_eq!(hint.snap, Snap::Ground);
    assert!(w.editor.preview().is_empty(), "no start yet");
    assert_eq!(
        w.editor.measurement(&w.model),
        Some(("Length", String::new()))
    );
}

#[test]
fn line_two_clicks_draw_an_edge() {
    let mut w = World::new(Tool::Line);
    assert!(!w.click(v(0.0, 0.0, 0.0)), "first click only starts");
    w.move_to(v(2.0, 0.0, 0.0), NONE);
    let preview = w.editor.preview();
    assert_eq!(preview.len(), 1);
    assert_close(preview[0].0, v(0.0, 0.0, 0.0));
    assert_close(preview[0].1, v(2.0, 0.0, 0.0));
    assert_eq!(
        w.editor.measurement(&w.model),
        Some(("Length", "2m".to_string()))
    );
    assert!(w.click_n(v(2.0, 0.0, 0.0), NONE, 1));
    assert_eq!(w.ctx().edge_count(), 1);
    assert_eq!(w.editor.undo_stack().undo_count(), 1);
    let (a, b) = w.edge_ends()[0];
    assert_close(a, v(0.0, 0.0, 0.0));
    assert_close(b, v(2.0, 0.0, 0.0));
}

#[test]
fn line_chain_continues_from_the_last_point() {
    let mut w = World::new(Tool::Line);
    w.click(v(0.0, 0.0, 0.0));
    w.click(v(2.0, 0.0, 0.0));
    w.move_to(v(2.0, 1.0, 0.0), NONE);
    let preview = w.editor.preview();
    assert_eq!(preview.len(), 1);
    assert_close(preview[0].0, v(2.0, 0.0, 0.0));
    assert_close(preview[0].1, v(2.0, 1.0, 0.0));
    assert!(w.click_n(v(2.0, 1.0, 0.0), NONE, 1));
    assert_eq!(w.ctx().edge_count(), 2);
    assert_eq!(w.editor.undo_stack().undo_count(), 2);
}

#[test]
fn line_click_on_the_start_point_does_nothing() {
    let mut w = World::new(Tool::Line);
    w.click(v(1.0, 1.0, 0.0));
    assert!(!w.click(v(1.0, 1.0, 0.0)));
    assert_eq!(w.ctx().edge_count(), 0);
    assert_eq!(w.editor.message(), None);
}

#[test]
fn line_closing_a_square_makes_a_face_and_ends_the_chain() {
    let mut w = World::new(Tool::Line);
    w.square();
    w.editor.set_tool(Tool::Line);
    w.move_to(v(3.0, 1.0, 0.0), NONE);
    assert!(w.editor.preview().is_empty());
}

#[test]
fn line_snaps_to_an_existing_endpoint() {
    let mut w = World::new(Tool::Line);
    w.click(v(0.0, 0.0, 0.0));
    w.click(v(2.0, 0.0, 0.0));
    w.key(EditKey::Cancel);
    // A few pixels off the endpoint (2,0,0) still lands on it.
    let c = w.px(v(2.0, 0.0, 0.0)) + DVec2::new(3.0, -2.0);
    w.editor.pointer_move(&mut w.model, &w.view, c, NONE);
    let hint = w.editor.hint().expect("hint");
    assert!(matches!(hint.snap, Snap::Endpoint(_)), "{hint:?}");
    assert_close(hint.point, v(2.0, 0.0, 0.0));
}

#[test]
fn line_axis_hint() {
    let mut w = World::new(Tool::Line);
    w.click(v(0.0, 0.0, 0.0));
    w.move_to(v(0.0, 2.0, 0.0), NONE);
    assert_eq!(w.editor.hint().expect("hint").axis, Some(1));
}

#[test]
fn measurement_uses_the_model_unit() {
    let mut w = World::new(Tool::Line);
    set_unit(&mut w.model, Unit::Inch);
    w.click(v(0.0, 0.0, 0.0));
    w.move_to(v(0.0254 * 40.0, 0.0, 0.0), NONE);
    assert_eq!(
        w.editor.measurement(&w.model),
        Some(("Length", "40\"".to_string()))
    );
}

// ---------------------------------------------------------------- typed text

#[test]
fn typed_length_draws_along_the_cursor() {
    let mut w = World::new(Tool::Line);
    w.click(v(0.0, 0.0, 0.0));
    w.move_to(v(1.0, 0.0, 0.0), NONE);
    assert!(w.editor.text("3"));
    assert_eq!(w.editor.typed(), "3");
    assert!(w.key(EditKey::Enter));
    assert_eq!(w.editor.typed(), "");
    assert_eq!(w.ctx().edge_count(), 1);
    let (a, b) = w.edge_ends()[0];
    assert_close(a, v(0.0, 0.0, 0.0));
    assert_close(b, v(3.0, 0.0, 0.0));
}

#[test]
fn typed_bare_number_uses_the_model_unit() {
    let mut w = World::new(Tool::Line);
    set_unit(&mut w.model, Unit::Inch);
    w.click(v(0.0, 0.0, 0.0));
    w.move_to(v(0.0, 1.0, 0.0), NONE);
    w.editor.text("10");
    assert!(w.key(EditKey::Enter));
    let (_, b) = w.edge_ends()[0];
    assert_close(b, v(0.0, 0.254, 0.0));
}

#[test]
fn typed_text_with_units_types_through() {
    let mut w = World::new(Tool::Line);
    assert!(w.editor.text("5"));
    assert!(w.editor.text("0"));
    assert!(w.editor.text("cm"));
    assert_eq!(w.editor.typed(), "50cm");
    w.click(v(0.0, 0.0, 0.0));
    w.move_to(v(0.0, -1.0, 0.0), NONE);
    assert!(w.key(EditKey::Enter));
    let (_, b) = w.edge_ends()[0];
    assert_close(b, v(0.0, -0.5, 0.0));
}

#[test]
fn text_must_start_like_a_number() {
    let mut w = World::new(Tool::Line);
    for s in ["m", "L", " ", " 5", "'", "\"", "x5"] {
        assert!(!w.editor.text(s), "{s:?}");
        assert_eq!(w.editor.typed(), "", "{s:?}");
    }
    for s in ["5", ".5", "-2", "+1", ",3"] {
        let mut w = World::new(Tool::Line);
        assert!(w.editor.text(s), "{s:?}");
        assert_eq!(w.editor.typed(), s);
    }
}

#[test]
fn text_drops_control_characters() {
    let mut w = World::new(Tool::Line);
    assert!(w.editor.text("\u{8}\t5"));
    assert_eq!(w.editor.typed(), "5");
    assert!(w.editor.text("' 4\"\n"));
    assert_eq!(w.editor.typed(), "5' 4\"");
    assert!(!w.editor.text("\r"));
    assert_eq!(w.editor.typed(), "5' 4\"");
}

#[test]
fn text_is_refused_by_other_tools() {
    let mut w = World::new(Tool::Select);
    assert!(!w.editor.text("5"));
    assert_eq!(w.editor.typed(), "");
}

#[test]
fn backspace_drops_the_last_character() {
    let mut w = World::new(Tool::Line);
    w.editor.text("12cm");
    assert!(!w.key(EditKey::Backspace));
    assert_eq!(w.editor.typed(), "12c");
    w.key(EditKey::Backspace);
    w.key(EditKey::Backspace);
    w.key(EditKey::Backspace);
    assert_eq!(w.editor.typed(), "");
    w.key(EditKey::Backspace);
    assert_eq!(w.editor.typed(), "");
}

#[test]
fn enter_with_nothing_typed_does_nothing() {
    let mut w = World::new(Tool::Line);
    w.click(v(0.0, 0.0, 0.0));
    w.move_to(v(1.0, 0.0, 0.0), NONE);
    assert!(!w.key(EditKey::Enter));
    assert_eq!(w.ctx().edge_count(), 0);
    assert_eq!(w.editor.message(), None);
}

#[test]
fn refused_typed_text_sets_a_message_and_is_cleared() {
    let mut w = World::new(Tool::Line);
    w.editor.text("5");
    assert!(!w.key(EditKey::Enter), "nothing started");
    assert!(w.editor.message().is_some());
    assert_eq!(w.editor.typed(), "");

    w.click(v(0.0, 0.0, 0.0));
    assert_eq!(w.editor.message(), None, "an accepted click clears it");
    w.move_to(v(1.0, 0.0, 0.0), NONE);
    w.editor.text("5 apples");
    assert!(!w.key(EditKey::Enter));
    assert!(w.editor.message().is_some());
    assert_eq!(w.editor.typed(), "");
    assert_eq!(w.ctx().edge_count(), 0);

    w.editor.text("1");
    assert!(w.key(EditKey::Enter));
    assert_eq!(w.editor.message(), None);
}

// ---------------------------------------------------------------- cancel, undo

#[test]
fn cancel_drops_the_line_and_the_text() {
    let mut w = World::new(Tool::Line);
    w.click(v(0.0, 0.0, 0.0));
    w.move_to(v(1.0, 0.0, 0.0), NONE);
    w.editor.text("4");
    w.key(EditKey::Lock(1));
    assert!(!w.key(EditKey::Cancel));
    assert_eq!(w.editor.typed(), "");
    assert_eq!(w.editor.lock(), None);
    assert!(w.editor.preview().is_empty());
    assert!(w.editor.hint().is_some(), "the hint stays");
    w.move_to(v(1.0, 1.0, 0.0), NONE);
    assert!(w.editor.preview().is_empty(), "idle");
    assert_eq!(w.ctx().edge_count(), 0);
}

#[test]
fn set_tool_resets_everything_but_selection_and_undo() {
    let mut w = World::new(Tool::Line);
    w.square();
    w.editor.set_tool(Tool::Select);
    let f = w.face();
    w.click_n(v(1.0, 1.0, 0.0), NONE, 1);
    assert_eq!(w.editor.selection(), &[f]);

    w.editor.set_tool(Tool::Line);
    w.click(v(4.0, 0.0, 0.0));
    w.move_to(v(4.0, 1.0, 0.0), NONE);
    w.editor.text("7");
    w.key(EditKey::Lock(0));
    w.editor.set_tool(Tool::Line);
    assert_eq!(w.editor.tool(), Tool::Line);
    assert_eq!(w.editor.typed(), "");
    assert_eq!(w.editor.lock(), None);
    assert_eq!(w.editor.hint(), None);
    assert!(w.editor.preview().is_empty());
    assert_eq!(w.editor.selection(), &[f]);
    assert_eq!(w.editor.undo_stack().undo_count(), 4);
}

#[test]
fn undo_and_redo_edges() {
    let mut w = World::new(Tool::Line);
    w.click(v(0.0, 0.0, 0.0));
    w.click(v(2.0, 0.0, 0.0));
    w.click(v(2.0, 2.0, 0.0));
    assert_eq!(w.ctx().edge_count(), 2);
    assert!(w.key(EditKey::Undo));
    assert_eq!(w.ctx().edge_count(), 1);
    assert!(w.key(EditKey::Undo));
    assert_eq!(w.ctx().edge_count(), 0);
    assert!(!w.key(EditKey::Undo), "nothing left");
    assert!(w.key(EditKey::Redo));
    assert_eq!(w.ctx().edge_count(), 1);
    assert!(w.key(EditKey::Redo));
    assert_eq!(w.ctx().edge_count(), 2);
    assert!(!w.key(EditKey::Redo));
    assert_eq!(w.editor.message(), None);
}

#[test]
fn undo_drops_the_line_in_progress() {
    let mut w = World::new(Tool::Line);
    w.click(v(0.0, 0.0, 0.0));
    w.click(v(2.0, 0.0, 0.0));
    w.editor.text("3");
    assert!(w.key(EditKey::Undo));
    assert_eq!(w.editor.typed(), "");
    w.move_to(v(2.0, 1.0, 0.0), NONE);
    assert!(w.editor.preview().is_empty());
}

#[test]
fn undo_prunes_the_selection() {
    let mut w = World::new(Tool::Select);
    w.square();
    let f = w.face();
    w.click_n(v(1.0, 1.0, 0.0), NONE, 2);
    assert_eq!(w.editor.selection().len(), 5);
    assert!(
        w.key(EditKey::Undo),
        "removes the closing edge and the face"
    );
    assert_eq!(w.ctx().edge_count(), 3);
    assert_eq!(w.editor.selection().len(), 3);
    assert!(!w.editor.selection().contains(&f));
    for e in w.editor.selection() {
        let Entity::Edge(id) = e else { panic!("{e:?}") };
        assert!(w.ctx().edge(*id).is_some());
    }
}

// ---------------------------------------------------------------- locks

#[test]
fn arrow_lock_toggles() {
    let mut w = World::new(Tool::Line);
    assert!(!w.key(EditKey::Lock(0)));
    assert_eq!(w.editor.lock(), Some(DVec3::X));
    w.key(EditKey::Lock(0));
    assert_eq!(w.editor.lock(), None);
    w.key(EditKey::Lock(1));
    assert_eq!(w.editor.lock(), Some(DVec3::Y));
    w.key(EditKey::Lock(2));
    assert_eq!(w.editor.lock(), Some(DVec3::Z));
    w.key(EditKey::Lock(3));
    assert_eq!(w.editor.lock(), Some(DVec3::Z));
}

#[test]
fn arrow_lock_needs_the_line_tool() {
    let mut w = World::new(Tool::Select);
    w.key(EditKey::Lock(0));
    assert_eq!(w.editor.lock(), None);
}

#[test]
fn arrow_lock_keeps_the_line_on_the_axis_then_releases() {
    let mut w = World::new(Tool::Line);
    w.click(v(0.0, 0.0, 0.0));
    w.key(EditKey::Lock(0));
    w.move_to(v(2.0, 1.0, 0.0), NONE);
    assert_close(w.editor.hint().expect("hint").point, v(2.0, 0.0, 0.0));
    assert!(w.click_n(v(2.0, 1.0, 0.0), NONE, 1));
    let (_, b) = w.edge_ends()[0];
    assert_close(b, v(2.0, 0.0, 0.0));
    assert_eq!(w.editor.lock(), None, "released after the click");
}

#[test]
fn shift_locks_the_axis_shown_by_the_previous_hint() {
    let mut w = World::new(Tool::Line);
    w.click(v(0.0, 0.0, 0.0));
    w.move_to(v(2.0, 0.0, 0.0), NONE);
    assert_eq!(w.editor.hint().expect("hint").axis, Some(0));
    w.move_to(v(2.0, 1.0, 0.0), SHIFT);
    assert_eq!(w.editor.lock(), Some(DVec3::X));
    assert_close(w.editor.hint().expect("hint").point, v(2.0, 0.0, 0.0));
    w.move_to(v(3.0, 2.0, 0.0), SHIFT);
    assert_close(w.editor.hint().expect("hint").point, v(3.0, 0.0, 0.0));
    w.move_to(v(3.0, 2.0, 0.0), NONE);
    assert_eq!(w.editor.lock(), None);
    assert_close(w.editor.hint().expect("hint").point, v(3.0, 2.0, 0.0));
}

#[test]
fn shift_locks_an_off_axis_direction() {
    let mut w = World::new(Tool::Line);
    w.click(v(0.0, 0.0, 0.0));
    w.move_to(v(2.0, 1.0, 0.0), NONE);
    assert_eq!(w.editor.hint().expect("hint").axis, None);
    w.move_to(v(4.0, 0.0, 0.0), SHIFT);
    let lock = w.editor.lock().expect("locked");
    assert!(
        lock.distance(v(2.0, 1.0, 0.0).normalize()) < 1e-6,
        "{lock:?}"
    );
    let p = w.editor.hint().expect("hint").point;
    assert!(
        p.cross(v(2.0, 1.0, 0.0)).length() < 1e-6,
        "on the line: {p:?}"
    );
}

#[test]
fn shift_without_a_start_point_does_not_lock() {
    let mut w = World::new(Tool::Line);
    w.move_to(v(2.0, 0.0, 0.0), NONE);
    w.move_to(v(2.0, 1.0, 0.0), SHIFT);
    assert_eq!(w.editor.lock(), None);
}

#[test]
fn arrow_lock_wins_over_shift() {
    let mut w = World::new(Tool::Line);
    w.click(v(0.0, 0.0, 0.0));
    w.move_to(v(2.0, 0.0, 0.0), NONE);
    w.move_to(v(2.0, 1.0, 0.0), SHIFT);
    assert_eq!(w.editor.lock(), Some(DVec3::X));
    w.key(EditKey::Lock(1));
    assert_eq!(w.editor.lock(), Some(DVec3::Y));
    w.key(EditKey::Lock(1));
    assert_eq!(w.editor.lock(), Some(DVec3::X), "Shift lock is still held");
}

// ---------------------------------------------------------------- select tool

#[test]
fn select_tool_has_no_hint_and_draws_nothing() {
    let mut w = World::new(Tool::Select);
    w.move_to(v(1.0, 1.0, 0.0), NONE);
    assert_eq!(w.editor.hint(), None);
    assert!(!w.click_n(v(1.0, 1.0, 0.0), NONE, 1));
    assert!(w.editor.preview().is_empty());
    assert_eq!(w.editor.measurement(&w.model), None);
}

#[test]
fn click_selects_face_or_edge() {
    let mut w = World::new(Tool::Select);
    w.square();
    let f = w.face();
    let bottom = w.edge_at(v(1.0, 0.0, 0.0));
    w.click_n(v(1.0, 1.0, 0.0), NONE, 1);
    assert_eq!(w.editor.selection(), &[f]);
    w.click_n(v(1.0, 0.0, 0.0), NONE, 1);
    assert_eq!(w.editor.selection(), &[bottom]);
    w.click_n(v(4.0, 4.0, 0.0), NONE, 1);
    assert!(w.editor.selection().is_empty(), "empty space clears");
}

#[test]
fn click_modifiers() {
    let mut w = World::new(Tool::Select);
    w.square();
    let f = w.face();
    let bottom = w.edge_at(v(1.0, 0.0, 0.0));
    let right = w.edge_at(v(2.0, 1.0, 0.0));
    w.click_n(v(1.0, 0.0, 0.0), NONE, 1);
    w.click_n(v(2.0, 1.0, 0.0), SHIFT, 1);
    assert_eq!(w.editor.selection(), sorted(vec![bottom, right]).as_slice());
    w.click_n(v(1.0, 1.0, 0.0), CTRL, 1);
    assert_eq!(
        w.editor.selection(),
        sorted(vec![bottom, right, f]).as_slice()
    );
    w.click_n(v(1.0, 0.0, 0.0), CTRL, 1);
    assert_eq!(w.editor.selection(), sorted(vec![right, f]).as_slice());
    w.click_n(v(1.0, 1.0, 0.0), ALT, 1);
    assert_eq!(w.editor.selection(), &[right]);
    w.click_n(v(4.0, 4.0, 0.0), SHIFT, 1);
    assert_eq!(w.editor.selection(), &[right], "Shift on nothing keeps it");
}

#[test]
fn double_click_selects_bounded() {
    let mut w = World::new(Tool::Select);
    w.square();
    let f = w.face();
    let mut all = w.edges();
    all.push(f);
    w.click_n(v(1.0, 1.0, 0.0), NONE, 2);
    assert_eq!(w.editor.selection(), sorted(all).as_slice());
    let bottom = w.edge_at(v(1.0, 0.0, 0.0));
    w.click_n(v(1.0, 0.0, 0.0), NONE, 2);
    assert_eq!(w.editor.selection(), sorted(vec![bottom, f]).as_slice());
}

#[test]
fn triple_click_selects_connected() {
    let mut w = World::new(Tool::Select);
    w.square();
    // A separate edge that is not connected.
    w.editor.set_tool(Tool::Line);
    w.click(v(4.0, 0.0, 0.0));
    w.click(v(4.0, 2.0, 0.0));
    w.editor.set_tool(Tool::Select);
    let lone = w.edge_at(v(4.0, 1.0, 0.0));
    let mut square: Vec<Entity> = w.edges().into_iter().filter(|e| *e != lone).collect();
    square.push(w.face());
    w.click_n(v(1.0, 0.0, 0.0), NONE, 3);
    assert_eq!(w.editor.selection(), sorted(square.clone()).as_slice());
    w.click_n(v(4.0, 1.0, 0.0), SHIFT, 3);
    let mut everything = square;
    everything.push(lone);
    assert_eq!(w.editor.selection(), sorted(everything).as_slice());
}

#[test]
fn double_click_on_nothing_acts_like_a_click() {
    let mut w = World::new(Tool::Select);
    w.square();
    w.click_n(v(1.0, 1.0, 0.0), NONE, 1);
    w.click_n(v(4.0, 4.0, 0.0), NONE, 2);
    assert!(w.editor.selection().is_empty());
}

#[test]
fn window_box_selects_what_is_inside() {
    let mut w = World::new(Tool::Select);
    w.square();
    let mut all = w.edges();
    all.push(w.face());
    // Screen corners of the world box (-1,-1)..(3,3), dragged so x increases.
    let a = w.px(v(-1.0, -1.0, 0.0));
    let b = w.px(v(3.0, 3.0, 0.0));
    let (start, end) = (a.min(b), a.max(b));
    w.editor.box_select(&w.model, &w.view, start, end, NONE);
    assert_eq!(w.editor.selection(), sorted(all).as_slice());
}

#[test]
fn window_box_partly_over_selects_only_whole_entities() {
    let mut w = World::new(Tool::Select);
    w.square();
    let a = w.px(v(1.5, -1.0, 0.0));
    let b = w.px(v(3.0, 3.0, 0.0));
    let (start, end) = (a.min(b), a.max(b));
    w.editor.box_select(&w.model, &w.view, start, end, NONE);
    assert_eq!(
        w.editor.selection(),
        &[w.edge_at(v(2.0, 1.0, 0.0))],
        "only the right edge lies inside"
    );
}

#[test]
fn crossing_box_selects_what_it_touches() {
    let mut w = World::new(Tool::Select);
    w.square();
    let a = w.px(v(1.5, -1.0, 0.0));
    let b = w.px(v(3.0, 3.0, 0.0));
    // Dragged so x decreases: crossing.
    let start = DVec2::new(a.x.max(b.x), a.y.min(b.y));
    let end = DVec2::new(a.x.min(b.x), a.y.max(b.y));
    w.editor.box_select(&w.model, &w.view, start, end, NONE);
    let left = w.edge_at(v(0.0, 1.0, 0.0));
    let mut touched: Vec<Entity> = w.edges().into_iter().filter(|e| *e != left).collect();
    touched.push(w.face());
    assert_eq!(w.editor.selection(), sorted(touched).as_slice());
}

#[test]
fn box_select_uses_modifiers() {
    let mut w = World::new(Tool::Select);
    w.square();
    let f = w.face();
    w.click_n(v(1.0, 1.0, 0.0), NONE, 1);
    let a = w.px(v(1.5, -1.0, 0.0));
    let b = w.px(v(3.0, 3.0, 0.0));
    let (start, end) = (a.min(b), a.max(b));
    w.editor.box_select(&w.model, &w.view, start, end, SHIFT);
    assert_eq!(
        w.editor.selection(),
        sorted(vec![f, w.edge_at(v(2.0, 1.0, 0.0))]).as_slice()
    );
    w.editor.box_select(&w.model, &w.view, start, end, ALT);
    assert_eq!(w.editor.selection(), &[f]);
}

#[test]
fn box_select_needs_the_select_tool() {
    let mut w = World::new(Tool::Line);
    w.square();
    w.editor.set_tool(Tool::Line);
    let a = w.px(v(-1.0, -1.0, 0.0));
    let b = w.px(v(3.0, 3.0, 0.0));
    w.editor
        .box_select(&w.model, &w.view, a.min(b), a.max(b), NONE);
    assert!(w.editor.selection().is_empty());
}

#[test]
fn drawing_through_a_selected_edge_prunes_it() {
    let mut w = World::new(Tool::Line);
    w.click(v(0.0, 0.0, 0.0));
    w.click(v(2.0, 0.0, 0.0));
    w.editor.set_tool(Tool::Select);
    w.click_n(v(1.0, 0.0, 0.0), NONE, 1);
    assert_eq!(w.editor.selection().len(), 1);
    // A line crossing the selected edge splits it: the old edge is gone.
    w.editor.set_tool(Tool::Line);
    w.click(v(1.0, -1.0, 0.0));
    assert!(w.click(v(1.0, 1.0, 0.0)));
    for e in w.editor.selection() {
        let Entity::Edge(id) = e else { panic!("{e:?}") };
        assert!(w.ctx().edge(*id).is_some(), "stale {e:?}");
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
