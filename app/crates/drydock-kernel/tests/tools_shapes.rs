// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Rectangle, circle and arc tools (`docs/specs/m2-tool-shapes.md`).

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use drydock_kernel::model::*;
use drydock_kernel::sticky::StickyError;
use drydock_kernel::tools::arc::ArcTool;
use drydock_kernel::tools::circle::CircleTool;
use drydock_kernel::tools::plane::DrawPlane;
use drydock_kernel::tools::rectangle::RectTool;
use drydock_kernel::tools::*;
use drydock_kernel::units::{ParseError, Unit};
use glam::DVec3;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn assert_close(a: DVec3, b: DVec3) {
    assert!(a.distance(b) < 1e-9, "{a:?} != {b:?}");
}

fn xy() -> DrawPlane {
    DrawPlane {
        u: DVec3::X,
        v: DVec3::Y,
    }
}

struct World {
    model: Model,
    undo: UndoStack,
}

impl World {
    fn new() -> Self {
        World {
            model: Model::new(),
            undo: UndoStack::new(),
        }
    }

    fn target(&mut self) -> Target<'_> {
        Target {
            model: &mut self.model,
            body: BodyId::Root,
            undo: &mut self.undo,
            unit: Unit::Meter,
        }
    }

    fn counts(&self) -> (usize, usize, usize) {
        let c = self.model.body(BodyId::Root).unwrap().geometry();
        (c.vertices().count(), c.edges().count(), c.faces().count())
    }

    fn vertex_positions(&self) -> Vec<DVec3> {
        let c = self.model.body(BodyId::Root).unwrap().geometry();
        c.vertices().map(|(_, vx)| vx.position).collect()
    }

    fn depth(&self) -> usize {
        let mut u = self.undo.clone();
        let mut m = self.model.clone();
        let mut n = 0;
        while u.undo(&mut m).unwrap().is_some() {
            n += 1;
        }
        n
    }

    fn rect(&mut self, t: &mut RectTool, i: Input) -> Result<(), ToolError> {
        let mut tg = self.target();
        t.handle(i, &mut tg)
    }

    fn circ(&mut self, t: &mut CircleTool, i: Input) -> Result<(), ToolError> {
        let mut tg = self.target();
        t.handle(i, &mut tg)
    }

    fn arc(&mut self, t: &mut ArcTool, i: Input) -> Result<(), ToolError> {
        let mut tg = self.target();
        t.handle(i, &mut tg)
    }
}

fn typed(s: &str) -> Input {
    Input::Typed(s.to_string())
}

// ---------- DrawPlane ----------

#[test]
fn plane_normal_and_project() {
    let p = xy();
    assert_close(p.normal(), DVec3::Z);
    assert_close(
        p.project(v(1.0, 1.0, 1.0), v(4.0, 5.0, 9.0)),
        v(4.0, 5.0, 1.0),
    );
}

// ---------- rectangle ----------

#[test]
fn rect_two_clicks_make_a_face() {
    let mut w = World::new();
    let mut t = RectTool::new(xy());
    assert_eq!(t.anchor(), None);
    assert_eq!(t.preview(), None);
    w.rect(&mut t, Input::Click(v(1.0, 1.0, 0.0))).unwrap();
    assert_eq!(t.anchor(), Some(v(1.0, 1.0, 0.0)));
    assert_eq!(w.counts(), (0, 0, 0));
    w.rect(&mut t, Input::Move(v(4.0, 3.0, 0.0))).unwrap();
    let pv = t.preview().unwrap();
    assert_close(pv[2], v(4.0, 3.0, 0.0));
    let (sw, sh) = t.size().unwrap();
    assert!(near(sw, 3.0) && near(sh, 2.0));
    w.rect(&mut t, Input::Click(v(4.0, 3.0, 0.0))).unwrap();
    assert_eq!(w.counts(), (4, 4, 1));
    assert_eq!(w.depth(), 1);
    assert_eq!(t.anchor(), None);
    assert_eq!(t.preview(), None);
    assert_eq!(t.size(), None);
}

