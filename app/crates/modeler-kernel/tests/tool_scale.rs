// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Scale tool state machine (`docs/specs/m3-tool-scale.md`).

use glam::{DMat4, DVec3};
use modeler_kernel::model::*;
use modeler_kernel::select::Entity;
use modeler_kernel::tools::scale::*;
use modeler_kernel::tools::{Input, Target};
use modeler_kernel::topology::FaceId;
use modeler_kernel::transform::TransformError;
use modeler_kernel::units::{ParseError, Unit};

mod transform_common;
use transform_common::*;

/// A model with the unit cube (0..1 on every axis), or a flat unit square at x 1..2, y 0..1.
struct World {
    model: Model,
    undo: UndoStack,
    faces: Vec<FaceId>,
}

impl World {
    fn cube() -> World {
        let mut model = Model::new();
        let faces = cube(&mut model);
        World {
            model,
            undo: UndoStack::new(),
            faces,
        }
    }

    fn square() -> World {
        let mut model = Model::new();
        let faces = mesh(&mut model, &SQUARE, &[&[0, 1, 2, 3]]);
        World {
            model,
            undo: UndoStack::new(),
            faces,
        }
    }

    fn sel(&self) -> Vec<Entity> {
        self.faces.iter().map(|&f| Entity::Face(f)).collect()
    }

    fn grab(&self, tool: &mut ScaleTool, h: Handle) -> Result<(), ScaleError> {
        tool.grab(h, &self.model, BodyId::Root, &self.sel())
    }

    fn run(&mut self, tool: &mut ScaleTool, input: Input) -> Result<(), ScaleError> {
        let sel = self.sel();
        self.run_with(tool, input, &sel)
    }

