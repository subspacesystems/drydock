// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Tool plumbing and the line tool (`docs/specs/m2-tool-line.md`).

use glam::DVec3;
use modeler_kernel::model::*;
use modeler_kernel::sticky::StickyError;
use modeler_kernel::tools::line::LineTool;
use modeler_kernel::tools::*;
use modeler_kernel::topology::*;
use modeler_kernel::units::{ParseError, Unit};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn assert_close(a: DVec3, b: DVec3) {
    assert!(a.distance(b) < 1e-9, "{a:?} != {b:?}");
}

struct World {
    model: Model,
    undo: UndoStack,
    body: BodyId,
    unit: Unit,
}

impl World {
    fn new() -> Self {
        World {
            model: Model::new(),
            undo: UndoStack::new(),
            body: BodyId::Root,
            unit: Unit::Meter,
        }
    }

    fn target(&mut self) -> Target<'_> {
        Target {
            model: &mut self.model,
            body: self.body,
            undo: &mut self.undo,
            unit: self.unit,
        }
    }

    fn ctx(&self) -> &Context {
        self.model.body(self.body).unwrap().geometry()
    }

    fn counts(&self) -> (usize, usize, usize) {
        let c = self.ctx();
        (c.vertices().count(), c.edges().count(), c.faces().count())
    }

    fn faces(&self) -> Vec<FaceId> {
        self.ctx().faces().map(|(f, _)| f).collect()
    }

    fn materials(&self, f: FaceId) -> FaceMaterials {
        self.model.body(self.body).unwrap().face_materials(f)
    }

    fn draw(
        &mut self,
        segments: &[(DVec3, DVec3)],
    ) -> Result<Vec<modeler_kernel::sticky::EdgeChain>, ToolError> {
        let mut t = self.target();
        draw_segments(&mut t, segments, "Draw")
    }

    fn line(&mut self, tool: &mut LineTool, input: Input) -> Result<(), ToolError> {
        let mut t = self.target();
        tool.handle(input, &mut t)
    }

    /// Two materials, painted front and back on face `f` as their own undo step.
    fn paint(&mut self, f: FaceId) -> FaceMaterials {
        let mut log = ModelEdit::new();
        let red = self.model.add_material(
            Material {
                name: "red".into(),
                color: [1.0, 0.0, 0.0, 1.0],
                texture: None,
            },
            &mut log,
        );
        let blue = self.model.add_material(
            Material {
                name: "blue".into(),
                color: [0.0, 0.0, 1.0, 1.0],
                texture: None,
            },
            &mut log,
        );
        let fm = FaceMaterials {
            front: Some(red),
            back: Some(blue),
            ..Default::default()
        };
        self.model
            .set_face_materials(self.body, f, fm, &mut log)
            .unwrap();
        self.undo.push("Paint", log);
        fm
    }

    fn undo_all(&mut self) {
        while self.undo.undo(&mut self.model).unwrap().is_some() {}
    }
}

fn square_segments(x0: f64, y0: f64, s: f64) -> Vec<(DVec3, DVec3)> {
    let p = [
        v(x0, y0, 0.0),
        v(x0 + s, y0, 0.0),
        v(x0 + s, y0 + s, 0.0),
        v(x0, y0 + s, 0.0),
    ];
    (0..4).map(|i| (p[i], p[(i + 1) % 4])).collect()
}

/// A unit square face at the origin, drawn as one step. Returns its face.
fn unit_square(w: &mut World) -> FaceId {
    w.draw(&square_segments(0.0, 0.0, 1.0)).unwrap();
    let faces = w.faces();
    assert_eq!(faces.len(), 1);
    faces[0]
}

// -- draw_segments ------------------------------------------------------------------

#[test]
fn draw_nothing_changes_nothing() {
    let mut w = World::new();
    let chains = w.draw(&[]).unwrap();
    assert!(chains.is_empty());
    assert_eq!(w.model, Model::new());
    assert_eq!(w.undo.undo_label(), None);
}

#[test]
fn draw_one_segment_is_one_labelled_step() {
    let mut w = World::new();
    let chains = w.draw(&[(v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0))]).unwrap();
    assert_eq!(chains.len(), 1);
    assert_eq!(chains[0].edges.len(), 1);
    assert_eq!(chains[0].vertices.len(), 2);
    assert_eq!(w.counts(), (2, 1, 0));
    assert_eq!(w.undo.undo_label(), Some("Draw"));
    assert_eq!(w.undo.undo(&mut w.model).unwrap(), Some("Draw".to_string()));
    assert_eq!(w.model, Model::new());
    assert_eq!(w.undo.undo_label(), None);
}

