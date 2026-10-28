// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Rectangle, circle and arc tools in the editor (`docs/specs/m2-app-shapes.md`).

use glam::{DVec2, DVec3};
use modeler::editor::*;
use modeler::input::{EditKey, Modifiers, Tool};
use modeler_kernel::model::{BodyId, Model, UndoStack};
use modeler_kernel::tools::plane::DrawPlane;
use modeler_kernel::tools::{Target, draw_segments};
use modeler_kernel::topology::Context;
use modeler_kernel::units::{Unit, format_length};
use modeler_view::{Bounds, Camera, Projection, StandardView};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

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

const GROUND: DrawPlane = DrawPlane {
    u: DVec3::X,
    v: DVec3::Y,
};

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

// ---------------------------------------------------------------- plane_for_normal

#[test]
fn plane_for_normal_ground_is_x_y() {
    let p = plane_for_normal(DVec3::Z);
    assert_close(p.u, DVec3::X);
    assert_close(p.v, DVec3::Y);
}

#[test]
fn plane_for_normal_down_is_x_minus_y() {
    let p = plane_for_normal(-DVec3::Z);
    assert_close(p.u, DVec3::X);
    assert_close(p.v, -DVec3::Y);
}

#[test]
fn plane_for_normal_wall_facing_y() {
    let p = plane_for_normal(DVec3::Y);
    assert_close(p.u, -DVec3::X);
    assert_close(p.v, DVec3::Z);
}

#[test]
fn plane_for_normal_wall_facing_x() {
    let p = plane_for_normal(DVec3::X);
    assert_close(p.u, DVec3::Y);
    assert_close(p.v, DVec3::Z);
}

// ---------------------------------------------------------------- tools and planes

#[test]
fn shape_tools_start_on_the_ground_plane() {
    for tool in [Tool::Rectangle, Tool::Circle, Tool::Arc] {
        let w = World::new(tool);
        let p = w.editor.draw_plane().expect("draw plane");
        assert_close(p.u, GROUND.u);
        assert_close(p.v, GROUND.v);
        assert_eq!(w.editor.plane_lock(), None);
        assert_eq!(w.editor.lock(), None);
        assert!(w.editor.preview().is_empty());
    }
}

#[test]
fn other_tools_have_no_draw_plane() {
    for tool in [Tool::Orbit, Tool::Select, Tool::Line] {
        let w = World::new(tool);
        assert_eq!(w.editor.draw_plane(), None);
        assert_eq!(w.editor.plane_lock(), None);
    }
}

#[test]
fn ground_in_top_view_faces_up() {
    let mut w = World::new(Tool::Rectangle);
    w.move_to(v(1.0, 1.0, 0.0));
    let p = w.editor.draw_plane().unwrap();
    assert_close(p.u, DVec3::X);
    assert_close(p.v, DVec3::Y);
}

// ---------------------------------------------------------------- rectangle

#[test]
fn rectangle_click_click_draws_a_face() {
    let mut w = World::new(Tool::Rectangle);
    assert_eq!(w.measurement(), ("Dimensions", String::new()));
    assert!(!w.click(v(0.0, 0.0, 0.0)));
    w.move_to(v(3.0, 2.0, 0.0));
    let preview = w.editor.preview();
    assert_eq!(preview.len(), 4);
    for i in 0..4 {
        assert_close(preview[i].1, preview[(i + 1) % 4].0);
    }
    assert_eq!(
        w.measurement(),
        ("Dimensions", format!("{}, {}", f(3.0), f(2.0)))
    );
    assert!(w.click(v(3.0, 2.0, 0.0)));
    assert_eq!(w.ctx().edge_count(), 4);
    assert_eq!(w.ctx().face_count(), 1);
    assert_eq!(w.editor.undo_stack().undo_count(), 1);
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Rectangle"));
    assert!(w.has_vertex(v(3.0, 0.0, 0.0)));
    assert!(w.has_vertex(v(0.0, 2.0, 0.0)));
    assert!(w.editor.preview().is_empty());
    assert_eq!(w.measurement(), ("Dimensions", String::new()));
    assert!(w.model.validate().is_empty());
}

#[test]
fn rectangle_typed_dimensions_follow_the_cursor_side() {
    let mut w = World::new(Tool::Rectangle);
    w.click(v(0.0, 0.0, 0.0));
    w.move_to(v(-1.0, 1.0, 0.0));
    assert!(w.enter("3,2"));
    assert_eq!(w.editor.typed(), "");
    assert!(w.has_vertex(v(-3.0, 2.0, 0.0)));
    assert_eq!(w.ctx().face_count(), 1);
    assert_eq!(w.editor.message(), None);
}

