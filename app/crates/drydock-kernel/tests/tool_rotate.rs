// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Rotate tool state machine (`docs/specs/m3-tool-rotate.md`).

use drydock_kernel::model::*;
use drydock_kernel::select::Entity;
use drydock_kernel::tools::plane::DrawPlane;
use drydock_kernel::tools::rotate::*;
use drydock_kernel::tools::{Input, Target};
use drydock_kernel::topology::FaceId;
use drydock_kernel::transform::TransformError;
use drydock_kernel::units::{ParseError, Unit};
use glam::{DQuat, DVec3};

mod transform_common;
use transform_common::*;

fn xy() -> DrawPlane {
    DrawPlane {
        u: DVec3::X,
        v: DVec3::Y,
    }
}

/// A model with one unit square at x 1..2, y 0..1, z 0.
struct World {
    model: Model,
    undo: UndoStack,
    face: FaceId,
}

impl World {
    fn new() -> World {
        let mut model = Model::new();
        let face = mesh(
            &mut model,
            &[
                v(1.0, 0.0, 0.0),
                v(2.0, 0.0, 0.0),
                v(2.0, 1.0, 0.0),
                v(1.0, 1.0, 0.0),
            ],
            &[&[0, 1, 2, 3]],
        )[0];
        World {
            model,
            undo: UndoStack::new(),
            face,
        }
    }

    fn sel(&self) -> Vec<Entity> {
        vec![Entity::Face(self.face)]
    }

    fn run(&mut self, tool: &mut RotateTool, input: Input) -> Result<(), RotateError> {
        let sel = self.sel();
        self.run_with(tool, input, &sel)
    }

    fn run_with(
        &mut self,
        tool: &mut RotateTool,
        input: Input,
        sel: &[Entity],
    ) -> Result<(), RotateError> {
        let mut target = Target {
            model: &mut self.model,
            body: BodyId::Root,
            undo: &mut self.undo,
            unit: Unit::Inch,
        };
        tool.handle(input, sel, &mut target)
    }
}

const SQUARE: [DVec3; 4] = [
    DVec3::new(1.0, 0.0, 0.0),
    DVec3::new(2.0, 0.0, 0.0),
    DVec3::new(2.0, 1.0, 0.0),
    DVec3::new(1.0, 1.0, 0.0),
];

/// `pts` turned by `deg` degrees about the axis through `center` along `axis`.
fn rotated(pts: &[DVec3], center: DVec3, axis: DVec3, deg: f64) -> Vec<DVec3> {
    let q = DQuat::from_axis_angle(axis.normalize(), deg.to_radians());
    pts.iter().map(|&p| center + q * (p - center)).collect()
}

fn assert_positions(model: &Model, expected: &[DVec3]) {
    let got = positions(model);
    assert_eq!(got.len(), expected.len(), "{got:?} vs {expected:?}");
    for e in expected {
        assert!(
            got.iter().any(|g| g.distance(*e) < 1e-9),
            "{e:?} missing from {got:?}"
        );
    }
}

fn assert_angle(got: Option<f64>, want: f64) {
    let got = got.expect("an angle");
    assert!((got - want).abs() < 1e-9, "angle {got} != {want}");
}

/// A point at `deg` degrees on a circle of radius `r` about `center` in the xy plane.
fn at(center: DVec3, r: f64, deg: f64) -> DVec3 {
    let a = deg.to_radians();
    center + v(a.cos(), a.sin(), 0.0) * r
}

fn typed(s: &str) -> Input {
    Input::Typed(s.to_string())
}

/// Center at the origin, start along +x; the tool is left at `Angle`.
fn start_at_origin(w: &mut World, tool: &mut RotateTool) {
    w.run(tool, Input::Click(DVec3::ZERO)).unwrap();
    w.run(tool, Input::Click(v(3.0, 0.0, 0.0))).unwrap();
    assert_eq!(tool.stage(), RotateStage::Angle);
}

// -- parse_angle -----------------------------------------------------------------------

