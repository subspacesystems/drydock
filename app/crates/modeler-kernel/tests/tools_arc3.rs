// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Three-point arc tool (`docs/specs/m2-arc-3pt.md`).

use glam::DVec3;
use modeler_kernel::model::*;
use modeler_kernel::sticky::StickyError;
use modeler_kernel::tools::arc3::{Arc3Tool, three_point_arc};
use modeler_kernel::tools::plane::DrawPlane;
use modeler_kernel::tools::*;
use modeler_kernel::units::{ParseError, Unit};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
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

    fn edges(&self) -> usize {
        self.model
            .body(BodyId::Root)
            .unwrap()
            .geometry()
            .edges()
            .count()
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

    fn send(&mut self, t: &mut Arc3Tool, i: Input) -> Result<(), ToolError> {
        let mut tg = Target {
            model: &mut self.model,
            body: BodyId::Root,
            undo: &mut self.undo,
            unit: Unit::Meter,
        };
        t.handle(i, &mut tg)
    }
}

fn typed(s: &str) -> Input {
    Input::Typed(s.to_string())
}

// ---------- three_point_arc ----------

#[test]
fn half_circle_through_top() {
    let pts = three_point_arc(
        DVec3::Z,
        v(1.0, 0.0, 0.0),
        v(-1.0, 0.0, 0.0),
        v(0.0, 1.0, 0.0),
        8,
    )
    .unwrap();
    assert_eq!(pts.len(), 9);
    assert_close(pts[0], v(1.0, 0.0, 0.0));
    assert_close(pts[8], v(-1.0, 0.0, 0.0));
    assert_close(pts[4], v(0.0, 1.0, 0.0));
    for p in &pts {
        assert!((p.length() - 1.0).abs() < 1e-9);
        assert!(p.y >= -1e-9);
    }
}

#[test]
fn half_circle_through_bottom_goes_clockwise() {
    let pts = three_point_arc(
        DVec3::Z,
        v(1.0, 0.0, 0.0),
        v(-1.0, 0.0, 0.0),
        v(0.0, -1.0, 0.0),
        8,
    )
    .unwrap();
    assert_close(pts[4], v(0.0, -1.0, 0.0));
    for p in &pts {
        assert!(p.y <= 1e-9);
    }
}

#[test]
fn major_arc_when_through_is_far_around() {
    // circle of radius 1 about the origin; a at angle 0, b at 90 degrees, through at 225 degrees
    let t = 225f64.to_radians();
    let pts = three_point_arc(
        DVec3::Z,
        v(1.0, 0.0, 0.0),
        v(0.0, 1.0, 0.0),
        v(t.cos(), t.sin(), 0.0),
        30,
    )
    .unwrap();
    assert_close(pts[0], v(1.0, 0.0, 0.0));
    assert_close(pts[30], v(0.0, 1.0, 0.0));
    // the arc covers 270 degrees going clockwise from a: its middle is at -135 degrees
    let m = (-135f64).to_radians();
    assert_close(pts[15], v(m.cos(), m.sin(), 0.0));
    let step = 270f64.to_radians() / 30.0;
    let d = pts[0].distance(pts[1]);
    assert!((d - 2.0 * (step / 2.0).sin()).abs() < 1e-9);
}

#[test]
fn minor_arc_off_origin_in_a_tilted_plane() {
    // plane x = 2 (normal X); circle centre (2, 0, 0), radius 3
    let a = v(2.0, 3.0, 0.0);
    let b = v(2.0, 0.0, 3.0);
    let t = 45f64.to_radians();
    let through = v(2.0, 3.0 * t.cos(), 3.0 * t.sin());
    let pts = three_point_arc(DVec3::X, a, b, through, 6).unwrap();
    assert_eq!(pts.len(), 7);
    assert_close(pts[0], a);
    assert_close(pts[6], b);
    assert_close(pts[3], through);
    for p in &pts {
        assert!((p.distance(v(2.0, 0.0, 0.0)) - 3.0).abs() < 1e-9);
    }
}

#[test]
fn through_off_the_plane_is_projected() {
    let pts = three_point_arc(
        DVec3::Z,
        v(1.0, 0.0, 0.0),
        v(-1.0, 0.0, 0.0),
        v(0.0, 1.0, 5.0),
        4,
    )
    .unwrap();
    assert_close(pts[2], v(0.0, 1.0, 0.0));
    for p in &pts {
        assert!(p.z.abs() < 1e-9);
    }
}