#[test]
fn draw_square_is_one_step_with_a_face() {
    let mut w = World::new();
    let chains = w.draw(&square_segments(0.0, 0.0, 1.0)).unwrap();
    assert_eq!(chains.len(), 4);
    assert!(chains[..3].iter().all(|c| c.faces.is_empty()));
    assert_eq!(chains[3].faces.len(), 1);
    assert_eq!(chains[3].faces[0].1, None);
    assert_eq!(w.counts(), (4, 4, 1));
    let after = w.model.clone();
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(w.model, Model::new());
    assert_eq!(w.undo.undo_label(), None);
    w.undo.redo(&mut w.model).unwrap();
    assert_eq!(w.model, after);
}

#[test]
fn draw_chains_follow_segment_order() {
    let mut w = World::new();
    let segs = [
        (v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0)),
        (v(1.0, 0.0, 0.0), v(1.0, 2.0, 0.0)),
    ];
    let chains = w.draw(&segs).unwrap();
    let ctx = w.ctx();
    for (chain, (a, b)) in chains.iter().zip(segs) {
        let first = ctx.vertex(chain.vertices[0]).unwrap().position;
        let last = ctx
            .vertex(*chain.vertices.last().unwrap())
            .unwrap()
            .position;
        assert_close(first, a);
        assert_close(last, b);
    }
    assert_eq!(chains[0].vertices[1], chains[1].vertices[0]);
}

#[test]
fn draw_failure_rolls_back_earlier_segments() {
    let mut w = World::new();
    w.draw(&[(v(5.0, 5.0, 0.0), v(6.0, 5.0, 0.0))]).unwrap();
    let before = w.model.clone();
    let mut segs = square_segments(0.0, 0.0, 1.0);
    segs.insert(2, (v(0.0, 0.0, 0.0), v(f64::NAN, 0.0, 0.0)));
    assert_eq!(
        w.draw(&segs),
        Err(ToolError::Geometry(StickyError::NotFinite))
    );
    assert_eq!(w.model, before);
    assert_eq!(w.undo.undo_label(), Some("Draw"));
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(w.undo.undo_label(), None, "the failed call pushed nothing");
}

#[test]
fn draw_degenerate_segment_fails() {
    let mut w = World::new();
    let segs = [
        (v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0)),
        (v(1.0, 0.0, 0.0), v(1.0, 0.0, 0.0)),
    ];
    assert_eq!(
        w.draw(&segs),
        Err(ToolError::Geometry(StickyError::Degenerate))
    );
    assert_eq!(w.model, Model::new());
    assert_eq!(w.undo.undo_label(), None);
}

#[test]
fn draw_into_missing_body_fails() {
    let mut w = World::new();
    w.body = BodyId::Definition(DefinitionId(99));
    assert_eq!(
        w.draw(&[(v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0))]),
        Err(ToolError::Model(ModelError::MissingBody(
            BodyId::Definition(DefinitionId(99))
        )))
    );
    assert_eq!(w.model, Model::new());
    assert_eq!(w.undo.undo_label(), None);
}

#[test]
fn draw_into_a_definition_body() {
    let mut w = World::new();
    let mut log = ModelEdit::new();
    let d = w.model.add_definition(
        DefinitionInfo {
            name: "Thing".into(),
            group: false,
        },
        &mut log,
    );
    w.undo.push("Definition", log);
    w.body = BodyId::Definition(d);
    w.draw(&square_segments(0.0, 0.0, 1.0)).unwrap();
    assert_eq!(w.counts(), (4, 4, 1));
    assert!(w.model.root().geometry().vertices().next().is_none());
    assert_eq!(w.undo.undo_label(), Some("Draw"));
}

#[test]
fn split_face_copies_materials_to_both_halves() {
    let mut w = World::new();
    let f = unit_square(&mut w);
    let fm = w.paint(f);
    let before = w.model.clone();
    let chains = w.draw(&[(v(0.5, 0.0, 0.0), v(0.5, 1.0, 0.0))]).unwrap();
    assert_eq!(chains[0].faces.len(), 2);
    assert!(chains[0].faces.iter().all(|(_, o)| *o == Some(f)));
    let faces = w.faces();
    assert_eq!(faces.len(), 2);
    assert!(!faces.contains(&f));
    for g in faces {
        assert_eq!(w.materials(g), fm, "face {g:?}");
    }
    // the copy is part of the same undo step
    assert_eq!(w.undo.undo_label(), Some("Draw"));
    w.undo.undo(&mut w.model).unwrap();
    assert_eq!(w.model, before);
    assert_eq!(w.undo.undo_label(), Some("Paint"));
}