#[test]
fn parse_angle_reads_degrees_with_optional_suffix() {
    assert_eq!(parse_angle("45"), Ok(45.0));
    assert_eq!(parse_angle("  -7.5 "), Ok(-7.5));
    assert_eq!(parse_angle("+90"), Ok(90.0));
    assert_eq!(parse_angle("1e1"), Ok(10.0));
    assert_eq!(parse_angle("30°"), Ok(30.0));
    assert_eq!(parse_angle("30 °"), Ok(30.0));
    assert_eq!(parse_angle("12.5deg"), Ok(12.5));
    assert_eq!(parse_angle(" 12.5 deg "), Ok(12.5));
}

#[test]
fn parse_angle_refuses_empty_and_junk() {
    assert_eq!(parse_angle(""), Err(ParseError::Empty));
    assert_eq!(parse_angle("   "), Err(ParseError::Empty));
    assert_eq!(parse_angle("°"), Err(ParseError::Empty));
    assert_eq!(parse_angle("abc"), Err(ParseError::Invalid));
    assert_eq!(parse_angle("4 5"), Err(ParseError::Invalid));
    assert_eq!(parse_angle("inf"), Err(ParseError::Invalid));
    assert_eq!(parse_angle("NaN"), Err(ParseError::Invalid));
    assert_eq!(parse_angle("45rad"), Err(ParseError::Invalid));
}

// -- stages ----------------------------------------------------------------------------

#[test]
fn a_new_tool_waits_for_the_center() {
    let tool = RotateTool::new(xy());
    assert_eq!(tool.stage(), RotateStage::Center);
    assert_eq!(tool.plane(), xy());
    assert_eq!(tool.center(), None);
    assert_eq!(tool.angle(), None);
    assert_eq!(tool.last_angle(&UndoStack::new()), None);
    assert_eq!(tool.matrix(), None);
    assert!(tool.preview().is_empty());
}

#[test]
fn three_clicks_rotate_the_selection_one_undo_step() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    w.run(&mut tool, Input::Move(v(0.5, 0.5, 0.0))).unwrap();
    assert_eq!(tool.stage(), RotateStage::Center);

    w.run(&mut tool, Input::Click(DVec3::ZERO)).unwrap();
    assert_eq!(tool.stage(), RotateStage::Start);
    assert_eq!(tool.center(), Some(DVec3::ZERO));

    w.run(&mut tool, Input::Click(v(3.0, 0.0, 0.0))).unwrap();
    assert_eq!(tool.stage(), RotateStage::Angle);
    assert_angle(tool.angle(), 0.0);

    w.run(&mut tool, Input::Move(v(0.0, 2.0, 0.0))).unwrap();
    assert_angle(tool.angle(), 90.0);
    assert_positions(&w.model, &SQUARE);
    assert_eq!(w.undo.undo_count(), 0);

    w.run(&mut tool, Input::Click(v(0.0, 2.0, 0.0))).unwrap();
    assert_positions(&w.model, &rotated(&SQUARE, DVec3::ZERO, DVec3::Z, 90.0));
    assert_eq!(tool.stage(), RotateStage::Center);
    assert_eq!(tool.center(), None);
    assert_eq!(tool.angle(), None);
    assert_angle(tool.last_angle(&w.undo), 90.0);
    assert_eq!(w.undo.undo_count(), 1);

    assert_eq!(
        w.undo.undo(&mut w.model).unwrap().as_deref(),
        Some("Rotate")
    );
    assert_positions(&w.model, &SQUARE);
}

#[test]
fn rotation_is_about_the_center_and_the_plane_normal() {
    let mut w = World::new();
    // Normal +x: from +y toward +z is a positive turn.
    let plane = DrawPlane {
        u: DVec3::Y,
        v: DVec3::Z,
    };
    let mut tool = RotateTool::new(plane);
    let c = v(1.0, 0.0, 0.0);
    w.run(&mut tool, Input::Click(c)).unwrap();
    w.run(&mut tool, Input::Click(c + DVec3::Y)).unwrap();
    w.run(&mut tool, Input::Move(c + DVec3::Z * 4.0)).unwrap();
    assert_angle(tool.angle(), 90.0);
    w.run(&mut tool, Input::Click(c + DVec3::Z * 4.0)).unwrap();
    assert_positions(&w.model, &rotated(&SQUARE, c, DVec3::X, 90.0));
}

