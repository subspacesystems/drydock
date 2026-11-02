// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Draw plane and stage accessors of the shape tools, for the editor (`m2-app-shapes`).

use drydock_kernel::model::*;
use drydock_kernel::tools::arc::ArcTool;
use drydock_kernel::tools::circle::CircleTool;
use drydock_kernel::tools::plane::DrawPlane;
use drydock_kernel::tools::rectangle::RectTool;
use drydock_kernel::tools::*;
use drydock_kernel::units::Unit;
use glam::DVec3;

const GROUND: DrawPlane = DrawPlane {
    u: DVec3::X,
    v: DVec3::Y,
};
const WALL: DrawPlane = DrawPlane {
    u: DVec3::X,
    v: DVec3::Z,
};

fn send(tool: &mut dyn FnMut(Input, &mut Target) -> Result<(), ToolError>, input: Input) {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let mut t = Target {
        model: &mut model,
        body: BodyId::Root,
        undo: &mut undo,
        unit: Unit::Meter,
    };
    tool(input, &mut t).unwrap();
}

#[test]
fn rect_set_plane_only_while_idle() {
    let mut r = RectTool::new(GROUND);
    r.set_plane(WALL);
    assert_eq!(r.plane(), WALL);
    send(&mut |i, t| r.handle(i, t), Input::Click(DVec3::ZERO));
    r.set_plane(GROUND);
    assert_eq!(r.plane(), WALL);
    send(&mut |i, t| r.handle(i, t), Input::Cancel);
    r.set_plane(GROUND);
    assert_eq!(r.plane(), GROUND);
}

#[test]
fn circle_set_plane_only_while_idle() {
    let mut c = CircleTool::new(GROUND);
    c.set_plane(WALL);
    assert_eq!(c.plane(), WALL);
    send(&mut |i, t| c.handle(i, t), Input::Click(DVec3::ZERO));
    c.set_plane(GROUND);
    assert_eq!(c.plane(), WALL);
}

#[test]
fn arc_placed_counts_stages_and_set_plane_waits_for_idle() {
    let mut a = ArcTool::new(GROUND);
    assert_eq!(a.placed(), 0);
    send(&mut |i, t| a.handle(i, t), Input::Click(DVec3::ZERO));
    assert_eq!(a.placed(), 1);
    a.set_plane(WALL);
    assert_eq!(a.plane(), GROUND);
    send(&mut |i, t| a.handle(i, t), Input::Click(DVec3::X));
    assert_eq!(a.placed(), 2);
    send(&mut |i, t| a.handle(i, t), Input::Cancel);
    assert_eq!(a.placed(), 0);
    a.set_plane(WALL);
    assert_eq!(a.plane(), WALL);
}