#[test]
fn second_split_in_one_call_copies_from_a_new_face() {
    let mut w = World::new();
    let f = unit_square(&mut w);
    let fm = w.paint(f);
    w.draw(&[
        (v(0.5, 0.0, 0.0), v(0.5, 1.0, 0.0)),
        (v(0.0, 0.5, 0.0), v(0.5, 0.5, 0.0)),
    ])
    .unwrap();
    let faces = w.faces();
    assert_eq!(faces.len(), 3);
    for g in faces {
        assert_eq!(w.materials(g), fm, "face {g:?}");
    }
}

#[test]
fn hole_and_inner_face_both_get_materials() {
    let mut w = World::new();
    w.draw(&square_segments(0.0, 0.0, 4.0)).unwrap();
    let f = w.faces()[0];
    let fm = w.paint(f);
    w.draw(&square_segments(1.0, 1.0, 1.0)).unwrap();
    let faces = w.faces();
    assert_eq!(faces.len(), 2);
    for g in faces {
        assert_eq!(w.materials(g), fm, "face {g:?}");
    }
}

#[test]
fn faces_over_empty_space_get_no_materials() {
    let mut w = World::new();
    let f = unit_square(&mut w);
    w.paint(f);
    w.draw(&square_segments(3.0, 0.0, 1.0)).unwrap();
    let faces = w.faces();
    assert_eq!(faces.len(), 2);
    assert_eq!(w.materials(faces[1]), FaceMaterials::default());
    // a face sharing an edge with the painted one is not split from it either
    w.draw(&[
        (v(1.0, 0.0, 0.0), v(2.0, 0.0, 0.0)),
        (v(2.0, 0.0, 0.0), v(2.0, 1.0, 0.0)),
        (v(2.0, 1.0, 0.0), v(1.0, 1.0, 0.0)),
    ])
    .unwrap();
    let faces = w.faces();
    assert_eq!(faces.len(), 3);
    assert_eq!(w.materials(faces[2]), FaceMaterials::default());
}

#[test]
fn unpainted_split_leaves_default_materials() {
    let mut w = World::new();
    unit_square(&mut w);
    w.draw(&[(v(0.5, 0.0, 0.0), v(0.5, 1.0, 0.0))]).unwrap();
    for g in w.faces() {
        assert_eq!(w.materials(g), FaceMaterials::default());
    }
    assert_eq!(
        w.model.root().face_material_entries().count(),
        0,
        "no entries written for default materials"
    );
}

// -- line tool: state ---------------------------------------------------------------

#[test]
fn new_tool_is_idle() {
    let t = LineTool::new();
    assert_eq!(t.anchor(), None);
    assert_eq!(t.preview(), None);
    assert_eq!(t.length(), None);
}

#[test]
fn move_while_idle_does_nothing() {
    let mut w = World::new();
    let mut t = LineTool::new();
    assert_eq!(w.line(&mut t, Input::Move(v(1.0, 2.0, 0.0))), Ok(()));
    assert_eq!(t.anchor(), None);
    assert_eq!(t.preview(), None);
    assert_eq!(w.model, Model::new());
}

#[test]
fn first_click_starts_without_touching_the_model() {
    let mut w = World::new();
    let mut t = LineTool::new();
    assert_eq!(w.line(&mut t, Input::Click(v(1.0, 2.0, 3.0))), Ok(()));
    assert_eq!(t.anchor(), Some(v(1.0, 2.0, 3.0)));
    assert_eq!(t.preview(), None);
    assert_eq!(t.length(), None);
    assert_eq!(w.model, Model::new());
    assert_eq!(w.undo.undo_label(), None);
}

#[test]
fn move_while_drawing_previews() {
    let mut w = World::new();
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Move(v(3.0, 4.0, 0.0))).unwrap();
    assert_eq!(t.preview(), Some((v(0.0, 0.0, 0.0), v(3.0, 4.0, 0.0))));
    assert!((t.length().unwrap() - 5.0).abs() < 1e-12);
    w.line(&mut t, Input::Move(v(1.0, 0.0, 0.0))).unwrap();
    assert_eq!(t.preview(), Some((v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0))));
    assert_eq!(w.model, Model::new());
}