    fn run_with(
        &mut self,
        tool: &mut ScaleTool,
        input: Input,
        sel: &[Entity],
    ) -> Result<(), ScaleError> {
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

fn h(x: i8, y: i8, z: i8) -> Handle {
    Handle::new(x, y, z)
}

fn typed(s: &str) -> Input {
    Input::Typed(s.to_string())
}

fn cube_points() -> Vec<DVec3> {
    let mut out = Vec::new();
    for z in [0.0, 1.0] {
        for y in [0.0, 1.0] {
            for x in [0.0, 1.0] {
                out.push(v(x, y, z));
            }
        }
    }
    out
}

/// `pts` scaled by `s` about `f`.
fn scaled(pts: &[DVec3], f: DVec3, s: DVec3) -> Vec<DVec3> {
    pts.iter().map(|&p| f + (p - f) * s).collect()
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

fn assert_vec(got: Option<DVec3>, want: DVec3) {
    let got = got.expect("a vector");
    assert!(got.distance(want) < 1e-9, "{got:?} != {want:?}");
}

fn assert_bounds(got: Option<Bounds>, min: DVec3, max: DVec3) {
    let got = got.expect("a box");
    assert!(
        got.min.distance(min) < 1e-9 && got.max.distance(max) < 1e-9,
        "{got:?} != {min:?}..{max:?}"
    );
}

// -- parse_factors ---------------------------------------------------------------------

#[test]
fn parse_factors_reads_comma_lists() {
    assert_eq!(parse_factors("2"), Ok(vec![2.0]));
    assert_eq!(parse_factors(" 0.5 "), Ok(vec![0.5]));
    assert_eq!(parse_factors("2,3"), Ok(vec![2.0, 3.0]));
    assert_eq!(parse_factors(" 2 , -3 ,+1e-1"), Ok(vec![2.0, -3.0, 0.1]));
    assert_eq!(parse_factors("1,2,3,4"), Ok(vec![1.0, 2.0, 3.0, 4.0]));
    assert_eq!(parse_factors("0"), Ok(vec![0.0]));
}

#[test]
fn parse_factors_refuses_blank_and_junk() {
    assert_eq!(parse_factors(""), Err(ParseError::Empty));
    assert_eq!(parse_factors("   "), Err(ParseError::Empty));
    assert_eq!(parse_factors("x"), Err(ParseError::Invalid));
    assert_eq!(parse_factors("2,"), Err(ParseError::Invalid));
    assert_eq!(parse_factors(",2"), Err(ParseError::Invalid));
    assert_eq!(parse_factors("2,,3"), Err(ParseError::Invalid));
    assert_eq!(parse_factors("2m"), Err(ParseError::Invalid));
    assert_eq!(parse_factors("inf"), Err(ParseError::Invalid));
    assert_eq!(parse_factors("NaN"), Err(ParseError::Invalid));
}

// -- bounds and handles ----------------------------------------------------------------

#[test]
fn bounds_cover_the_points_the_selection_moves() {
    let w = World::cube();
    assert_bounds(
        bounds(&w.model, BodyId::Root, &w.sel()),
        DVec3::ZERO,
        DVec3::ONE,
    );
    // The top face alone: flat at z 1.
    let top = [Entity::Face(w.faces[1])];
    assert_bounds(
        bounds(&w.model, BodyId::Root, &top),
        v(0.0, 0.0, 1.0),
        v(1.0, 1.0, 1.0),
    );
    // An edge: its two ends.
    let edge = w
        .model
        .root()
        .geometry()
        .edges()
        .find(|(_, e)| {
            let c = w.model.root().geometry();
            let a = c.vertex(e.ends[0]).unwrap().position;
            let b = c.vertex(e.ends[1]).unwrap().position;
            a.x == 1.0 && a.y == 1.0 && b.x == 1.0 && b.y == 1.0
        })
        .map(|(id, _)| id)
        .unwrap();
    assert_bounds(
        bounds(&w.model, BodyId::Root, &[Entity::Edge(edge)]),
        v(1.0, 1.0, 0.0),
        v(1.0, 1.0, 1.0),
    );
    assert_eq!(bounds(&w.model, BodyId::Root, &[]), None);
    assert_eq!(
        bounds(&w.model, BodyId::Root, &[Entity::Face(FaceId(999))]),
        None
    );
}

#[test]
fn bounds_include_selected_instances() {
    let mut model = Model::new();
    let mut edit = ModelEdit::new();
    let def = model.add_definition(
        DefinitionInfo {
            name: "stick".into(),
            group: false,
        },
        &mut edit,
    );
    model
        .edit_geometry(BodyId::Definition(def), &mut edit, |ctx, log| {
            let a = ctx.insert_vertex(v(0.0, 0.0, 0.0), log);
            let b = ctx.insert_vertex(v(0.0, 0.0, 2.0), log);
            ctx.insert_edge(a, b, log).unwrap();
        })
        .unwrap();
    let id = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: def,
                transform: DMat4::from_translation(v(5.0, 1.0, 0.0)),
                name: "i".into(),
                material: None,
            },
            &mut edit,
        )
        .unwrap();
    let faces = mesh(&mut model, &SQUARE, &[&[0, 1, 2, 3]]);
    assert_bounds(
        bounds(&model, BodyId::Root, &[Entity::Instance(id)]),
        v(5.0, 1.0, 0.0),
        v(5.0, 1.0, 2.0),
    );
    assert_bounds(
        bounds(
            &model,
            BodyId::Root,
            &[Entity::Instance(id), Entity::Face(faces[0])],
        ),
        v(1.0, 0.0, 0.0),
        v(5.0, 1.0, 2.0),
    );
}

#[test]
fn a_solid_box_has_26_handles_sorted() {
    let b = Bounds {
        min: DVec3::ZERO,
        max: v(1.0, 2.0, 3.0),
    };
    let got = handles(&b);
    let mut want = Vec::new();
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                if (x, y, z) != (0, 0, 0) {
                    want.push(h(x, y, z));
                }
            }
        }
    }
    want.sort();
    assert_eq!(got, want);
    assert_eq!(got.len(), 26);
}

#[test]
fn flat_and_thin_boxes_have_fewer_handles() {
    let flat = Bounds {
        min: DVec3::ZERO,
        max: v(1.0, 1.0, 0.0),
    };
    let got = handles(&flat);
    assert_eq!(got.len(), 8);
    assert!(got.iter().all(|h| h.z == 0));
    assert!(got.contains(&h(1, 1, 0)) && got.contains(&h(-1, 0, 0)));

    let line = Bounds {
        min: v(0.0, 2.0, 0.0),
        max: v(0.0, 2.0, 5.0),
    };
    assert_eq!(handles(&line), vec![h(0, 0, -1), h(0, 0, 1)]);

    let point = Bounds {
        min: DVec3::ONE,
        max: DVec3::ONE,
    };
    assert_eq!(handles(&point), Vec::<Handle>::new());

    // Shorter than LINEAR_TOL counts as flat.
    let nearly = Bounds {
        min: DVec3::ZERO,
        max: v(1.0, 1.0, 1e-7),
    };
    assert_eq!(handles(&nearly).len(), 8);
}