#[test]
fn points_off_the_plane_are_projected_into_it() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    w.run(&mut tool, Input::Click(v(0.0, 0.0, 1.0))).unwrap();
    assert_eq!(tool.center(), Some(v(0.0, 0.0, 1.0)));
    // Start straight above the center: no direction once projected, ignored.
    w.run(&mut tool, Input::Click(v(0.0, 0.0, 7.0))).unwrap();
    assert_eq!(tool.stage(), RotateStage::Start);
    w.run(&mut tool, Input::Click(v(2.0, 0.0, -3.0))).unwrap();
    assert_eq!(tool.stage(), RotateStage::Angle);
    w.run(&mut tool, Input::Move(v(-1.0, 0.0, 9.0))).unwrap();
    assert_angle(tool.angle(), 180.0);
    w.run(&mut tool, Input::Click(v(-1.0, 0.0, 9.0))).unwrap();
    assert_positions(
        &w.model,
        &rotated(&SQUARE, v(0.0, 0.0, 1.0), DVec3::Z, 180.0),
    );
}

#[test]
fn set_plane_only_before_the_center() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    let side = DrawPlane {
        u: DVec3::Y,
        v: DVec3::Z,
    };
    tool.set_plane(side);
    assert_eq!(tool.plane(), side);
    tool.set_plane(xy());
    w.run(&mut tool, Input::Click(DVec3::ZERO)).unwrap();
    tool.set_plane(side);
    assert_eq!(tool.plane(), xy());
    w.run(&mut tool, Input::Click(v(3.0, 0.0, 0.0))).unwrap();
    tool.set_plane(side);
    assert_eq!(tool.plane(), xy());
}

// -- snapping --------------------------------------------------------------------------

#[test]
fn cursor_angles_snap_to_15_degrees_within_the_window() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    start_at_origin(&mut w, &mut tool);
    let cases = [
        (46.0, 45.0),
        (43.1, 45.0),
        (47.5, 47.5),
        (13.5, 15.0),
        (7.0, 7.0),
        (1.5, 0.0),
        (-31.0, -30.0),
        (-52.0, -52.0),
        (179.0, 180.0),
        (-179.0, 180.0),
        (-170.0, -170.0),
        (91.9, 90.0),
    ];
    for (cursor, want) in cases {
        w.run(&mut tool, Input::Move(at(DVec3::ZERO, 2.5, cursor)))
            .unwrap();
        assert_angle(tool.angle(), want);
    }
}

#[test]
fn angle_snap_off_keeps_the_exact_cursor_angle() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    start_at_origin(&mut w, &mut tool);
    tool.set_angle_snap(false);
    w.run(&mut tool, Input::Move(at(DVec3::ZERO, 2.5, 89.0)))
        .unwrap();
    assert_angle(tool.angle(), 89.0);
    w.run(&mut tool, Input::Click(at(DVec3::ZERO, 2.5, 43.5)))
        .unwrap();
    assert_angle(tool.last_angle(&w.undo), 43.5);
    tool.set_angle_snap(true);
    start_at_origin(&mut w, &mut tool);
    w.run(&mut tool, Input::Move(at(DVec3::ZERO, 2.5, 89.0)))
        .unwrap();
    assert_angle(tool.angle(), 90.0);
}

#[test]
fn a_cursor_on_the_center_keeps_the_previous_angle() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    start_at_origin(&mut w, &mut tool);
    w.run(&mut tool, Input::Move(at(DVec3::ZERO, 1.0, 60.0)))
        .unwrap();
    w.run(&mut tool, Input::Move(v(0.0, 0.0, 5.0))).unwrap();
    assert_angle(tool.angle(), 60.0);
}

#[test]
fn a_click_snaps_like_a_move() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    start_at_origin(&mut w, &mut tool);
    w.run(&mut tool, Input::Click(at(DVec3::ZERO, 2.0, 31.2)))
        .unwrap();
    assert_positions(&w.model, &rotated(&SQUARE, DVec3::ZERO, DVec3::Z, 30.0));
    assert_angle(tool.last_angle(&w.undo), 30.0);
}