#[test]
fn three_point_arc_refuses_degenerate_input() {
    let a = v(0.0, 0.0, 0.0);
    let b = v(2.0, 0.0, 0.0);
    assert!(three_point_arc(DVec3::Z, a, b, v(1.0, 0.0, 0.0), 8).is_none());
    assert!(three_point_arc(DVec3::Z, a, b, v(5.0, 0.0, 0.0), 8).is_none());
    assert!(three_point_arc(DVec3::Z, a, a, v(1.0, 1.0, 0.0), 8).is_none());
    assert!(three_point_arc(DVec3::Z, a, b, a, 8).is_none());
    assert!(three_point_arc(DVec3::Z, a, b, b, 8).is_none());
    assert!(three_point_arc(DVec3::Z, a, b, v(1.0, 1.0, 0.0), 0).is_none());
    assert!(three_point_arc(DVec3::ZERO, a, b, v(1.0, 1.0, 0.0), 8).is_none());
    assert!(three_point_arc(DVec3::Z, a, b, v(f64::NAN, 1.0, 0.0), 8).is_none());
    assert!(
        three_point_arc(DVec3::Z, a, v(f64::INFINITY, 0.0, 0.0), v(1.0, 1.0, 0.0), 8).is_none()
    );
}

#[test]
fn unit_length_normal_not_required() {
    let pts = three_point_arc(
        v(0.0, 0.0, 7.0),
        v(1.0, 0.0, 0.0),
        v(-1.0, 0.0, 0.0),
        v(0.0, 1.0, 0.0),
        2,
    )
    .unwrap();
    assert_close(pts[1], v(0.0, 1.0, 0.0));
}

// ---------- tool state ----------