// -- grab ------------------------------------------------------------------------------

#[test]
fn a_new_tool_waits_for_a_handle() {
    let tool = ScaleTool::new();
    assert_eq!(tool.stage(), ScaleStage::Pick);
    assert_eq!(tool.handle_grabbed(), None);
    assert_eq!(tool.uniform(), None);
    assert_eq!(tool.fixed_point(), None);
    assert_eq!(tool.factors(), None);
    assert_eq!(tool.matrix(), None);
    assert_eq!(tool.preview(), None);
    assert_eq!(tool.last_factors(&UndoStack::new()), None);
    assert_eq!(ScaleTool::default().stage(), ScaleStage::Pick);
}

#[test]
fn grab_needs_a_selection_with_points_and_a_live_handle() {
    let w = World::square();
    let mut tool = ScaleTool::new();
    assert_eq!(
        tool.grab(h(1, 0, 0), &w.model, BodyId::Root, &[]),
        Err(ScaleError::Empty)
    );
    assert_eq!(
        tool.grab(
            h(1, 0, 0),
            &w.model,
            BodyId::Root,
            &[Entity::Face(FaceId(999))]
        ),
        Err(ScaleError::Empty)
    );
    // The square is flat in z.
    assert_eq!(w.grab(&mut tool, h(1, 0, 1)), Err(ScaleError::NoHandle));
    assert_eq!(w.grab(&mut tool, h(0, 0, 0)), Err(ScaleError::NoHandle));
    assert_eq!(w.grab(&mut tool, h(2, 0, 0)), Err(ScaleError::NoHandle));
    assert_eq!(tool.stage(), ScaleStage::Pick);
    w.grab(&mut tool, h(1, 0, 0)).unwrap();
    assert_eq!(tool.stage(), ScaleStage::Drag);
}

#[test]
fn grab_starts_a_drag_at_factor_one() {
    let w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(1, 0, 0)).unwrap();
    assert_eq!(tool.stage(), ScaleStage::Drag);
    assert_eq!(tool.handle_grabbed(), Some(h(1, 0, 0)));
    assert_eq!(tool.uniform(), Some(false));
    assert_vec(tool.fixed_point(), v(0.0, 0.5, 0.5));
    assert_vec(tool.factors(), DVec3::ONE);
    assert!(tool.matrix().unwrap().abs_diff_eq(DMat4::IDENTITY, 1e-12));
    assert_bounds(tool.preview(), DVec3::ZERO, DVec3::ONE);

    // A corner is uniform; its fixed point is the opposite corner.
    w.grab(&mut tool, h(1, -1, 1)).unwrap();
    assert_eq!(tool.handle_grabbed(), Some(h(1, -1, 1)));
    assert_eq!(tool.uniform(), Some(true));
    assert_vec(tool.fixed_point(), v(0.0, 1.0, 0.0));
    // An edge middle is not.
    w.grab(&mut tool, h(0, 1, 1)).unwrap();
    assert_eq!(tool.uniform(), Some(false));
    assert_vec(tool.fixed_point(), v(0.5, 0.0, 0.0));
}

#[test]
fn a_grab_while_dragging_starts_over() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(1, 0, 0)).unwrap();
    w.run(&mut tool, Input::Move(v(3.0, 0.0, 0.0))).unwrap();
    assert_vec(tool.factors(), v(3.0, 1.0, 1.0));
    w.grab(&mut tool, h(0, 1, 0)).unwrap();
    assert_eq!(tool.handle_grabbed(), Some(h(0, 1, 0)));
    assert_vec(tool.factors(), DVec3::ONE);
    // No cursor point yet in the new drag: a switch keeps the factors.
    tool.set_about_center(true);
    assert_vec(tool.factors(), DVec3::ONE);
}

// -- cursor factors --------------------------------------------------------------------

#[test]
fn a_face_handle_scales_one_axis_from_the_opposite_face() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(1, 0, 0)).unwrap();
    w.run(&mut tool, Input::Move(v(3.0, 0.2, 9.0))).unwrap();
    assert_vec(tool.factors(), v(3.0, 1.0, 1.0));
    assert_bounds(tool.preview(), DVec3::ZERO, v(3.0, 1.0, 1.0));
    assert_eq!(positions(&w.model).len(), 8);
    assert_positions(&w.model, &cube_points());

    w.run(&mut tool, Input::Click(v(2.0, 5.0, -1.0))).unwrap();
    assert_positions(
        &w.model,
        &scaled(&cube_points(), DVec3::ZERO, v(2.0, 1.0, 1.0)),
    );
    assert_eq!(tool.stage(), ScaleStage::Pick);
    assert_eq!(tool.factors(), None);
    assert_eq!(w.undo.undo_count(), 1);
    assert!(normals_outward(&w.model));

    w.undo.undo(&mut w.model).unwrap();
    assert_positions(&w.model, &cube_points());
}