#[test]
fn preview_needs_distance() {
    let mut w = World::new();
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Move(v(5e-6, 0.0, 0.0))).unwrap();
    assert_eq!(t.preview(), None);
    assert_eq!(t.length(), None);
    assert_eq!(t.anchor(), Some(v(0.0, 0.0, 0.0)));
}

#[test]
fn non_finite_move_is_ignored() {
    let mut w = World::new();
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Move(v(1.0, 0.0, 0.0))).unwrap();
    assert_eq!(w.line(&mut t, Input::Move(v(f64::NAN, 0.0, 0.0))), Ok(()));
    assert_eq!(
        w.line(&mut t, Input::Move(v(0.0, f64::INFINITY, 0.0))),
        Ok(())
    );
    assert_eq!(t.preview(), Some((v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0))));
}

#[test]
fn non_finite_click_is_refused() {
    let mut w = World::new();
    let mut t = LineTool::new();
    assert_eq!(
        w.line(&mut t, Input::Click(v(f64::NAN, 0.0, 0.0))),
        Err(ToolError::Geometry(StickyError::NotFinite))
    );
    assert_eq!(t.anchor(), None);
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Move(v(1.0, 0.0, 0.0))).unwrap();
    assert_eq!(
        w.line(&mut t, Input::Click(v(0.0, f64::NEG_INFINITY, 0.0))),
        Err(ToolError::Geometry(StickyError::NotFinite))
    );
    assert_eq!(t.anchor(), Some(v(0.0, 0.0, 0.0)));
    assert_eq!(t.preview(), Some((v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0))));
    assert_eq!(w.model, Model::new());
}

#[test]
fn click_on_the_start_point_does_nothing() {
    let mut w = World::new();
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Move(v(2.0, 0.0, 0.0))).unwrap();
    assert_eq!(w.line(&mut t, Input::Click(v(5e-6, 0.0, 0.0))), Ok(()));
    assert_eq!(t.anchor(), Some(v(0.0, 0.0, 0.0)));
    assert_eq!(t.preview(), Some((v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0))));
    assert_eq!(w.model, Model::new());
    assert_eq!(w.undo.undo_label(), None);
}

#[test]
fn cancel_goes_idle_and_keeps_edges() {
    let mut w = World::new();
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Move(v(1.0, 1.0, 0.0))).unwrap();
    let after = w.model.clone();
    assert_eq!(w.line(&mut t, Input::Cancel), Ok(()));
    assert_eq!(t.anchor(), None);
    assert_eq!(t.preview(), None);
    assert_eq!(w.model, after);
    // the next click starts a new chain
    w.line(&mut t, Input::Click(v(5.0, 5.0, 0.0))).unwrap();
    assert_eq!(t.anchor(), Some(v(5.0, 5.0, 0.0)));
    assert_eq!(w.counts(), (2, 1, 0));
}

#[test]
fn cancel_while_idle_is_fine() {
    let mut w = World::new();
    let mut t = LineTool::new();
    assert_eq!(w.line(&mut t, Input::Cancel), Ok(()));
    assert_eq!(t.anchor(), None);
}

// -- line tool: clicks --------------------------------------------------------------

#[test]
fn second_click_draws_an_edge_and_continues() {
    let mut w = World::new();
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Move(v(1.0, 0.0, 0.0))).unwrap();
    assert_eq!(w.line(&mut t, Input::Click(v(1.0, 0.0, 0.0))), Ok(()));
    assert_eq!(w.counts(), (2, 1, 0));
    assert_eq!(t.anchor(), Some(v(1.0, 0.0, 0.0)));
    assert_eq!(t.preview(), None, "no cursor after placing");
    assert_eq!(w.undo.undo_label(), Some("Line"));
}

#[test]
fn each_edge_is_its_own_undo_step() {
    let mut w = World::new();
    let mut t = LineTool::new();
    for p in [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(2.0, 1.0, 0.0),
    ] {
        w.line(&mut t, Input::Click(p)).unwrap();
    }
    assert_eq!(w.counts(), (4, 3, 0));
    assert_eq!(t.anchor(), Some(v(2.0, 1.0, 0.0)));
    let mut steps = 0;
    while let Some(label) = w.undo.undo(&mut w.model).unwrap() {
        assert_eq!(label, "Line");
        steps += 1;
    }
    assert_eq!(steps, 3);
    assert_eq!(w.model, Model::new());
}