#[test]
fn clicks_draw_one_arc_in_one_undo_step() {
    let mut w = World::new();
    let mut t = Arc3Tool::new(xy());
    assert_eq!(t.placed(), 0);
    assert_eq!(t.anchor(), None);
    assert_eq!(t.segments(), 12);
    w.send(&mut t, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    assert_eq!(t.placed(), 1);
    assert_close(t.anchor().unwrap(), v(1.0, 0.0, 0.0));
    w.send(&mut t, Input::Click(v(-1.0, 0.0, 0.0))).unwrap();
    assert_eq!(t.placed(), 2);
    assert_close(t.anchor().unwrap(), v(-1.0, 0.0, 0.0));
    assert_eq!(w.edges(), 0);
    w.send(&mut t, Input::Click(v(0.0, 1.0, 0.0))).unwrap();
    assert_eq!(t.placed(), 0);
    assert_eq!(w.edges(), 12);
    assert_eq!(w.depth(), 1);
    assert_eq!(w.undo.undo_label(), Some("Arc"));
}

#[test]
fn segment_count_can_be_typed_anywhere() {
    let mut w = World::new();
    let mut t = Arc3Tool::new(xy());
    w.send(&mut t, typed("5s")).unwrap();
    assert_eq!(t.segments(), 5);
    assert_eq!(t.placed(), 0);
    w.send(&mut t, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    w.send(&mut t, typed("7S")).unwrap();
    assert_eq!(t.segments(), 7);
    assert_eq!(t.placed(), 1);
    w.send(&mut t, Input::Click(v(-1.0, 0.0, 0.0))).unwrap();
    w.send(&mut t, Input::Click(v(0.0, 1.0, 0.0))).unwrap();
    assert_eq!(w.edges(), 7);
    w.send(&mut t, Input::Cancel).unwrap();
    assert_eq!(t.segments(), 7);
    for bad in ["0s", "361s", "x s", "-3s", "2.5s"] {
        assert_eq!(
            w.send(&mut t, typed(bad)),
            Err(ToolError::Parse(ParseError::Invalid)),
            "{bad}"
        );
        assert_eq!(t.segments(), 7);
    }
    w.send(&mut t, typed("1s")).unwrap();
    assert_eq!(t.segments(), 1);
    w.send(&mut t, typed("360s")).unwrap();
    assert_eq!(t.segments(), 360);
}

#[test]
fn degenerate_clicks_change_nothing() {
    let mut w = World::new();
    let mut t = Arc3Tool::new(xy());
    w.send(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    // end on the start (also when only above it)
    w.send(&mut t, Input::Click(v(0.0, 0.0, 4.0))).unwrap();
    assert_eq!(t.placed(), 1);
    w.send(&mut t, Input::Click(v(2.0, 0.0, 0.0))).unwrap();
    assert_eq!(t.placed(), 2);
    // collinear bulge
    w.send(&mut t, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    assert_eq!(t.placed(), 2);
    w.send(&mut t, Input::Click(v(9.0, 0.0, 3.0))).unwrap();
    assert_eq!(t.placed(), 2);
    assert_eq!(w.edges(), 0);
    assert_eq!(w.depth(), 0);
}

#[test]
fn non_finite_input() {
    let mut w = World::new();
    let mut t = Arc3Tool::new(xy());
    w.send(&mut t, Input::Move(v(f64::NAN, 0.0, 0.0))).unwrap();
    assert_eq!(
        w.send(&mut t, Input::Click(v(f64::NAN, 0.0, 0.0))),
        Err(ToolError::Geometry(StickyError::NotFinite))
    );
    assert_eq!(t.placed(), 0);
    w.send(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.send(&mut t, Input::Move(v(f64::INFINITY, 0.0, 0.0)))
        .unwrap();
    assert_eq!(t.preview(), None);
    assert_eq!(
        w.send(&mut t, Input::Click(v(0.0, f64::INFINITY, 0.0))),
        Err(ToolError::Geometry(StickyError::NotFinite))
    );
    assert_eq!(t.placed(), 1);
}

#[test]
fn move_while_idle_and_cancel() {
    let mut w = World::new();
    let mut t = Arc3Tool::new(xy());
    w.send(&mut t, Input::Move(v(1.0, 1.0, 0.0))).unwrap();
    assert_eq!(t.preview(), None);
    assert_eq!(t.chord(), None);
    w.send(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.send(&mut t, Input::Click(v(2.0, 0.0, 0.0))).unwrap();
    w.send(&mut t, Input::Cancel).unwrap();
    assert_eq!(t.placed(), 0);
    assert_eq!(t.anchor(), None);
    assert_eq!(w.depth(), 0);
}

#[test]
fn previews_and_readouts() {
    let mut w = World::new();
    let mut t = Arc3Tool::new(xy());
    w.send(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    assert_eq!(t.preview(), None);
    assert_eq!(t.chord(), None);
    w.send(&mut t, Input::Move(v(3.0, 4.0, 9.0))).unwrap();
    let p = t.preview().unwrap();
    assert_eq!(p.len(), 2);
    assert_close(p[0], v(0.0, 0.0, 0.0));
    assert_close(p[1], v(3.0, 4.0, 0.0));
    assert!((t.chord().unwrap() - 5.0).abs() < 1e-9);
    assert_eq!(t.radius(), None);
    w.send(&mut t, Input::Move(v(0.0, 0.0, 9.0))).unwrap();
    assert_eq!(t.preview(), None);
    assert_eq!(t.chord(), None);

    w.send(&mut t, Input::Click(v(2.0, 0.0, 0.0))).unwrap();
    assert!((t.chord().unwrap() - 2.0).abs() < 1e-9);
    assert_eq!(t.preview(), None);
    w.send(&mut t, Input::Move(v(1.0, 1.0, 0.0))).unwrap();
    let arc = t.preview().unwrap();
    assert_eq!(arc.len(), 13);
    assert_close(arc[0], v(0.0, 0.0, 0.0));
    assert_close(arc[12], v(2.0, 0.0, 0.0));
    assert!((t.radius().unwrap() - 1.0).abs() < 1e-9);
    w.send(&mut t, Input::Move(v(1.0, 0.0, 0.0))).unwrap();
    assert_eq!(t.preview(), None);
    assert_eq!(t.radius(), None);
    assert_eq!(w.edges(), 0);
}

#[test]
fn plane_is_only_replaced_while_idle() {
    let mut w = World::new();
    let mut t = Arc3Tool::new(xy());
    let xz = DrawPlane {
        u: DVec3::X,
        v: DVec3::Z,
    };
    t.set_plane(xz);
    assert_eq!(t.plane(), xz);
    w.send(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    t.set_plane(xy());
    assert_eq!(t.plane(), xz);
    // in the xz plane the normal is -Y; draw through a point with z > 0
    w.send(&mut t, Input::Click(v(2.0, 0.0, 0.0))).unwrap();
    w.send(&mut t, Input::Click(v(1.0, 0.0, 1.0))).unwrap();
    assert_eq!(w.edges(), 12);
    let c = w.model.body(BodyId::Root).unwrap().geometry();
    for (_, vx) in c.vertices() {
        assert!(vx.position.y.abs() < 1e-9);
        assert!(vx.position.z >= -1e-9);
    }
}

// ---------- typed values ----------

#[test]
fn typed_needs_a_start() {
    let mut w = World::new();
    let mut t = Arc3Tool::new(xy());
    assert_eq!(w.send(&mut t, typed("2")), Err(ToolError::NotStarted));
}

#[test]
fn typed_chord_then_bulge() {
    let mut w = World::new();
    let mut t = Arc3Tool::new(xy());
    w.send(&mut t, Input::Click(v(1.0, 1.0, 0.0))).unwrap();
    w.send(&mut t, Input::Move(v(1.0, 9.0, 0.0))).unwrap();
    w.send(&mut t, typed("2")).unwrap();
    assert_eq!(t.placed(), 2);
    assert_close(t.anchor().unwrap(), v(1.0, 3.0, 0.0));
    assert_eq!(w.edges(), 0);
    // chord runs along +Y; left of it (normal Z x chord) is -X; cursor on the right (+X)
    w.send(&mut t, Input::Move(v(5.0, 2.0, 0.0))).unwrap();
    w.send(&mut t, typed("1")).unwrap();
    assert_eq!(t.placed(), 0);
    assert_eq!(w.edges(), 12);
    assert_eq!(w.depth(), 1);
    let c = w.model.body(BodyId::Root).unwrap().geometry();
    let max_x = c
        .vertices()
        .map(|(_, vx)| vx.position.x)
        .fold(f64::MIN, f64::max);
    let min_x = c
        .vertices()
        .map(|(_, vx)| vx.position.x)
        .fold(f64::MAX, f64::min);
    assert!(
        (max_x - 2.0).abs() < 1e-9,
        "apex is 1 to the right of the chord"
    );
    assert!((min_x - 1.0).abs() < 1e-9);
}

#[test]
fn typed_bulge_side_follows_the_cursor_and_ignores_the_sign() {
    for (cursor, apex_x) in [
        (v(5.0, 1.0, 0.0), 1.0),
        (v(-5.0, 1.0, 0.0), -1.0),
        // no useful side: default is left of the chord
        (v(0.0, 1.0, 0.0), -1.0),
    ] {
        for text in ["1", "-1"] {
            let mut w = World::new();
            let mut t = Arc3Tool::new(xy());
            w.send(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
            w.send(&mut t, Input::Click(v(0.0, 2.0, 0.0))).unwrap();
            w.send(&mut t, Input::Move(cursor)).unwrap();
            w.send(&mut t, typed(text)).unwrap();
            let c = w.model.body(BodyId::Root).unwrap().geometry();
            let far = c
                .vertices()
                .map(|(_, vx)| vx.position.x)
                .fold(0.0f64, |m, x| if x.abs() > m.abs() { x } else { m });
            assert!((far - apex_x).abs() < 1e-9, "{cursor:?} {text}: {far}");
        }
    }
}

#[test]
fn typed_bulge_without_cursor_is_left_of_the_chord() {
    let mut w = World::new();
    let mut t = Arc3Tool::new(xy());
    w.send(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.send(&mut t, Input::Click(v(2.0, 0.0, 0.0))).unwrap();
    w.send(&mut t, typed("1")).unwrap();
    // chord along +X, left is +Y: a half circle above the chord
    let c = w.model.body(BodyId::Root).unwrap().geometry();
    assert!(c.vertices().all(|(_, vx)| vx.position.y >= -1e-9));
    assert!(
        c.vertices()
            .any(|(_, vx)| (vx.position.y - 1.0).abs() < 1e-9)
    );
}

#[test]
fn typed_chord_without_cursor_uses_u() {
    let mut w = World::new();
    let mut t = Arc3Tool::new(xy());
    w.send(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.send(&mut t, typed("500mm")).unwrap();
    assert_close(t.anchor().unwrap(), v(0.5, 0.0, 0.0));
}

#[test]
fn typed_errors_change_nothing() {
    let mut w = World::new();
    let mut t = Arc3Tool::new(xy());
    w.send(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    assert_eq!(
        w.send(&mut t, typed("abc")),
        Err(ToolError::Parse(ParseError::Invalid))
    );
    assert_eq!(w.send(&mut t, typed("0")), Err(ToolError::NoDirection));
    assert_eq!(t.placed(), 1);
    w.send(&mut t, typed("2")).unwrap();
    assert_eq!(
        w.send(&mut t, typed("abc")),
        Err(ToolError::Parse(ParseError::Invalid))
    );
    assert_eq!(w.send(&mut t, typed("0")), Err(ToolError::NoDirection));
    assert_eq!(t.placed(), 2);
    assert_eq!(w.edges(), 0);
    assert_eq!(w.depth(), 0);
}

// ---------- drawing onto existing geometry ----------

#[test]
fn arc_across_an_existing_edge_splits_it() {
    let mut w = World::new();
    let mut t = Arc3Tool::new(xy());
    // a closed half-disc: the arc plus the chord makes a face
    w.send(&mut t, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    w.send(&mut t, Input::Click(v(-1.0, 0.0, 0.0))).unwrap();
    w.send(&mut t, Input::Click(v(0.0, 1.0, 0.0))).unwrap();
    let mut line = line::LineTool::new();
    {
        let mut tg = Target {
            model: &mut w.model,
            body: BodyId::Root,
            undo: &mut w.undo,
            unit: Unit::Meter,
        };
        line.handle(Input::Click(v(-1.0, 0.0, 0.0)), &mut tg)
            .unwrap();
        line.handle(Input::Click(v(1.0, 0.0, 0.0)), &mut tg)
            .unwrap();
    }
    let c = w.model.body(BodyId::Root).unwrap().geometry();
    assert_eq!(c.faces().count(), 1);
    assert_eq!(w.depth(), 2);
}