#[test]
fn a_min_side_handle_measures_from_the_max_side() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(0, -1, 0)).unwrap();
    assert_vec(tool.fixed_point(), v(0.5, 1.0, 0.5));
    w.run(&mut tool, Input::Move(v(0.0, -1.0, 0.0))).unwrap();
    assert_vec(tool.factors(), v(1.0, 2.0, 1.0));
    assert_bounds(tool.preview(), v(0.0, -1.0, 0.0), DVec3::ONE);
}

#[test]
fn an_edge_handle_scales_two_axes_independently() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(1, 1, 0)).unwrap();
    w.run(&mut tool, Input::Move(v(2.0, 3.0, 0.2))).unwrap();
    assert_vec(tool.factors(), v(2.0, 3.0, 1.0));
    w.run(&mut tool, Input::Click(v(2.0, 3.0, 0.2))).unwrap();
    assert_positions(
        &w.model,
        &scaled(&cube_points(), DVec3::ZERO, v(2.0, 3.0, 1.0)),
    );
}

#[test]
fn a_corner_scales_uniformly_along_the_diagonal() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(1, 1, 1)).unwrap();
    w.run(&mut tool, Input::Move(v(2.0, 2.0, 2.0))).unwrap();
    assert_vec(tool.factors(), v(2.0, 2.0, 2.0));
    // Off the diagonal: projected onto it.
    w.run(&mut tool, Input::Move(v(2.0, 0.0, 0.0))).unwrap();
    let t = 2.0 / 3.0;
    assert_vec(tool.factors(), v(t, t, t));
    w.run(&mut tool, Input::Click(v(3.0, 0.0, 0.0))).unwrap();
    assert_positions(&w.model, &scaled(&cube_points(), DVec3::ZERO, DVec3::ONE));
}

#[test]
fn invert_uniform_swaps_the_defaults() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    tool.set_invert_uniform(true);
    w.grab(&mut tool, h(1, 1, 1)).unwrap();
    assert_eq!(tool.uniform(), Some(false));
    w.run(&mut tool, Input::Move(v(2.0, 3.0, 4.0))).unwrap();
    assert_vec(tool.factors(), v(2.0, 3.0, 4.0));

    // A face handle made uniform scales all three axes.
    w.grab(&mut tool, h(1, 0, 0)).unwrap();
    assert_eq!(tool.uniform(), Some(true));
    w.run(&mut tool, Input::Move(v(2.0, 7.0, 7.0))).unwrap();
    assert_vec(tool.factors(), v(2.0, 2.0, 2.0));
    w.run(&mut tool, Input::Click(v(2.0, 7.0, 7.0))).unwrap();
    assert_positions(
        &w.model,
        &scaled(&cube_points(), v(0.0, 0.5, 0.5), v(2.0, 2.0, 2.0)),
    );
}

#[test]
fn about_center_scales_about_the_box_center() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    tool.set_about_center(true);
    w.grab(&mut tool, h(1, 0, 0)).unwrap();
    assert_vec(tool.fixed_point(), v(0.5, 0.5, 0.5));
    w.run(&mut tool, Input::Move(v(1.5, 0.0, 0.0))).unwrap();
    assert_vec(tool.factors(), v(2.0, 1.0, 1.0));
    assert_bounds(tool.preview(), v(-0.5, 0.0, 0.0), v(1.5, 1.0, 1.0));
    w.run(&mut tool, Input::Click(v(1.5, 0.0, 0.0))).unwrap();
    assert_positions(
        &w.model,
        &scaled(&cube_points(), v(0.5, 0.5, 0.5), v(2.0, 1.0, 1.0)),
    );
}