#[test]
fn a_click_at_zero_angle_rotates_nothing() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    start_at_origin(&mut w, &mut tool);
    // 1 degree snaps to 0.
    w.run(&mut tool, Input::Click(at(DVec3::ZERO, 2.0, 1.0)))
        .unwrap();
    assert_eq!(tool.stage(), RotateStage::Angle);
    assert_eq!(w.undo.undo_count(), 0);
    assert_positions(&w.model, &SQUARE);
}

// -- typed angles ----------------------------------------------------------------------

#[test]
fn typed_angle_follows_the_cursor_side() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    start_at_origin(&mut w, &mut tool);
    w.run(&mut tool, Input::Move(at(DVec3::ZERO, 2.0, -30.0)))
        .unwrap();
    w.run(&mut tool, typed("45")).unwrap();
    assert_positions(&w.model, &rotated(&SQUARE, DVec3::ZERO, DVec3::Z, -45.0));
    assert_angle(tool.last_angle(&w.undo), -45.0);
    assert_eq!(tool.stage(), RotateStage::Center);
}

#[test]
fn typed_negative_angle_goes_against_the_cursor() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    start_at_origin(&mut w, &mut tool);
    w.run(&mut tool, Input::Move(at(DVec3::ZERO, 2.0, -30.0)))
        .unwrap();
    w.run(&mut tool, typed("-20")).unwrap();
    assert_positions(&w.model, &rotated(&SQUARE, DVec3::ZERO, DVec3::Z, 20.0));
}

#[test]
fn typed_angle_at_zero_cursor_is_counter_clockwise_and_not_snapped() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    start_at_origin(&mut w, &mut tool);
    w.run(&mut tool, typed("14")).unwrap();
    assert_positions(&w.model, &rotated(&SQUARE, DVec3::ZERO, DVec3::Z, 14.0));
}

#[test]
fn typed_zero_or_junk_changes_nothing() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    start_at_origin(&mut w, &mut tool);
    w.run(&mut tool, Input::Move(at(DVec3::ZERO, 2.0, 60.0)))
        .unwrap();
    assert_eq!(w.run(&mut tool, typed("0")), Ok(()));
    assert_eq!(
        w.run(&mut tool, typed("x")),
        Err(RotateError::Parse(ParseError::Invalid))
    );
    assert_eq!(tool.stage(), RotateStage::Angle);
    assert_angle(tool.angle(), 60.0);
    assert_eq!(w.undo.undo_count(), 0);
    assert_positions(&w.model, &SQUARE);
}

#[test]
fn typed_angle_needs_a_start() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    assert_eq!(w.run(&mut tool, typed("30")), Err(RotateError::NotStarted));
    w.run(&mut tool, Input::Click(DVec3::ZERO)).unwrap();
    assert_eq!(w.run(&mut tool, typed("30")), Err(RotateError::NotStarted));
    assert_eq!(tool.stage(), RotateStage::Start);
    assert_eq!(w.undo.undo_count(), 0);
}

#[test]
fn typed_angle_right_after_changes_the_last_rotation() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    start_at_origin(&mut w, &mut tool);
    w.run(&mut tool, Input::Click(at(DVec3::ZERO, 2.0, 90.0)))
        .unwrap();
    assert_eq!(w.undo.undo_count(), 1);

    w.run(&mut tool, typed("45")).unwrap();
    assert_positions(&w.model, &rotated(&SQUARE, DVec3::ZERO, DVec3::Z, 45.0));
    assert_angle(tool.last_angle(&w.undo), 45.0);
    assert_eq!(w.undo.undo_count(), 1);
    assert_eq!(w.undo.redo_count(), 0);

    // Negative: against the last direction. Still one step.
    w.run(&mut tool, typed("-10")).unwrap();
    assert_positions(&w.model, &rotated(&SQUARE, DVec3::ZERO, DVec3::Z, -10.0));
    assert_angle(tool.last_angle(&w.undo), -10.0);
    // And now the last direction is clockwise.
    w.run(&mut tool, typed("20")).unwrap();
    assert_positions(&w.model, &rotated(&SQUARE, DVec3::ZERO, DVec3::Z, -20.0));
    assert_eq!(w.undo.undo_count(), 1);

    w.undo.undo(&mut w.model).unwrap();
    assert_positions(&w.model, &SQUARE);
}