#[test]
fn closing_a_face_ends_the_chain() {
    let mut w = World::new();
    let mut t = LineTool::new();
    for p in [v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0)] {
        w.line(&mut t, Input::Click(p)).unwrap();
    }
    assert_eq!(t.anchor(), Some(v(0.0, 1.0, 0.0)));
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    assert_eq!(w.counts(), (3, 3, 1));
    assert_eq!(t.anchor(), None);
    assert_eq!(t.preview(), None);
    // the next click starts again
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    assert_eq!(t.anchor(), Some(v(0.0, 0.0, 0.0)));
    assert_eq!(w.counts(), (3, 3, 1));
}

#[test]
fn chain_continues_from_the_snapped_vertex() {
    let mut w = World::new();
    w.draw(&[(v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0))]).unwrap();
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    // within LINEAR_TOL of the existing vertex at (1, 0, 0)
    w.line(&mut t, Input::Click(v(1.0 + 4e-6, 3e-6, 0.0)))
        .unwrap();
    assert_eq!(t.anchor(), Some(v(1.0, 0.0, 0.0)));
    assert_eq!(w.counts(), (3, 2, 0));
}

#[test]
fn chain_continues_from_a_point_on_an_edge() {
    let mut w = World::new();
    w.draw(&[(v(1.0, -1.0, 0.0), v(1.0, 1.0, 0.0))]).unwrap();
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    assert_close(t.anchor().unwrap(), v(1.0, 0.0, 0.0));
    assert_eq!(w.counts(), (4, 3, 0), "the crossed edge is split");
}

#[test]
fn line_across_a_painted_face_splits_it_and_ends() {
    let mut w = World::new();
    let f = unit_square(&mut w);
    let fm = w.paint(f);
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.5, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Click(v(0.5, 1.0, 0.0))).unwrap();
    let faces = w.faces();
    assert_eq!(faces.len(), 2);
    for g in faces {
        assert_eq!(w.materials(g), fm);
    }
    assert_eq!(t.anchor(), None, "the edge closed faces");
    assert_eq!(w.undo.undo_label(), Some("Line"));
}

#[test]
fn line_tool_draws_into_its_target_body() {
    let mut w = World::new();
    let mut log = ModelEdit::new();
    let d = w.model.add_definition(
        DefinitionInfo {
            name: "Thing".into(),
            group: true,
        },
        &mut log,
    );
    w.body = BodyId::Definition(d);
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 1.0))).unwrap();
    assert_eq!(w.counts(), (2, 1, 0));
    assert!(w.model.root().geometry().edges().next().is_none());
}

#[test]
fn placing_into_a_missing_body_fails_and_keeps_state() {
    let mut w = World::new();
    w.body = BodyId::Definition(DefinitionId(7));
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Move(v(1.0, 0.0, 0.0))).unwrap();
    assert_eq!(
        w.line(&mut t, Input::Click(v(1.0, 0.0, 0.0))),
        Err(ToolError::Model(ModelError::MissingBody(
            BodyId::Definition(DefinitionId(7))
        )))
    );
    assert_eq!(t.anchor(), Some(v(0.0, 0.0, 0.0)));
    assert_eq!(t.preview(), Some((v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0))));
    assert_eq!(w.model, Model::new());
    assert_eq!(w.undo.undo_label(), None);
}

// -- line tool: typed lengths -------------------------------------------------------

#[test]
fn typed_while_idle_is_not_started() {
    let mut w = World::new();
    let mut t = LineTool::new();
    assert_eq!(
        w.line(&mut t, Input::Typed("2".into())),
        Err(ToolError::NotStarted)
    );
    assert_eq!(
        w.line(&mut t, Input::Typed("rubbish".into())),
        Err(ToolError::NotStarted),
        "not parsed while idle"
    );
    assert_eq!(t.anchor(), None);
}

#[test]
fn typed_parse_errors_come_first() {
    let mut w = World::new();
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    assert_eq!(
        w.line(&mut t, Input::Typed("rubbish".into())),
        Err(ToolError::Parse(ParseError::Invalid))
    );
    assert_eq!(
        w.line(&mut t, Input::Typed("   ".into())),
        Err(ToolError::Parse(ParseError::Empty))
    );
    assert_eq!(t.anchor(), Some(v(0.0, 0.0, 0.0)));
}

