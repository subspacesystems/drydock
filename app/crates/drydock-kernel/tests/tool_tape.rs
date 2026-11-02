// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Tape tool state machine (`docs/specs/m3-tool-tape.md`).

use drydock_kernel::model::*;
use drydock_kernel::tape::{Guide, measure, parallel_guide, point_guide};
use drydock_kernel::tools::tape::*;
use drydock_kernel::tools::{Input, Target};
use drydock_kernel::units::{ParseError, Unit};
use glam::DVec3;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// An empty model and its undo stack.
struct World {
    model: Model,
    undo: UndoStack,
}

impl World {
    fn new() -> World {
        World {
            model: Model::new(),
            undo: UndoStack::new(),
        }
    }

    fn run(&mut self, tool: &mut TapeTool, input: Input) -> Result<(), TapeError> {
        let mut target = Target {
            model: &mut self.model,
            body: BodyId::Root,
            undo: &mut self.undo,
            unit: Unit::Inch,
        };
        tool.handle(input, &mut target)
    }

    fn guides(&self) -> Vec<Guide> {
        self.model
            .root()
            .guides()
            .iter()
            .map(|(_, guide)| *guide)
            .collect()
    }
}

const A: DVec3 = DVec3::new(0.0, 0.0, 0.0);
const B: DVec3 = DVec3::new(4.0, 0.0, 0.0);