#[test]
fn typed_angle_after_an_undo_or_a_new_center_is_not_started() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    start_at_origin(&mut w, &mut tool);
    w.run(&mut tool, Input::Click(at(DVec3::ZERO, 2.0, 90.0)))
        .unwrap();
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(tool.last_angle(&w.undo), None);
    assert_eq!(w.run(&mut tool, typed("45")), Err(RotateError::NotStarted));
    assert_positions(&w.model, &SQUARE);

    w.undo.redo(&mut w.model).unwrap();
    assert_angle(tool.last_angle(&w.undo), 90.0);
    w.run(&mut tool, Input::Click(v(5.0, 5.0, 0.0))).unwrap();
    assert_eq!(tool.last_angle(&w.undo), None);
    w.run(&mut tool, Input::Cancel).unwrap();
    assert_eq!(tool.stage(), RotateStage::Center);
    assert_eq!(w.run(&mut tool, typed("45")), Err(RotateError::NotStarted));
    assert_positions(&w.model, &rotated(&SQUARE, DVec3::ZERO, DVec3::Z, 90.0));
}

#[test]
fn typed_angle_after_another_edit_is_not_started() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    start_at_origin(&mut w, &mut tool);
    w.run(&mut tool, Input::Click(at(DVec3::ZERO, 2.0, 90.0)))
        .unwrap();
    let sel = w.sel();
    drydock_kernel::transform::transform(
        &mut w.model,
        BodyId::Root,
        &sel,
        glam::DMat4::from_translation(v(0.0, 0.0, 1.0)),
        &mut w.undo,
        "Move",
    )
    .unwrap();
    assert_eq!(tool.last_angle(&w.undo), None);
    assert_eq!(w.run(&mut tool, typed("45")), Err(RotateError::NotStarted));
}

#[test]
fn typed_angle_after_refused_keeps_the_old_rotation() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    start_at_origin(&mut w, &mut tool);
    w.run(&mut tool, Input::Click(at(DVec3::ZERO, 2.0, 90.0)))
        .unwrap();
    let bogus = [Entity::Face(FaceId(999))];
    assert_eq!(
        w.run_with(&mut tool, typed("45"), &bogus),
        Err(RotateError::Transform(TransformError::MissingEntity(
            Entity::Face(FaceId(999))
        )))
    );
    assert_positions(&w.model, &rotated(&SQUARE, DVec3::ZERO, DVec3::Z, 90.0));
    assert_eq!(w.undo.undo_count(), 1);
    assert_eq!(w.undo.redo_count(), 0);
    assert_eq!(
        w.run_with(&mut tool, typed("45"), &[]),
        Err(RotateError::Empty)
    );
    assert_positions(&w.model, &rotated(&SQUARE, DVec3::ZERO, DVec3::Z, 90.0));
    assert_eq!(w.undo.undo_count(), 1);
}

// -- refusals and cancel ---------------------------------------------------------------

#[test]
fn center_needs_a_selection_and_a_finite_point() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    assert_eq!(
        w.run_with(&mut tool, Input::Click(DVec3::ZERO), &[]),
        Err(RotateError::Empty)
    );
    assert_eq!(tool.stage(), RotateStage::Center);
    assert_eq!(
        w.run(&mut tool, Input::Click(v(f64::NAN, 0.0, 0.0))),
        Err(RotateError::NotFinite)
    );
    assert_eq!(tool.stage(), RotateStage::Center);
    w.run(&mut tool, Input::Click(DVec3::ZERO)).unwrap();
    assert_eq!(
        w.run(&mut tool, Input::Click(v(f64::INFINITY, 0.0, 0.0))),
        Err(RotateError::NotFinite)
    );
    assert_eq!(tool.stage(), RotateStage::Start);
}