#[test]
fn typed_without_cursor_has_no_direction() {
    let mut w = World::new();
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    assert_eq!(
        w.line(&mut t, Input::Typed("2".into())),
        Err(ToolError::NoDirection)
    );
    w.line(&mut t, Input::Move(v(1e-6, 0.0, 0.0))).unwrap();
    assert_eq!(
        w.line(&mut t, Input::Typed("2".into())),
        Err(ToolError::NoDirection)
    );
    assert_eq!(w.model, Model::new());
    assert_eq!(t.anchor(), Some(v(0.0, 0.0, 0.0)));
}

#[test]
fn typed_length_goes_toward_the_cursor() {
    let mut w = World::new();
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(1.0, 1.0, 0.0))).unwrap();
    w.line(&mut t, Input::Move(v(1.3, 1.4, 0.0))).unwrap();
    assert_eq!(w.line(&mut t, Input::Typed("2".into())), Ok(()));
    let end = v(1.0 + 1.2, 1.0 + 1.6, 0.0);
    assert_close(t.anchor().unwrap(), end);
    assert_eq!(t.preview(), None);
    assert_eq!(w.counts(), (2, 1, 0));
    assert_eq!(w.undo.undo_label(), Some("Line"));
    let ctx = w.ctx();
    assert!(ctx.vertices().any(|(_, x)| x.position.distance(end) < 1e-9));
}

#[test]
fn negative_typed_length_goes_the_other_way() {
    let mut w = World::new();
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Move(v(0.0, 0.0, 5.0))).unwrap();
    w.line(&mut t, Input::Typed("-1.5".into())).unwrap();
    assert_close(t.anchor().unwrap(), v(0.0, 0.0, -1.5));
}

#[test]
fn typed_length_uses_the_target_unit() {
    let mut w = World::new();
    w.unit = Unit::Foot;
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Move(v(1.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Typed("2".into())).unwrap();
    assert_close(t.anchor().unwrap(), v(0.6096, 0.0, 0.0));
    w.line(&mut t, Input::Move(v(0.6096, 1.0, 0.0))).unwrap();
    w.line(&mut t, Input::Typed("100mm".into())).unwrap();
    assert_close(t.anchor().unwrap(), v(0.6096, 0.1, 0.0));
    w.unit = Unit::Millimeter;
    w.line(&mut t, Input::Move(v(2.0, 0.1, 0.0))).unwrap();
    w.line(&mut t, Input::Typed("5'".into())).unwrap();
    assert_close(t.anchor().unwrap(), v(0.6096 + 1.524, 0.1, 0.0));
    assert_eq!(w.counts(), (4, 3, 0));
}

#[test]
fn zero_typed_length_is_degenerate() {
    let mut w = World::new();
    let mut t = LineTool::new();
    w.line(&mut t, Input::Click(v(0.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Move(v(1.0, 0.0, 0.0))).unwrap();
    assert_eq!(
        w.line(&mut t, Input::Typed("0".into())),
        Err(ToolError::Geometry(StickyError::Degenerate))
    );
    assert_eq!(t.anchor(), Some(v(0.0, 0.0, 0.0)));
    assert_eq!(t.preview(), Some((v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0))));
    assert_eq!(w.model, Model::new());
    assert_eq!(w.undo.undo_label(), None);
}

#[test]
fn typed_length_can_close_a_face() {
    let mut w = World::new();
    let mut t = LineTool::new();
    for p in [v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(1.0, 1.0, 0.0)] {
        w.line(&mut t, Input::Click(p)).unwrap();
    }
    w.line(&mut t, Input::Move(v(0.0, 1.0, 0.0))).unwrap();
    w.line(&mut t, Input::Typed("1".into())).unwrap();
    w.line(&mut t, Input::Move(v(0.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Typed("1".into())).unwrap();
    assert_eq!(w.counts(), (4, 4, 1));
    assert_eq!(t.anchor(), None);
}

#[test]
fn undo_all_after_a_session_is_empty() {
    let mut w = World::new();
    let mut t = LineTool::new();
    for p in [
        v(0.0, 0.0, 0.0),
        v(2.0, 0.0, 0.0),
        v(2.0, 2.0, 0.0),
        v(0.0, 2.0, 0.0),
        v(0.0, 0.0, 0.0),
    ] {
        w.line(&mut t, Input::Click(p)).unwrap();
    }
    let f = w.faces()[0];
    w.paint(f);
    w.line(&mut t, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    w.line(&mut t, Input::Click(v(1.0, 2.0, 0.0))).unwrap();
    assert_eq!(w.faces().len(), 2);
    assert!(w.model.validate().is_empty());
    w.undo_all();
    assert_eq!(w.model, Model::new());
}