/// A tool that has started on the edge A-B at (1, 0, 0).
fn parallel_tool(w: &mut World) -> TapeTool {
    let mut tool = TapeTool::new();
    tool.set_hover_edge(Some((A, B)));
    w.run(&mut tool, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    tool
}

fn assert_guide_eq(got: &Guide, want: &Guide) {
    match (got, want) {
        (Guide::Line { point: p, dir: d }, Guide::Line { point: q, dir: e }) => {
            assert!(
                p.distance(*q) < 1e-9 && d.distance(*e) < 1e-9,
                "{got:?} vs {want:?}"
            );
        }
        (Guide::Point(p), Guide::Point(q)) => assert!(p.distance(*q) < 1e-9, "{got:?} vs {want:?}"),
        _ => panic!("{got:?} vs {want:?}"),
    }
}

#[test]
fn new_tool_is_at_start() {
    let tool = TapeTool::new();
    assert_eq!(tool.stage(), TapeStage::Start);
    assert_eq!(tool.start(), None);
    assert_eq!(tool.edge(), None);
    assert_eq!(tool.distance(), None);
    assert_eq!(tool.last_distance(), None);
    assert_eq!(tool.preview(), None);
}

#[test]
fn plain_measurement_reads_distance_and_edits_nothing() {
    let mut w = World::new();
    let mut tool = TapeTool::new();
    w.run(&mut tool, Input::Click(v(1.0, 1.0, 0.0))).unwrap();
    assert_eq!(tool.stage(), TapeStage::End);
    assert_eq!(tool.start(), Some(v(1.0, 1.0, 0.0)));
    assert_eq!(tool.edge(), None);
    assert_eq!(tool.distance(), None);
    w.run(&mut tool, Input::Move(v(4.0, 5.0, 0.0))).unwrap();
    assert!((tool.distance().unwrap() - 5.0).abs() < 1e-12);
    assert_eq!(tool.preview(), None);
    w.run(&mut tool, Input::Click(v(1.0, 1.0, 12.0))).unwrap();
    assert_eq!(tool.stage(), TapeStage::Start);
    assert!((tool.last_distance().unwrap() - 12.0).abs() < 1e-12);
    assert_eq!(tool.start(), None);
    assert_eq!(tool.distance(), None);
    assert_eq!(w.undo.undo_count(), 0);
    assert!(w.guides().is_empty());
}

#[test]
fn measurement_matches_kernel_measure() {
    let mut w = World::new();
    let mut tool = TapeTool::new();
    let (p, q) = (v(0.3, -2.0, 1.5), v(-4.0, 0.25, 9.0));
    w.run(&mut tool, Input::Click(p)).unwrap();
    w.run(&mut tool, Input::Click(q)).unwrap();
    assert_eq!(tool.last_distance(), Some(measure(p, q)));
}

#[test]
fn typed_length_on_plain_measurement_is_refused() {
    let mut w = World::new();
    let mut tool = TapeTool::new();
    w.run(&mut tool, Input::Click(A)).unwrap();
    w.run(&mut tool, Input::Move(B)).unwrap();
    assert_eq!(
        w.run(&mut tool, Input::Typed("2".into())),
        Err(TapeError::NotParallel)
    );
    assert_eq!(tool.stage(), TapeStage::End);
}

#[test]
fn typed_at_start_is_not_started() {
    let mut w = World::new();
    let mut tool = TapeTool::new();
    assert_eq!(
        w.run(&mut tool, Input::Typed("2".into())),
        Err(TapeError::NotStarted)
    );
}

#[test]
fn non_finite_points_are_refused_everywhere() {
    let mut w = World::new();
    let mut tool = TapeTool::new();
    let bad = v(f64::NAN, 0.0, 0.0);
    assert_eq!(
        w.run(&mut tool, Input::Move(bad)),
        Err(TapeError::NotFinite)
    );
    assert_eq!(
        w.run(&mut tool, Input::Click(bad)),
        Err(TapeError::NotFinite)
    );
    assert_eq!(tool.stage(), TapeStage::Start);
    w.run(&mut tool, Input::Click(A)).unwrap();
    assert_eq!(
        w.run(&mut tool, Input::Move(bad)),
        Err(TapeError::NotFinite)
    );
    assert_eq!(
        w.run(&mut tool, Input::Click(bad)),
        Err(TapeError::NotFinite)
    );
    assert_eq!(tool.stage(), TapeStage::End);
    assert_eq!(tool.distance(), None);
}

#[test]
fn cancel_drops_the_measurement_but_keeps_the_last_distance() {
    let mut w = World::new();
    let mut tool = TapeTool::new();
    w.run(&mut tool, Input::Click(A)).unwrap();
    w.run(&mut tool, Input::Click(B)).unwrap();
    w.run(&mut tool, Input::Click(A)).unwrap();
    w.run(&mut tool, Input::Move(v(1.0, 0.0, 0.0))).unwrap();
    w.run(&mut tool, Input::Cancel).unwrap();
    assert_eq!(tool.stage(), TapeStage::Start);
    assert_eq!(tool.start(), None);
    assert_eq!(tool.last_distance(), Some(4.0));
    w.run(&mut tool, Input::Cancel).unwrap();
    assert_eq!(tool.stage(), TapeStage::Start);
}

#[test]
fn start_on_an_edge_makes_a_parallel_measurement() {
    let mut w = World::new();
    let tool = parallel_tool(&mut w);
    assert_eq!(tool.stage(), TapeStage::End);
    assert_eq!(tool.start(), Some(v(1.0, 0.0, 0.0)));
    assert_eq!(tool.edge(), Some((A, B)));
}

#[test]
fn hover_edge_is_forgotten_after_the_start_click() {
    let mut w = World::new();
    let mut tool = parallel_tool(&mut w);
    w.run(&mut tool, Input::Cancel).unwrap();
    w.run(&mut tool, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    assert_eq!(tool.edge(), None);
}

#[test]
fn degenerate_hover_edge_gives_a_plain_measurement() {
    let mut w = World::new();
    let mut tool = TapeTool::new();
    tool.set_hover_edge(Some((A, A)));
    w.run(&mut tool, Input::Click(A)).unwrap();
    assert_eq!(tool.edge(), None);
}

#[test]
fn cleared_hover_edge_gives_a_plain_measurement() {
    let mut w = World::new();
    let mut tool = TapeTool::new();
    tool.set_hover_edge(Some((A, B)));
    tool.set_hover_edge(None);
    w.run(&mut tool, Input::Click(A)).unwrap();
    assert_eq!(tool.edge(), None);
}

#[test]
fn parallel_distance_is_the_offset_from_the_edge_line() {
    let mut w = World::new();
    let mut tool = parallel_tool(&mut w);
    assert_eq!(tool.distance(), None);
    assert_eq!(tool.preview(), None);
    // Far along the edge's line and 3 to the side: only the side counts.
    w.run(&mut tool, Input::Move(v(10.0, 3.0, 0.0))).unwrap();
    assert!((tool.distance().unwrap() - 3.0).abs() < 1e-12);
    w.run(&mut tool, Input::Move(v(2.0, 3.0, 4.0))).unwrap();
    assert!((tool.distance().unwrap() - 5.0).abs() < 1e-12);
}

#[test]
fn parallel_preview_is_the_guide_through_the_cursor() {
    let mut w = World::new();
    let mut tool = parallel_tool(&mut w);
    let cursor = v(2.0, 3.0, 0.0);
    w.run(&mut tool, Input::Move(cursor)).unwrap();
    let want = parallel_guide(A, B, cursor, None).unwrap();
    assert_guide_eq(&tool.preview().unwrap(), &want);
    assert!(w.guides().is_empty());
    assert_eq!(w.undo.undo_count(), 0);
}

#[test]
fn parallel_click_leaves_a_guide_as_one_undo_step() {
    let mut w = World::new();
    let mut tool = parallel_tool(&mut w);
    let cursor = v(2.0, 0.0, 3.0);
    w.run(&mut tool, Input::Move(cursor)).unwrap();
    w.run(&mut tool, Input::Click(cursor)).unwrap();
    assert_eq!(tool.stage(), TapeStage::Start);
    assert_eq!(tool.edge(), None);
    assert!((tool.last_distance().unwrap() - 3.0).abs() < 1e-12);
    let guides = w.guides();
    assert_eq!(guides.len(), 1);
    assert_guide_eq(&guides[0], &parallel_guide(A, B, cursor, None).unwrap());
    assert_eq!(w.undo.undo_count(), 1);
    assert_eq!(w.undo.undo_label(), Some("Guide"));
    w.undo.undo(&mut w.model).unwrap();
    assert!(w.guides().is_empty());
    w.undo.redo(&mut w.model).unwrap();
    assert_eq!(w.guides().len(), 1);
}

#[test]
fn parallel_click_uses_the_clicked_point_not_the_last_move() {
    let mut w = World::new();
    let mut tool = parallel_tool(&mut w);
    w.run(&mut tool, Input::Move(v(2.0, 1.0, 0.0))).unwrap();
    let click = v(2.0, 6.0, 0.0);
    w.run(&mut tool, Input::Click(click)).unwrap();
    assert_guide_eq(&w.guides()[0], &parallel_guide(A, B, click, None).unwrap());
    assert!((tool.last_distance().unwrap() - 6.0).abs() < 1e-12);
}

#[test]
fn parallel_click_on_the_edge_line_is_refused() {
    let mut w = World::new();
    let mut tool = parallel_tool(&mut w);
    assert_eq!(
        w.run(&mut tool, Input::Click(v(3.0, 0.0, 0.0))),
        Err(TapeError::NoDirection)
    );
    assert_eq!(tool.stage(), TapeStage::End);
    assert_eq!(tool.start(), Some(v(1.0, 0.0, 0.0)));
    assert_eq!(w.undo.undo_count(), 0);
    assert!(w.guides().is_empty());
}

#[test]
fn typed_offset_uses_the_cursor_side() {
    let mut w = World::new();
    let mut tool = parallel_tool(&mut w);
    // The cursor is on the -y side; the typed 2 in is 0.0508 m.
    let cursor = v(2.0, -0.25, 0.0);
    w.run(&mut tool, Input::Move(cursor)).unwrap();
    w.run(&mut tool, Input::Typed("2".into())).unwrap();
    assert_eq!(tool.stage(), TapeStage::Start);
    let want = parallel_guide(A, B, cursor, Some(0.0508)).unwrap();
    assert_guide_eq(&w.guides()[0], &want);
    assert!((tool.last_distance().unwrap() - 0.0508).abs() < 1e-12);
    assert_eq!(w.undo.undo_count(), 1);
    assert_eq!(w.undo.undo_label(), Some("Guide"));
}

#[test]
fn typed_offset_reads_units() {
    let mut w = World::new();
    let mut tool = parallel_tool(&mut w);
    w.run(&mut tool, Input::Move(v(2.0, 1.0, 0.0))).unwrap();
    w.run(&mut tool, Input::Typed("50cm".into())).unwrap();
    assert_guide_eq(
        &w.guides()[0],
        &Guide::Line {
            point: v(0.0, 0.5, 0.0),
            dir: v(1.0, 0.0, 0.0),
        },
    );
}

#[test]
fn typed_offset_errors_change_nothing() {
    let mut w = World::new();
    let mut tool = parallel_tool(&mut w);
    // No cursor yet.
    assert_eq!(
        w.run(&mut tool, Input::Typed("2".into())),
        Err(TapeError::NoDirection)
    );
    w.run(&mut tool, Input::Move(v(2.0, 1.0, 0.0))).unwrap();
    assert_eq!(
        w.run(&mut tool, Input::Typed("abc".into())),
        Err(TapeError::Parse(ParseError::Invalid))
    );
    assert_eq!(
        w.run(&mut tool, Input::Typed("  ".into())),
        Err(TapeError::Parse(ParseError::Empty))
    );
    assert_eq!(
        w.run(&mut tool, Input::Typed("0".into())),
        Err(TapeError::BadLength)
    );
    assert_eq!(
        w.run(&mut tool, Input::Typed("-3".into())),
        Err(TapeError::BadLength)
    );
    // The cursor on the edge's line gives no side.
    w.run(&mut tool, Input::Move(v(3.0, 0.0, 0.0))).unwrap();
    assert_eq!(
        w.run(&mut tool, Input::Typed("2".into())),
        Err(TapeError::NoDirection)
    );
    assert_eq!(tool.stage(), TapeStage::End);
    assert_eq!(w.undo.undo_count(), 0);
    assert!(w.guides().is_empty());
}

#[test]
fn parallel_cancel_leaves_no_guide() {
    let mut w = World::new();
    let mut tool = parallel_tool(&mut w);
    w.run(&mut tool, Input::Move(v(2.0, 1.0, 0.0))).unwrap();
    w.run(&mut tool, Input::Cancel).unwrap();
    assert_eq!(tool.stage(), TapeStage::Start);
    assert!(w.guides().is_empty());
    assert_eq!(w.undo.undo_count(), 0);
}

#[test]
fn point_guide_modifier_leaves_a_guide_point_per_click() {
    let mut w = World::new();
    let mut tool = TapeTool::new();
    tool.set_point_guides(true);
    w.run(&mut tool, Input::Click(v(1.0, 2.0, 3.0))).unwrap();
    assert_eq!(tool.stage(), TapeStage::Start);
    w.run(&mut tool, Input::Click(v(-1.0, 0.0, 0.5))).unwrap();
    assert_eq!(tool.stage(), TapeStage::Start);
    assert_eq!(
        w.guides(),
        vec![
            point_guide(v(1.0, 2.0, 3.0)).unwrap(),
            point_guide(v(-1.0, 0.0, 0.5)).unwrap()
        ]
    );
    assert_eq!(w.undo.undo_count(), 2);
    assert_eq!(w.undo.undo_label(), Some("Guide point"));
}

#[test]
fn point_guide_modifier_beats_the_hover_edge() {
    let mut w = World::new();
    let mut tool = TapeTool::new();
    tool.set_hover_edge(Some((A, B)));
    tool.set_point_guides(true);
    w.run(&mut tool, Input::Click(v(1.0, 0.0, 0.0))).unwrap();
    assert_eq!(tool.stage(), TapeStage::Start);
    assert_eq!(w.guides().len(), 1);
}

#[test]
fn point_guide_modifier_off_again_measures() {
    let mut w = World::new();
    let mut tool = TapeTool::new();
    tool.set_point_guides(true);
    tool.set_point_guides(false);
    w.run(&mut tool, Input::Click(A)).unwrap();
    assert_eq!(tool.stage(), TapeStage::End);
    assert!(w.guides().is_empty());
}

#[test]
fn point_guide_click_keeps_the_last_distance() {
    let mut w = World::new();
    let mut tool = TapeTool::new();
    w.run(&mut tool, Input::Click(A)).unwrap();
    w.run(&mut tool, Input::Click(B)).unwrap();
    tool.set_point_guides(true);
    w.run(&mut tool, Input::Click(A)).unwrap();
    assert_eq!(tool.last_distance(), Some(4.0));
}

#[test]
fn modifier_is_read_only_at_start() {
    let mut w = World::new();
    let mut tool = TapeTool::new();
    w.run(&mut tool, Input::Click(A)).unwrap();
    tool.set_point_guides(true);
    w.run(&mut tool, Input::Click(B)).unwrap();
    assert_eq!(tool.stage(), TapeStage::Start);
    assert_eq!(tool.last_distance(), Some(4.0));
    assert!(w.guides().is_empty());
}

#[test]
fn guides_are_not_selectable_geometry() {
    let mut w = World::new();
    let mut tool = parallel_tool(&mut w);
    w.run(&mut tool, Input::Click(v(2.0, 2.0, 0.0))).unwrap();
    assert_eq!(w.model.root().geometry().edges().count(), 0);
}