#[test]
fn switches_change_the_drag_in_progress() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(1, 0, 0)).unwrap();
    w.run(&mut tool, Input::Move(v(3.0, 0.0, 0.0))).unwrap();
    assert_vec(tool.factors(), v(3.0, 1.0, 1.0));

    tool.set_about_center(true);
    assert_vec(tool.fixed_point(), v(0.5, 0.5, 0.5));
    assert_vec(tool.factors(), v(5.0, 1.0, 1.0));
    tool.set_about_center(false);
    assert_vec(tool.factors(), v(3.0, 1.0, 1.0));

    tool.set_invert_uniform(true);
    assert_eq!(tool.uniform(), Some(true));
    assert_vec(tool.factors(), v(3.0, 3.0, 3.0));
    tool.set_invert_uniform(false);
    assert_eq!(tool.uniform(), Some(false));
    assert_vec(tool.factors(), v(3.0, 1.0, 1.0));

    // The switches stay set for the next drag.
    tool.set_about_center(true);
    w.run(&mut tool, Input::Cancel).unwrap();
    w.grab(&mut tool, h(0, 0, 1)).unwrap();
    assert_vec(tool.fixed_point(), v(0.5, 0.5, 0.5));
}

#[test]
fn a_flat_corner_is_uniform() {
    let mut w = World::square();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(1, 1, 0)).unwrap();
    assert_eq!(tool.uniform(), Some(true));
    assert_vec(tool.fixed_point(), v(1.0, 0.0, 0.0));
    w.run(&mut tool, Input::Move(v(3.0, 0.2, 0.0))).unwrap();
    assert_vec(tool.factors(), v(1.1, 1.1, 1.1));
    w.run(&mut tool, Input::Click(v(3.0, 0.2, 0.0))).unwrap();
    assert_positions(
        &w.model,
        &scaled(&SQUARE, v(1.0, 0.0, 0.0), v(1.1, 1.1, 1.1)),
    );
    // An edge middle of the flat box scales one axis.
    w.grab(&mut tool, h(0, 1, 0)).unwrap();
    assert_eq!(tool.uniform(), Some(false));
}

#[test]
fn moves_and_clicks_while_picking_do_nothing() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.run(&mut tool, Input::Move(v(3.0, 0.0, 0.0))).unwrap();
    w.run(&mut tool, Input::Click(v(3.0, 0.0, 0.0))).unwrap();
    w.run(&mut tool, Input::Cancel).unwrap();
    assert_eq!(tool.stage(), ScaleStage::Pick);
    assert_eq!(w.undo.undo_count(), 0);
    assert_positions(&w.model, &cube_points());
}

#[test]
fn clicks_at_one_tiny_or_not_finite_scale_nothing() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(1, 0, 0)).unwrap();
    w.run(&mut tool, Input::Click(v(1.0, 0.3, 0.3))).unwrap();
    assert_eq!(tool.stage(), ScaleStage::Drag);
    assert_eq!(w.undo.undo_count(), 0);

    assert_eq!(
        w.run(&mut tool, Input::Click(v(0.0001, 0.0, 0.0))),
        Err(ScaleError::ZeroFactor)
    );
    assert_eq!(tool.stage(), ScaleStage::Drag);
    assert_vec(tool.factors(), v(0.0001, 1.0, 1.0));
    assert_eq!(w.undo.undo_count(), 0);

    assert_eq!(
        w.run(&mut tool, Input::Click(v(f64::NAN, 0.0, 0.0))),
        Err(ScaleError::NotFinite)
    );
    w.run(&mut tool, Input::Move(v(2.0, 0.0, 0.0))).unwrap();
    w.run(&mut tool, Input::Move(v(f64::INFINITY, 0.0, 0.0)))
        .unwrap();
    assert_vec(tool.factors(), v(2.0, 1.0, 1.0));
    assert_positions(&w.model, &cube_points());
}

#[test]
fn negative_factors_mirror_and_keep_faces_out() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(1, 0, 0)).unwrap();
    w.run(&mut tool, Input::Move(v(-1.0, 0.0, 0.0))).unwrap();
    assert_vec(tool.factors(), v(-1.0, 1.0, 1.0));
    assert_bounds(tool.preview(), v(-1.0, 0.0, 0.0), v(0.0, 1.0, 1.0));
    w.run(&mut tool, Input::Click(v(-1.0, 0.0, 0.0))).unwrap();
    assert_positions(
        &w.model,
        &scaled(&cube_points(), DVec3::ZERO, v(-1.0, 1.0, 1.0)),
    );
    assert!(normals_outward(&w.model));
    assert!(w.model.root().geometry().validate().is_empty());
}

// -- typed factors ---------------------------------------------------------------------