#[test]
fn rectangle_typed_while_idle_sets_a_message() {
    let mut w = World::new(Tool::Rectangle);
    assert!(!w.enter("3,2"));
    assert!(w.editor.message().is_some());
    assert_eq!(w.ctx().edge_count(), 0);
}

#[test]
fn rectangle_too_thin_click_does_nothing() {
    let mut w = World::new(Tool::Rectangle);
    w.click(v(0.0, 0.0, 0.0));
    assert!(!w.click(v(3.0, 0.0, 0.0)));
    assert_eq!(w.ctx().edge_count(), 0);
    // Still drawing from the first corner.
    assert!(w.click(v(3.0, 2.0, 0.0)));
}

#[test]
fn escape_drops_the_shape_in_progress() {
    let mut w = World::new(Tool::Rectangle);
    w.click(v(0.0, 0.0, 0.0));
    w.move_to(v(3.0, 2.0, 0.0));
    assert!(!w.key(EditKey::Cancel));
    assert!(w.editor.preview().is_empty());
    // The next click starts a new rectangle.
    assert!(!w.click(v(1.0, 1.0, 0.0)));
    assert!(w.click(v(2.0, 3.0, 0.0)));
    assert!(w.has_vertex(v(1.0, 1.0, 0.0)));
    assert!(!w.has_vertex(v(0.0, 0.0, 0.0)));
}

#[test]
fn undo_and_redo_the_rectangle() {
    let mut w = World::new(Tool::Rectangle);
    w.click(v(0.0, 0.0, 0.0));
    w.click(v(3.0, 2.0, 0.0));
    assert!(w.key(EditKey::Undo));
    assert_eq!(w.ctx().edge_count(), 0);
    assert!(w.key(EditKey::Redo));
    assert_eq!(w.ctx().edge_count(), 4);
}

#[test]
fn undo_drops_the_shape_in_progress() {
    let mut w = World::new(Tool::Rectangle);
    w.click(v(0.0, 0.0, 0.0));
    w.click(v(3.0, 2.0, 0.0));
    w.click(v(5.0, 5.0, 0.0));
    assert!(w.key(EditKey::Undo));
    assert!(w.editor.preview().is_empty());
    w.move_to(v(4.0, 4.0, 0.0));
    assert!(w.editor.preview().is_empty());
}

#[test]
fn set_tool_drops_the_shape_in_progress() {
    let mut w = World::new(Tool::Rectangle);
    w.click(v(0.0, 0.0, 0.0));
    w.editor.set_tool(Tool::Rectangle);
    w.move_to(v(3.0, 2.0, 0.0));
    assert!(w.editor.preview().is_empty());
}

#[test]
fn text_is_taken_by_every_drawing_tool() {
    for tool in [Tool::Line, Tool::Rectangle, Tool::Circle, Tool::Arc] {
        let mut w = World::new(tool);
        assert!(w.editor.text("8"), "{tool:?}");
        assert!(w.editor.text("s"), "{tool:?}");
        assert_eq!(w.editor.typed(), "8s");
    }
    let mut w = World::new(Tool::Select);
    assert!(!w.editor.text("8"));
}

// ---------------------------------------------------------------- circle

#[test]
fn circle_segments_typed_while_idle() {
    let mut w = World::new(Tool::Circle);
    assert_eq!(w.measurement(), ("Segments", "24".to_string()));
    assert!(!w.enter("8s"));
    assert_eq!(w.measurement(), ("Segments", "8".to_string()));
    assert_eq!(w.editor.message(), None);
    assert_eq!(w.editor.typed(), "");
}

#[test]
fn circle_bad_segment_count_sets_a_message() {
    let mut w = World::new(Tool::Circle);
    assert!(!w.enter("2s"));
    assert!(w.editor.message().is_some());
    assert_eq!(w.measurement(), ("Segments", "24".to_string()));
}

#[test]
fn circle_click_click_draws_a_face() {
    let mut w = World::new(Tool::Circle);
    w.enter("8s");
    assert!(!w.click(v(0.0, 0.0, 0.0)));
    assert_eq!(w.measurement(), ("Radius", String::new()));
    w.move_to(v(2.0, 0.0, 0.0));
    assert_eq!(w.measurement(), ("Radius", f(2.0)));
    let preview = w.editor.preview();
    assert_eq!(preview.len(), 8);
    assert_close(preview[0].0, v(2.0, 0.0, 0.0));
    assert_close(preview[7].1, v(2.0, 0.0, 0.0));
    assert!(w.click(v(2.0, 0.0, 0.0)));
    assert_eq!(w.ctx().edge_count(), 8);
    assert_eq!(w.ctx().face_count(), 1);
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Circle"));
    for p in w.vertices() {
        assert!((p.length() - 2.0).abs() < 1e-6, "{p:?}");
    }
    assert_eq!(w.measurement(), ("Segments", "8".to_string()));
}