#[test]
fn rect_drops_the_normal_component_of_the_second_click() {
    let mut w = World::new();
    let mut t = RectTool::new(xy());
    w.rect(&mut t, Input::Click(v(0.0, 0.0, 2.0))).unwrap();
    w.rect(&mut t, Input::Click(v(2.0, 1.0, 7.0))).unwrap();
    assert_eq!(w.counts(), (4, 4, 1));
    for p in w.vertex_positions() {
        assert!(near(p.z, 2.0));
    }
}

#[test]
fn rect_works_in_a_vertical_plane() {
    let mut w = World::new();
    let mut t = RectTool::new(DrawPlane {
        u: DVec3::X,
        v: DVec3::Z,
    });
    w.rect(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.rect(&mut t, Input::Click(v(2.0, 5.0, 3.0))).unwrap();
    assert_eq!(w.counts(), (4, 4, 1));
    for p in w.vertex_positions() {
        assert!(near(p.y, 0.0));
    }
}

#[test]
fn rect_negative_quadrant_and_thin_click() {
    let mut w = World::new();
    let mut t = RectTool::new(xy());
    w.rect(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.rect(&mut t, Input::Move(v(-2.0, 0.0, 0.0))).unwrap();
    assert_eq!(t.preview(), None);
    assert_eq!(t.size(), None);
    // zero height: ignored, still drawing
    w.rect(&mut t, Input::Click(v(-2.0, 0.0, 0.0))).unwrap();
    assert_eq!(w.counts(), (0, 0, 0));
    assert_eq!(t.anchor(), Some(DVec3::ZERO));
    w.rect(&mut t, Input::Click(v(-2.0, -1.0, 0.0))).unwrap();
    assert_eq!(w.counts(), (4, 4, 1));
}

#[test]
fn rect_typed_width_and_height_follow_the_cursor_quadrant() {
    let mut w = World::new();
    let mut t = RectTool::new(xy());
    w.rect(&mut t, Input::Click(v(1.0, 1.0, 0.0))).unwrap();
    w.rect(&mut t, Input::Move(v(0.0, 3.0, 0.0))).unwrap(); // du < 0, dv > 0
    w.rect(&mut t, typed("3,2")).unwrap();
    assert_eq!(w.counts(), (4, 4, 1));
    let ps = w.vertex_positions();
    let has = |p: DVec3| ps.iter().any(|q| q.distance(p) < 1e-9);
    assert!(has(v(1.0, 1.0, 0.0)) && has(v(-2.0, 3.0, 0.0)));
    assert!(has(v(-2.0, 1.0, 0.0)) && has(v(1.0, 3.0, 0.0)));
}

#[test]
fn rect_typed_negative_flips_a_side_and_units_apply() {
    let mut w = World::new();
    let mut t = RectTool::new(xy());
    w.rect(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    w.rect(&mut t, Input::Move(v(1.0, 1.0, 0.0))).unwrap();
    w.rect(&mut t, typed("2m,-500mm")).unwrap();
    let ps = w.vertex_positions();
    let has = |p: DVec3| ps.iter().any(|q| q.distance(p) < 1e-9);
    assert!(has(v(2.0, -0.5, 0.0)) && has(v(0.0, -0.5, 0.0)));
}

#[test]
fn rect_typed_one_value_keeps_the_cursor_height() {
    let mut w = World::new();
    let mut t = RectTool::new(xy());
    w.rect(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    // no cursor yet: nothing to take the height from
    assert_eq!(w.rect(&mut t, typed("2")), Err(ToolError::NoDirection));
    w.rect(&mut t, Input::Move(v(0.5, -1.5, 0.0))).unwrap();
    w.rect(&mut t, typed("2")).unwrap();
    let ps = w.vertex_positions();
    let has = |p: DVec3| ps.iter().any(|q| q.distance(p) < 1e-9);
    assert!(has(v(2.0, -1.5, 0.0)));
}

#[test]
fn rect_typed_errors_change_nothing() {
    let mut w = World::new();
    let mut t = RectTool::new(xy());
    assert_eq!(w.rect(&mut t, typed("1,2")), Err(ToolError::NotStarted));
    w.rect(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    w.rect(&mut t, Input::Move(v(1.0, 1.0, 0.0))).unwrap();
    assert_eq!(
        w.rect(&mut t, typed("abc")),
        Err(ToolError::Parse(ParseError::Invalid))
    );
    assert_eq!(
        w.rect(&mut t, typed("1,2,3")),
        Err(ToolError::Parse(ParseError::Invalid))
    );
    assert_eq!(w.rect(&mut t, typed("0,2")), Err(ToolError::NoDirection));
    assert_eq!(w.counts(), (0, 0, 0));
    assert_eq!(w.depth(), 0);
    assert_eq!(t.anchor(), Some(DVec3::ZERO));
    assert!(t.preview().is_some());
}

#[test]
fn rect_cancel_and_nonfinite() {
    let mut w = World::new();
    let mut t = RectTool::new(xy());
    assert_eq!(
        w.rect(&mut t, Input::Click(v(f64::NAN, 0.0, 0.0))),
        Err(ToolError::Geometry(StickyError::NotFinite))
    );
    w.rect(&mut t, Input::Move(v(1.0, 1.0, 0.0))).unwrap(); // idle: ignored
    w.rect(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    assert_eq!(t.preview(), None);
    w.rect(&mut t, Input::Move(v(f64::INFINITY, 0.0, 0.0)))
        .unwrap();
    assert_eq!(t.preview(), None);
    w.rect(&mut t, Input::Cancel).unwrap();
    assert_eq!(t.anchor(), None);
    assert_eq!(w.depth(), 0);
}

#[test]
fn rect_splits_an_existing_face() {
    let mut w = World::new();
    let mut t = RectTool::new(xy());
    w.rect(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    w.rect(&mut t, Input::Click(v(4.0, 4.0, 0.0))).unwrap();
    w.rect(&mut t, Input::Click(v(1.0, 1.0, 0.0))).unwrap();
    w.rect(&mut t, Input::Click(v(2.0, 2.0, 0.0))).unwrap();
    assert_eq!(w.counts(), (8, 8, 2)); // the square with a hole, and the inner face
    assert_eq!(w.depth(), 2);
}

// ---------- circle ----------

#[test]
fn circle_click_center_and_rim() {
    let mut w = World::new();
    let mut t = CircleTool::new(xy());
    assert_eq!(t.segments(), 24);
    w.circ(&mut t, Input::Click(v(1.0, 1.0, 0.0))).unwrap();
    assert_eq!(t.anchor(), Some(v(1.0, 1.0, 0.0)));
    assert_eq!(t.preview(), None);
    w.circ(&mut t, Input::Move(v(3.0, 1.0, 5.0))).unwrap();
    assert!(near(t.radius().unwrap(), 2.0));
    let pv = t.preview().unwrap();
    assert_eq!(pv.len(), 24);
    assert_close(pv[0], v(3.0, 1.0, 0.0));
    w.circ(&mut t, Input::Click(v(3.0, 1.0, 5.0))).unwrap();
    assert_eq!(w.counts(), (24, 24, 1));
    assert_eq!(w.depth(), 1);
    assert_eq!(t.anchor(), None);
    for p in w.vertex_positions() {
        assert!(near(p.distance(v(1.0, 1.0, 0.0)), 2.0));
        assert!(near(p.z, 0.0));
    }
}

#[test]
fn circle_click_on_center_is_ignored() {
    let mut w = World::new();
    let mut t = CircleTool::new(xy());
    w.circ(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    w.circ(&mut t, Input::Click(v(0.0, 0.0, 3.0))).unwrap();
    assert_eq!(w.counts(), (0, 0, 0));
    assert_eq!(t.anchor(), Some(DVec3::ZERO));
}

#[test]
fn circle_segment_count() {
    let mut w = World::new();
    let mut t = CircleTool::new(xy());
    w.circ(&mut t, typed("8s")).unwrap(); // idle is fine
    assert_eq!(t.segments(), 8);
    w.circ(&mut t, typed(" 12 S ")).unwrap();
    assert_eq!(t.segments(), 12);
    for bad in ["2s", "361s", "xs", "s", "-4s", "1.5s"] {
        assert_eq!(
            w.circ(&mut t, typed(bad)),
            Err(ToolError::Parse(ParseError::Invalid)),
            "{bad}"
        );
        assert_eq!(t.segments(), 12);
    }
    w.circ(&mut t, typed("360s")).unwrap();
    w.circ(&mut t, typed("6s")).unwrap();
    w.circ(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    w.circ(&mut t, typed("4s")).unwrap(); // mid-drawing: still drawing
    assert_eq!(t.anchor(), Some(DVec3::ZERO));
    w.circ(&mut t, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    assert_eq!(w.counts(), (4, 4, 1));
    w.circ(&mut t, Input::Cancel).unwrap();
    assert_eq!(t.segments(), 4);
}

#[test]
fn circle_typed_radius_uses_cursor_direction() {
    let mut w = World::new();
    let mut t = CircleTool::new(xy());
    assert_eq!(w.circ(&mut t, typed("2")), Err(ToolError::NotStarted));
    w.circ(&mut t, typed("4s")).unwrap();
    w.circ(&mut t, Input::Click(v(1.0, 1.0, 0.0))).unwrap();
    w.circ(&mut t, Input::Move(v(1.0, 3.0, 0.0))).unwrap();
    w.circ(&mut t, typed("500mm")).unwrap();
    assert_eq!(w.counts(), (4, 4, 1));
    let ps = w.vertex_positions();
    assert!(ps.iter().any(|p| p.distance(v(1.0, 1.5, 0.0)) < 1e-9));
    assert!(ps.iter().any(|p| p.distance(v(0.5, 1.0, 0.0)) < 1e-9));
}

#[test]
fn circle_typed_without_cursor_starts_along_u() {
    let mut w = World::new();
    let mut t = CircleTool::new(xy());
    w.circ(&mut t, typed("4s")).unwrap();
    w.circ(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    w.circ(&mut t, typed("-2")).unwrap(); // sign is ignored
    let ps = w.vertex_positions();
    assert!(ps.iter().any(|p| p.distance(v(2.0, 0.0, 0.0)) < 1e-9));
}

#[test]
fn circle_typed_errors_change_nothing() {
    let mut w = World::new();
    let mut t = CircleTool::new(xy());
    w.circ(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    assert_eq!(
        w.circ(&mut t, typed("zz")),
        Err(ToolError::Parse(ParseError::Invalid))
    );
    assert_eq!(w.circ(&mut t, typed("0")), Err(ToolError::NoDirection));
    assert_eq!(w.counts(), (0, 0, 0));
    assert_eq!(t.anchor(), Some(DVec3::ZERO));
}

#[test]
fn circle_in_a_vertical_plane() {
    let mut w = World::new();
    let mut t = CircleTool::new(DrawPlane {
        u: DVec3::Y,
        v: DVec3::Z,
    });
    w.circ(&mut t, Input::Click(v(5.0, 0.0, 0.0))).unwrap();
    w.circ(&mut t, Input::Click(v(5.0, 0.0, 1.0))).unwrap();
    assert_eq!(w.counts(), (24, 24, 1));
    for p in w.vertex_positions() {
        assert!(near(p.x, 5.0));
    }
}

// ---------- arc ----------

#[test]
fn arc_three_clicks() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    assert_eq!(t.segments(), 12);
    w.arc(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    assert_eq!(t.anchor(), Some(DVec3::ZERO));
    w.arc(&mut t, Input::Move(v(2.0, 0.0, 0.0))).unwrap();
    assert!(near(t.radius().unwrap(), 2.0));
    assert_eq!(t.preview(), None);
    assert_eq!(t.sweep(), None);
    w.arc(&mut t, Input::Click(v(2.0, 0.0, 0.0))).unwrap();
    assert_eq!(t.anchor(), Some(v(2.0, 0.0, 0.0)));
    assert!(near(t.radius().unwrap(), 2.0));
    w.arc(&mut t, Input::Move(v(0.0, 7.0, 0.0))).unwrap();
    assert!(near(t.sweep().unwrap(), FRAC_PI_2));
    let pv = t.preview().unwrap();
    assert_eq!(pv.len(), 13);
    assert_close(pv[0], v(2.0, 0.0, 0.0));
    assert_close(pv[12], v(0.0, 2.0, 0.0));
    w.arc(&mut t, Input::Click(v(0.0, 7.0, 0.0))).unwrap();
    assert_eq!(w.counts(), (13, 12, 0));
    assert_eq!(w.depth(), 1);
    assert_eq!(t.anchor(), None);
    for p in w.vertex_positions() {
        assert!(near(p.length(), 2.0));
    }
}

#[test]
fn arc_sweep_is_counter_clockwise() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    w.arc(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    w.arc(&mut t, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    w.arc(&mut t, Input::Move(v(0.0, -1.0, 0.0))).unwrap();
    assert!(near(t.sweep().unwrap(), 1.5 * PI));
    w.arc(&mut t, Input::Move(v(1.0, 0.0, 0.0))).unwrap(); // same direction as the start
    assert_eq!(t.sweep(), None);
    assert_eq!(t.preview(), None);
    w.arc(&mut t, Input::Move(DVec3::ZERO)).unwrap(); // on the center
    assert_eq!(t.sweep(), None);
    w.arc(&mut t, Input::Click(DVec3::ZERO)).unwrap(); // ignored
    w.arc(&mut t, Input::Click(v(3.0, 0.0, 0.0))).unwrap(); // ignored
    assert_eq!(w.counts(), (0, 0, 0));
    assert_eq!(t.anchor(), Some(v(1.0, 0.0, 0.0)));
}

#[test]
fn arc_center_stage_ignores_click_on_center() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    w.arc(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    w.arc(&mut t, Input::Click(v(0.0, 0.0, 4.0))).unwrap();
    assert_eq!(t.anchor(), Some(DVec3::ZERO));
}

#[test]
fn arc_segments() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    w.arc(&mut t, typed("4s")).unwrap();
    assert_eq!(t.segments(), 4);
    w.arc(&mut t, typed("1s")).unwrap();
    assert_eq!(t.segments(), 1);
    for bad in ["0s", "361s", "qs"] {
        assert_eq!(
            w.arc(&mut t, typed(bad)),
            Err(ToolError::Parse(ParseError::Invalid))
        );
    }
    assert_eq!(t.segments(), 1);
    w.arc(&mut t, typed("4s")).unwrap();
    w.arc(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    w.arc(&mut t, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    w.arc(&mut t, Input::Click(v(0.0, 1.0, 0.0))).unwrap();
    assert_eq!(w.counts(), (5, 4, 0));
}

#[test]
fn arc_typed_radius_then_angle() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    assert_eq!(w.arc(&mut t, typed("1")), Err(ToolError::NotStarted));
    w.arc(&mut t, typed("4s")).unwrap();
    w.arc(&mut t, Input::Click(v(1.0, 1.0, 0.0))).unwrap();
    w.arc(&mut t, Input::Move(v(1.0, 5.0, 0.0))).unwrap();
    w.arc(&mut t, typed("2")).unwrap();
    assert_eq!(w.counts(), (0, 0, 0));
    assert_close(t.anchor().unwrap(), v(1.0, 3.0, 0.0));
    // clockwise quarter turn from (1, 3): ends at (3, 1)
    w.arc(&mut t, typed("-90")).unwrap();
    assert_eq!(w.counts(), (5, 4, 0));
    let ps = w.vertex_positions();
    assert!(ps.iter().any(|p| p.distance(v(3.0, 1.0, 0.0)) < 1e-9));
    assert!(ps.iter().any(|p| p.distance(v(1.0, 3.0, 0.0)) < 1e-9));
    assert_eq!(t.anchor(), None);
}

#[test]
fn arc_typed_angle_with_degree_sign_and_full_turn() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    w.arc(&mut t, typed("4s")).unwrap();
    w.arc(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    w.arc(&mut t, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    w.arc(&mut t, typed("180°")).unwrap();
    assert_eq!(w.counts(), (5, 4, 0));
    assert!(
        w.vertex_positions()
            .iter()
            .any(|p| p.distance(v(-1.0, 0.0, 0.0)) < 1e-9)
    );

    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    w.arc(&mut t, typed("4s")).unwrap();
    w.arc(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    w.arc(&mut t, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    w.arc(&mut t, typed("360")).unwrap();
    assert_eq!(w.counts(), (4, 4, 1)); // closes into a face
}

#[test]
fn arc_typed_errors_change_nothing() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    w.arc(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    assert_eq!(
        w.arc(&mut t, typed("x")),
        Err(ToolError::Parse(ParseError::Invalid))
    );
    assert_eq!(w.arc(&mut t, typed("0")), Err(ToolError::NoDirection));
    w.arc(&mut t, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    for bad in ["x", "0", "361", "-400", "nan"] {
        assert_eq!(
            w.arc(&mut t, typed(bad)),
            Err(ToolError::Parse(ParseError::Invalid)),
            "{bad}"
        );
    }
    assert_eq!(w.counts(), (0, 0, 0));
    assert_eq!(w.depth(), 0);
    assert_eq!(t.anchor(), Some(v(1.0, 0.0, 0.0)));
    w.arc(&mut t, Input::Cancel).unwrap();
    assert_eq!(t.anchor(), None);
}

#[test]
fn arc_nonfinite_input() {
    let mut w = World::new();
    let mut t = ArcTool::new(xy());
    assert_eq!(
        w.arc(&mut t, Input::Click(v(0.0, f64::NAN, 0.0))),
        Err(ToolError::Geometry(StickyError::NotFinite))
    );
    w.arc(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    w.arc(&mut t, Input::Move(v(f64::NAN, 0.0, 0.0))).unwrap();
    assert_eq!(t.radius(), None);
    let _ = TAU;
}

#[test]
fn arc_endpoint_on_existing_edge_joins_it() {
    let mut w = World::new();
    let mut r = RectTool::new(xy());
    w.rect(&mut r, Input::Click(DVec3::ZERO)).unwrap();
    w.rect(&mut r, Input::Click(v(2.0, 2.0, 0.0))).unwrap();
    assert_eq!(w.counts(), (4, 4, 1));
    // quarter arc from (1, 0) around (0, 0) to (0, 1): inside the square, on its edges
    let mut t = ArcTool::new(xy());
    w.arc(&mut t, typed("2s")).unwrap();
    w.arc(&mut t, Input::Click(DVec3::ZERO)).unwrap();
    w.arc(&mut t, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    w.arc(&mut t, Input::Click(v(0.0, 1.0, 0.0))).unwrap();
    // the arc closes against the square's edges: it splits the face
    assert_eq!(w.counts().2, 2);
    assert_eq!(w.depth(), 2);
}