#[test]
fn typed_factors_follow_the_handle() {
    // (handle, invert_uniform, text, expected factors)
    let cases: [(Handle, bool, &str, DVec3); 7] = [
        (h(1, 1, 0), false, "2", v(2.0, 2.0, 1.0)),
        (h(1, 1, 0), false, "2,3", v(2.0, 3.0, 1.0)),
        (h(0, 1, 1), false, "2,3", v(1.0, 2.0, 3.0)),
        (h(-1, 0, 0), false, "0.5", v(0.5, 1.0, 1.0)),
        (h(1, 1, 1), false, "2", v(2.0, 2.0, 2.0)),
        (h(1, 1, 1), true, "2,3,4", v(2.0, 3.0, 4.0)),
        (h(1, 0, 0), true, "3", v(3.0, 3.0, 3.0)),
    ];
    for (handle, invert, text, want) in cases {
        let mut w = World::cube();
        let mut tool = ScaleTool::new();
        tool.set_invert_uniform(invert);
        w.grab(&mut tool, handle).unwrap();
        let f = tool.fixed_point().unwrap();
        w.run(&mut tool, typed(text)).unwrap();
        assert_eq!(tool.stage(), ScaleStage::Pick, "{handle:?} {text}");
        assert_eq!(w.undo.undo_count(), 1);
        assert_positions(&w.model, &scaled(&cube_points(), f, want));
        let got = tool.last_factors(&w.undo).unwrap();
        assert!(got.distance(want) < 1e-9, "{handle:?} {text}: {got:?}");
    }
}

#[test]
fn typed_factors_ignore_the_cursor() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(1, 0, 0)).unwrap();
    w.run(&mut tool, Input::Move(v(-4.0, 0.0, 0.0))).unwrap();
    w.run(&mut tool, typed("2")).unwrap();
    assert_positions(
        &w.model,
        &scaled(&cube_points(), DVec3::ZERO, v(2.0, 1.0, 1.0)),
    );
}

#[test]
fn typed_factors_with_the_wrong_count_are_refused() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    let cases: [(Handle, &str); 4] = [
        (h(1, 1, 0), "2,3,4"),
        (h(1, 0, 0), "2,3"),
        (h(1, 1, 1), "2,3"),
        (h(1, 1, 1), "2,3,4"),
    ];
    for (handle, text) in cases {
        w.grab(&mut tool, handle).unwrap();
        assert_eq!(
            w.run(&mut tool, typed(text)),
            Err(ScaleError::WrongCount),
            "{handle:?} {text}"
        );
        assert_eq!(tool.stage(), ScaleStage::Drag);
    }
    assert_eq!(w.undo.undo_count(), 0);
    assert_positions(&w.model, &cube_points());
}

#[test]
fn typed_zero_one_or_junk_changes_nothing() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(1, 1, 0)).unwrap();
    w.run(&mut tool, Input::Move(v(2.0, 2.0, 0.0))).unwrap();
    assert_eq!(w.run(&mut tool, typed("0")), Err(ScaleError::ZeroFactor));
    assert_eq!(
        w.run(&mut tool, typed("2,0.0001")),
        Err(ScaleError::ZeroFactor)
    );
    assert_eq!(
        w.run(&mut tool, typed("x")),
        Err(ScaleError::Parse(ParseError::Invalid))
    );
    assert_eq!(
        w.run(&mut tool, typed(" ")),
        Err(ScaleError::Parse(ParseError::Empty))
    );
    // Count is checked before zero.
    assert_eq!(
        w.run(&mut tool, typed("0,0,0")),
        Err(ScaleError::WrongCount)
    );
    w.run(&mut tool, typed("1")).unwrap();
    w.run(&mut tool, typed("1,1")).unwrap();
    assert_eq!(tool.stage(), ScaleStage::Drag);
    assert_vec(tool.factors(), v(2.0, 2.0, 1.0));
    assert_eq!(w.undo.undo_count(), 0);
    assert_positions(&w.model, &cube_points());
}

#[test]
fn typed_factors_while_picking_need_a_last_scale() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    assert_eq!(w.run(&mut tool, typed("2")), Err(ScaleError::NotStarted));
    assert_eq!(w.run(&mut tool, typed("junk")), Err(ScaleError::NotStarted));
    assert_eq!(w.undo.undo_count(), 0);
}