#[test]
fn circle_typed_radius() {
    let mut w = World::new(Tool::Circle);
    w.click(v(0.0, 0.0, 0.0));
    w.move_to(v(1.0, 0.0, 0.0));
    assert!(w.enter("1.5"));
    assert_eq!(w.ctx().edge_count(), 24);
    assert!(w.has_vertex(v(1.5, 0.0, 0.0)));
}

#[test]
fn segment_counts_survive_switching_tools() {
    let mut w = World::new(Tool::Circle);
    w.enter("8s");
    w.editor.set_tool(Tool::Arc);
    assert_eq!(w.measurement(), ("Segments", "12".to_string()));
    w.enter("5s");
    w.editor.set_tool(Tool::Line);
    w.editor.set_tool(Tool::Circle);
    assert_eq!(w.measurement(), ("Segments", "8".to_string()));
    w.editor.set_tool(Tool::Arc);
    assert_eq!(w.measurement(), ("Segments", "5".to_string()));
}

// ---------------------------------------------------------------- arc

#[test]
fn arc_three_clicks() {
    let mut w = World::new(Tool::Arc);
    assert_eq!(w.measurement(), ("Segments", "12".to_string()));
    assert!(!w.click(v(0.0, 0.0, 0.0)));
    w.move_to(v(2.0, 0.0, 0.0));
    assert_eq!(w.measurement(), ("Radius", f(2.0)));
    assert!(w.editor.preview().is_empty());
    assert!(!w.click(v(2.0, 0.0, 0.0)));
    w.move_to(v(0.0, 2.0, 0.0));
    assert_eq!(w.measurement(), ("Angle", "90.0°".to_string()));
    let preview = w.editor.preview();
    assert_eq!(preview.len(), 12);
    assert_close(preview[0].0, v(2.0, 0.0, 0.0));
    assert_close(preview[11].1, v(0.0, 2.0, 0.0));
    assert!(w.click(v(0.0, 2.0, 0.0)));
    assert_eq!(w.ctx().edge_count(), 12);
    assert_eq!(w.ctx().face_count(), 0);
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Arc"));
    assert!(w.has_vertex(v(2.0, 0.0, 0.0)));
    assert!(w.has_vertex(v(0.0, 2.0, 0.0)));
    assert_eq!(w.measurement(), ("Segments", "12".to_string()));
}

#[test]
fn arc_typed_radius_then_angle() {
    let mut w = World::new(Tool::Arc);
    w.click(v(0.0, 0.0, 0.0));
    w.move_to(v(1.0, 0.0, 0.0));
    assert!(!w.enter("2"));
    assert_eq!(w.measurement(), ("Angle", String::new()));
    assert!(w.enter("180"));
    assert_eq!(w.ctx().edge_count(), 12);
    assert!(w.has_vertex(v(2.0, 0.0, 0.0)));
    assert!(w.has_vertex(v(0.0, 2.0, 0.0)));
    assert!(w.has_vertex(v(-2.0, 0.0, 0.0)));
}

#[test]
fn arc_zero_degrees_is_refused() {
    let mut w = World::new(Tool::Arc);
    w.click(v(0.0, 0.0, 0.0));
    w.click(v(2.0, 0.0, 0.0));
    assert!(!w.enter("0"));
    assert!(w.editor.message().is_some());
    assert_eq!(w.measurement().0, "Angle");
    assert!(w.enter("-90"));
    assert!(w.has_vertex(v(0.0, -2.0, 0.0)));
    assert_eq!(w.editor.message(), None);
}

// ---------------------------------------------------------------- plane lock

#[test]
fn plane_lock_toggles_while_idle() {
    let mut w = World::new(Tool::Rectangle);
    assert!(!w.key(EditKey::Lock(0)));
    assert_eq!(w.editor.plane_lock(), Some(0));
    assert_eq!(w.editor.lock(), None);
    w.move_to(v(1.0, 1.0, 0.0));
    let n = w.editor.draw_plane().unwrap().normal();
    assert!(n.x.abs() > 0.999, "{n:?}");
    w.key(EditKey::Lock(1));
    assert_eq!(w.editor.plane_lock(), Some(1));
    w.key(EditKey::Lock(1));
    assert_eq!(w.editor.plane_lock(), None);
    w.move_to(v(1.0, 1.0, 0.0));
    assert_close(w.editor.draw_plane().unwrap().normal(), DVec3::Z);
    w.key(EditKey::Lock(3));
    assert_eq!(w.editor.plane_lock(), None);
}