#[test]
fn a_refused_rotation_keeps_the_tool_state() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    let bogus = [Entity::Face(FaceId(999))];
    w.run_with(&mut tool, Input::Click(DVec3::ZERO), &bogus)
        .unwrap();
    w.run_with(&mut tool, Input::Click(v(3.0, 0.0, 0.0)), &bogus)
        .unwrap();
    let r = w.run_with(&mut tool, Input::Click(at(DVec3::ZERO, 2.0, 90.0)), &bogus);
    assert_eq!(
        r,
        Err(RotateError::Transform(TransformError::MissingEntity(
            Entity::Face(FaceId(999))
        )))
    );
    assert_eq!(tool.stage(), RotateStage::Angle);
    assert_eq!(tool.center(), Some(DVec3::ZERO));
    assert_angle(tool.angle(), 90.0);
    assert_eq!(w.undo.undo_count(), 0);
    // The same click with a real selection goes through.
    w.run(&mut tool, Input::Click(at(DVec3::ZERO, 2.0, 90.0)))
        .unwrap();
    assert_positions(&w.model, &rotated(&SQUARE, DVec3::ZERO, DVec3::Z, 90.0));
}

#[test]
fn cancel_goes_back_to_the_center_stage() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    w.run(&mut tool, Input::Cancel).unwrap();
    assert_eq!(tool.stage(), RotateStage::Center);
    w.run(&mut tool, Input::Click(DVec3::ZERO)).unwrap();
    w.run(&mut tool, Input::Cancel).unwrap();
    assert_eq!(tool.stage(), RotateStage::Center);
    assert_eq!(tool.center(), None);
    start_at_origin(&mut w, &mut tool);
    w.run(&mut tool, Input::Move(at(DVec3::ZERO, 2.0, 90.0)))
        .unwrap();
    w.run(&mut tool, Input::Cancel).unwrap();
    assert_eq!(tool.stage(), RotateStage::Center);
    assert_eq!(tool.angle(), None);
    assert_eq!(tool.matrix(), None);
    assert!(tool.preview().is_empty());
    assert_eq!(w.undo.undo_count(), 0);
    assert_positions(&w.model, &SQUARE);
}

// -- queries ---------------------------------------------------------------------------

#[test]
fn matrix_is_the_rotation_the_click_would_apply() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    let c = v(1.0, 1.0, 0.0);
    w.run(&mut tool, Input::Click(c)).unwrap();
    assert_eq!(tool.matrix(), None);
    w.run(&mut tool, Input::Click(c + DVec3::X)).unwrap();
    w.run(&mut tool, Input::Move(at(c, 3.0, 90.0))).unwrap();
    let m = tool.matrix().expect("matrix while rotating");
    assert!(m.transform_point3(c).distance(c) < 1e-12);
    assert!(
        m.transform_point3(v(2.0, 1.0, 0.0))
            .distance(v(1.0, 2.0, 0.0))
            < 1e-12
    );
    assert!(
        m.transform_point3(v(2.0, 1.0, 5.0))
            .distance(v(1.0, 2.0, 5.0))
            < 1e-12
    );
}

#[test]
fn preview_shows_the_protractor_arms() {
    let mut w = World::new();
    let mut tool = RotateTool::new(xy());
    w.run(&mut tool, Input::Move(v(1.0, 0.0, 0.0))).unwrap();
    assert!(tool.preview().is_empty());
    let c = v(0.0, 0.0, 2.0);
    w.run(&mut tool, Input::Click(c)).unwrap();
    // No cursor yet.
    assert!(tool.preview().is_empty());
    w.run(&mut tool, Input::Move(v(0.0, 2.0, 0.0))).unwrap();
    let p = tool.preview();
    assert_eq!(p.len(), 1);
    assert!(p[0].0.distance(c) < 1e-12);
    assert!(p[0].1.distance(v(0.0, 2.0, 2.0)) < 1e-12);
    // On the center: no direction, no arm.
    w.run(&mut tool, Input::Move(c)).unwrap();
    assert!(tool.preview().is_empty());

    w.run(&mut tool, Input::Click(v(3.0, 0.0, 2.0))).unwrap();
    w.run(&mut tool, Input::Move(at(c, 0.5, 90.0))).unwrap();
    let p = tool.preview();
    assert_eq!(p.len(), 2);
    assert!(p[0].0.distance(c) < 1e-12);
    assert!(p[0].1.distance(v(3.0, 0.0, 2.0)) < 1e-12);
    assert!(p[1].0.distance(c) < 1e-12);
    assert!(p[1].1.distance(v(0.0, 3.0, 2.0)) < 1e-9);
}