#[test]
fn typed_factors_right_after_change_the_last_scale() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(1, 0, 0)).unwrap();
    w.run(&mut tool, Input::Click(v(3.0, 0.0, 0.0))).unwrap();
    assert_vec(tool.last_factors(&w.undo), v(3.0, 1.0, 1.0));
    assert_eq!(w.undo.undo_count(), 1);

    w.run(&mut tool, typed("2")).unwrap();
    assert_positions(
        &w.model,
        &scaled(&cube_points(), DVec3::ZERO, v(2.0, 1.0, 1.0)),
    );
    assert_vec(tool.last_factors(&w.undo), v(2.0, 1.0, 1.0));
    assert_eq!(w.undo.undo_count(), 1);
    assert_eq!(w.undo.redo_count(), 0);

    w.run(&mut tool, typed("0.5")).unwrap();
    assert_positions(
        &w.model,
        &scaled(&cube_points(), DVec3::ZERO, v(0.5, 1.0, 1.0)),
    );
    assert_eq!(w.undo.undo_count(), 1);
    assert_eq!(w.run(&mut tool, typed("2,3")), Err(ScaleError::WrongCount));
    assert_eq!(w.run(&mut tool, typed("0")), Err(ScaleError::ZeroFactor));
    w.run(&mut tool, typed("1")).unwrap();
    assert_positions(
        &w.model,
        &scaled(&cube_points(), DVec3::ZERO, v(0.5, 1.0, 1.0)),
    );

    w.undo.undo(&mut w.model).unwrap();
    assert_positions(&w.model, &cube_points());
}

#[test]
fn typed_factors_right_after_keep_the_fixed_point_and_uniform() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    tool.set_about_center(true);
    w.grab(&mut tool, h(1, 1, 1)).unwrap();
    w.run(&mut tool, Input::Click(v(1.5, 1.5, 1.5))).unwrap();
    let c = v(0.5, 0.5, 0.5);
    assert_positions(&w.model, &scaled(&cube_points(), c, DVec3::splat(2.0)));
    // Switching now does not change the finished scale's fixed point or uniform.
    tool.set_about_center(false);
    tool.set_invert_uniform(true);
    assert_eq!(
        w.run(&mut tool, typed("3,3,3")),
        Err(ScaleError::WrongCount)
    );
    w.run(&mut tool, typed("3")).unwrap();
    assert_positions(&w.model, &scaled(&cube_points(), c, DVec3::splat(3.0)));
    assert_eq!(w.undo.undo_count(), 1);
}

#[test]
fn typed_factors_after_an_undo_a_grab_or_another_edit_are_not_started() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(1, 0, 0)).unwrap();
    w.run(&mut tool, Input::Click(v(3.0, 0.0, 0.0))).unwrap();
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(tool.last_factors(&w.undo), None);
    assert_eq!(w.run(&mut tool, typed("2")), Err(ScaleError::NotStarted));
    assert_positions(&w.model, &cube_points());

    w.undo.redo(&mut w.model).unwrap();
    assert_vec(tool.last_factors(&w.undo), v(3.0, 1.0, 1.0));
    w.grab(&mut tool, h(0, 1, 0)).unwrap();
    assert_eq!(tool.last_factors(&w.undo), None);
    w.run(&mut tool, Input::Cancel).unwrap();
    assert_eq!(tool.stage(), ScaleStage::Pick);
    assert_eq!(w.run(&mut tool, typed("2")), Err(ScaleError::NotStarted));
    let three = scaled(&cube_points(), DVec3::ZERO, v(3.0, 1.0, 1.0));
    assert_positions(&w.model, &three);

    // Another edit on top.
    w.grab(&mut tool, h(0, 0, 1)).unwrap();
    w.run(&mut tool, Input::Click(v(0.0, 0.0, 2.0))).unwrap();
    assert!(tool.last_factors(&w.undo).is_some());
    let sel = w.sel();
    modeler_kernel::transform::transform(
        &mut w.model,
        BodyId::Root,
        &sel,
        DMat4::from_translation(v(0.0, 0.0, 1.0)),
        &mut w.undo,
        "Move",
    )
    .unwrap();
    assert_eq!(tool.last_factors(&w.undo), None);
    assert_eq!(w.run(&mut tool, typed("2")), Err(ScaleError::NotStarted));
}