#[test]
fn plane_lock_is_ignored_while_drawing() {
    let mut w = World::new(Tool::Circle);
    w.click(v(0.0, 0.0, 0.0));
    w.key(EditKey::Lock(0));
    assert_eq!(w.editor.plane_lock(), None);
    w.move_to(v(1.0, 0.0, 0.0));
    assert_close(w.editor.draw_plane().unwrap().normal(), DVec3::Z);
}

#[test]
fn plane_lock_stays_after_a_shape_and_clears_on_cancel_and_set_tool() {
    let mut w = World::new(Tool::Rectangle);
    w.key(EditKey::Lock(2));
    w.click(v(0.0, 0.0, 0.0));
    assert!(w.click(v(3.0, 2.0, 0.0)));
    assert_eq!(w.editor.plane_lock(), Some(2));
    w.key(EditKey::Cancel);
    assert_eq!(w.editor.plane_lock(), None);
    w.key(EditKey::Lock(2));
    w.editor.set_tool(Tool::Circle);
    assert_eq!(w.editor.plane_lock(), None);
}

#[test]
fn plane_lock_is_not_kept_for_the_line_tool() {
    let mut w = World::new(Tool::Rectangle);
    w.key(EditKey::Lock(0));
    w.editor.set_tool(Tool::Line);
    assert_eq!(w.editor.plane_lock(), None);
    assert_eq!(w.editor.lock(), None);
    w.key(EditKey::Lock(0));
    assert_eq!(w.editor.lock(), Some(DVec3::X));
    assert_eq!(w.editor.plane_lock(), None);
}

// ---------------------------------------------------------------- drawing on faces

#[test]
fn draw_plane_follows_the_face_under_the_cursor() {
    let mut w = World::with_view(Tool::Rectangle, StandardView::Front);
    w.wall();
    let at = v(1.0, 0.0, 1.0);
    w.move_to(at);
    let n = w.editor.draw_plane().unwrap().normal();
    assert!(n.y.abs() > 1.0 - 1e-6, "{n:?}");
    let (_, dir) = w.view.ray(w.px(at));
    assert!(
        n.dot(dir) < 0.0,
        "normal {n:?} faces away from the viewer {dir:?}"
    );
}

#[test]
fn rectangle_on_a_wall_splits_it() {
    let mut w = World::with_view(Tool::Rectangle, StandardView::Front);
    w.wall();
    assert!(!w.click(v(0.5, 0.0, 0.5)));
    let plane = w.editor.draw_plane().unwrap();
    w.move_to(v(1.5, 0.0, 1.5));
    assert_eq!(w.editor.draw_plane(), Some(plane));
    assert_eq!(
        w.measurement(),
        ("Dimensions", format!("{}, {}", f(1.0), f(1.0)))
    );
    assert!(w.click(v(1.5, 0.0, 1.5)));
    assert_eq!(w.ctx().edge_count(), 8);
    assert_eq!(w.ctx().face_count(), 2);
    for p in w.vertices() {
        assert!(p.y.abs() < 1e-6, "{p:?}");
    }
    assert!(w.model.validate().is_empty());
}

#[test]
fn circle_on_a_wall_in_iso_view() {
    let mut w = World::with_view(Tool::Circle, StandardView::Iso);
    w.wall();
    w.click(v(1.0, 0.0, 1.0));
    assert!(w.click(v(1.5, 0.0, 1.0)));
    assert_eq!(w.ctx().face_count(), 2);
    for p in w.vertices() {
        assert!(p.y.abs() < 1e-6, "{p:?}");
    }
}

#[test]
fn off_the_face_the_point_stays_in_the_draw_plane() {
    // The cursor beyond the wall's edge sees the ground behind it; the rectangle still ends
    // where the ray meets the wall's plane.
    let mut w = World::with_view(Tool::Rectangle, StandardView::Iso);
    w.wall();
    w.click(v(0.5, 0.0, 0.5));
    assert!(w.click(v(3.0, 0.0, 1.0)));
    assert!(w.has_vertex(v(3.0, 0.0, 1.0)));
    assert!(w.has_vertex(v(3.0, 0.0, 0.5)));
    for p in w.vertices() {
        assert!(p.y.abs() < 1e-6, "{p:?}");
    }
}

/// Typed numbers and measurements in `unit`, 2 places.
fn set_unit(model: &mut Model, unit: Unit) {
    model
        .set_length_format(
            modeler_kernel::units::LengthFormat::Decimal { unit, decimals: 2 },
            &mut modeler_kernel::model::ModelEdit::new(),
        )
        .unwrap();
}