#[test]
fn typed_factors_after_refused_keep_the_old_scale() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(1, 0, 0)).unwrap();
    w.run(&mut tool, Input::Click(v(3.0, 0.0, 0.0))).unwrap();
    let three = scaled(&cube_points(), DVec3::ZERO, v(3.0, 1.0, 1.0));
    let bogus = [Entity::Face(FaceId(999))];
    assert_eq!(
        w.run_with(&mut tool, typed("2"), &bogus),
        Err(ScaleError::Transform(TransformError::MissingEntity(
            Entity::Face(FaceId(999))
        )))
    );
    assert_positions(&w.model, &three);
    assert_eq!(w.undo.undo_count(), 1);
    assert_eq!(w.undo.redo_count(), 0);
    assert_vec(tool.last_factors(&w.undo), v(3.0, 1.0, 1.0));
    assert_eq!(
        w.run_with(&mut tool, typed("2"), &[]),
        Err(ScaleError::Empty)
    );
    assert_positions(&w.model, &three);
    // Still changeable.
    w.run(&mut tool, typed("2")).unwrap();
    assert_positions(
        &w.model,
        &scaled(&cube_points(), DVec3::ZERO, v(2.0, 1.0, 1.0)),
    );
}

// -- refusals, cancel, queries ---------------------------------------------------------

#[test]
fn a_refused_scale_keeps_the_tool_state() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    let mut sel = w.sel();
    sel.push(Entity::Face(FaceId(999)));
    tool.grab(h(1, 0, 0), &w.model, BodyId::Root, &sel).unwrap();
    let r = w.run_with(&mut tool, Input::Click(v(2.0, 0.0, 0.0)), &sel);
    assert_eq!(
        r,
        Err(ScaleError::Transform(TransformError::MissingEntity(
            Entity::Face(FaceId(999))
        )))
    );
    assert_eq!(tool.stage(), ScaleStage::Drag);
    assert_eq!(tool.handle_grabbed(), Some(h(1, 0, 0)));
    assert_vec(tool.factors(), v(2.0, 1.0, 1.0));
    assert_eq!(w.undo.undo_count(), 0);
    // The same click with a real selection goes through.
    w.run(&mut tool, Input::Click(v(2.0, 0.0, 0.0))).unwrap();
    assert_positions(
        &w.model,
        &scaled(&cube_points(), DVec3::ZERO, v(2.0, 1.0, 1.0)),
    );
}

#[test]
fn cancel_drops_the_drag() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(1, 0, 0)).unwrap();
    w.run(&mut tool, Input::Move(v(3.0, 0.0, 0.0))).unwrap();
    w.run(&mut tool, Input::Cancel).unwrap();
    assert_eq!(tool.stage(), ScaleStage::Pick);
    assert_eq!(tool.handle_grabbed(), None);
    assert_eq!(tool.factors(), None);
    assert_eq!(tool.preview(), None);
    assert_eq!(w.undo.undo_count(), 0);
    assert_positions(&w.model, &cube_points());
}

#[test]
fn matrix_is_the_scale_the_click_would_apply() {
    let mut w = World::cube();
    let mut tool = ScaleTool::new();
    w.grab(&mut tool, h(0, 1, 1)).unwrap();
    w.run(&mut tool, Input::Move(v(0.0, 2.0, 3.0))).unwrap();
    let m = tool.matrix().unwrap();
    let f = v(0.5, 0.0, 0.0);
    let want = DMat4::from_translation(f)
        * DMat4::from_scale(v(1.0, 2.0, 3.0))
        * DMat4::from_translation(-f);
    assert!(m.abs_diff_eq(want, 1e-12), "{m:?}");
    assert!(m.transform_point3(f).distance(f) < 1e-12);
    assert_bounds(tool.preview(), DVec3::ZERO, v(1.0, 2.0, 3.0));
}

#[test]
fn a_handle_on_a_line_is_not_a_corner() {
    let w = World::square();
    let c = w.model.root().geometry();
    let edge = c
        .edges()
        .find(|(_, e)| {
            let a = c.vertex(e.ends[0]).unwrap().position;
            let b = c.vertex(e.ends[1]).unwrap().position;
            a.y == 0.0 && b.y == 0.0
        })
        .map(|(id, _)| id)
        .unwrap();
    let sel = [Entity::Edge(edge)];
    let mut tool = ScaleTool::new();
    tool.grab(h(1, 0, 0), &w.model, BodyId::Root, &sel).unwrap();
    assert_eq!(tool.uniform(), Some(false));
    assert_vec(tool.fixed_point(), v(1.0, 0.0, 0.0));
    tool.set_invert_uniform(true);
    assert_eq!(tool.uniform(), Some(true));
}
