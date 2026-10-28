// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Tape Measure in the editor (`docs/specs/m3-app-tape.md`).

use glam::{DVec2, DVec3};
use modeler::editor::*;
use modeler::input::{EditKey, Modifiers, Tool};
use modeler_kernel::model::{BodyId, Model, UndoStack};
use modeler_kernel::select::Entity;
use modeler_kernel::tape::Guide;
use modeler_kernel::tools::tape::TapeStage;
use modeler_kernel::tools::{Target, draw_segments};
use modeler_kernel::units::{self, Unit};
use modeler_view::{Bounds, Camera, Projection, StandardView};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn view(standard: StandardView) -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(standard);
    camera.projection = Projection::Parallel;
    camera.zoom_extents(
        &Bounds {
            min: DVec3::splat(-8.0),
            max: DVec3::splat(8.0),
        },
        800.0 / 600.0,
    );
    Viewport {
        camera,
        size_px: (800.0, 600.0),
    }
}

/// The unit square at x 1..2, y 0..1 on the ground.
const SQUARE: [DVec3; 4] = [
    DVec3::new(1.0, 0.0, 0.0),
    DVec3::new(2.0, 0.0, 0.0),
    DVec3::new(2.0, 1.0, 0.0),
    DVec3::new(1.0, 1.0, 0.0),
];

fn f(value: f64) -> String {
    units::format_length(value, Unit::Meter, 2)
}

fn has_segment(segs: &[(DVec3, DVec3)], a: DVec3, b: DVec3) -> bool {
    segs.iter().any(|&(p, q)| {
        (p.distance(a) < 1e-6 && q.distance(b) < 1e-6)
            || (p.distance(b) < 1e-6 && q.distance(a) < 1e-6)
    })
}

struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
}

impl World {
    /// Top view, the square drawn straight into the model, the Tape tool.
    fn new() -> World {
        let mut w = World {
            model: Model::new(),
            editor: Editor::new(),
            view: view(StandardView::Top),
        };
        set_unit(&mut w.model, Unit::Meter);
        let n = SQUARE.len();
        let segs: Vec<_> = (0..n).map(|i| (SQUARE[i], SQUARE[(i + 1) % n])).collect();
        let mut undo = UndoStack::new();
        let mut t = Target {
            model: &mut w.model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: Unit::Meter,
        };
        draw_segments(&mut t, &segs, "Draw").expect("draw");
        w.editor.set_tool(Tool::Tape);
        w
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

    fn enter(&mut self, s: &str) -> bool {
        assert!(self.editor.text(s), "text {s:?} refused");
        self.key(EditKey::Enter)
    }

    fn stage(&self) -> TapeStage {
        self.editor.tape_tool().expect("tape tool").stage()
    }

    fn undo_count(&self) -> usize {
        self.editor.undo_stack().undo_count()
    }

    fn guides(&self) -> Vec<Guide> {
        self.model.root().guides().iter().map(|(_, g)| *g).collect()
    }

    /// Start on the middle of the bottom edge (y = 0, x 1..2).
    fn start_on_edge(&mut self) {
        assert!(!self.click(v(1.5, 0.0, 0.0)));
        assert_eq!(self.stage(), TapeStage::End);
    }
}

fn assert_line(guide: &Guide, y: f64) {
    match guide {
        Guide::Line { point, dir } => {
            assert!((point.y - y).abs() < 1e-6, "{guide:?}");
            assert!(point.z.abs() < 1e-6, "{guide:?}");
            assert!((dir.x.abs() - 1.0).abs() < 1e-6, "{guide:?}");
        }
        other => panic!("not a line: {other:?}"),
    }
}

#[test]
fn tape_tool_only_with_tape() {
    let mut w = World::new();
    assert!(w.editor.tape_tool().is_some());
    assert_eq!(w.stage(), TapeStage::Start);
    assert_eq!(w.editor.draw_plane(), None);
    for tool in [Tool::Line, Tool::Select, Tool::Move, Tool::Orbit] {
        w.editor.set_tool(tool);
        assert!(w.editor.tape_tool().is_none(), "{tool:?}");
    }
}

#[test]
fn measurement_is_an_empty_length_at_first() {
    let w = World::new();
    assert_eq!(
        w.editor.measurement(&w.model),
        Some(("Length", String::new()))
    );
    assert!(w.editor.preview().is_empty());
}

#[test]
fn plain_measurement_reads_the_distance_and_changes_nothing() {
    let mut w = World::new();
    assert!(!w.click(v(4.0, 3.0, 0.0)));
    assert_eq!(w.stage(), TapeStage::End);
    assert_eq!(w.editor.tape_tool().unwrap().edge(), None);
    assert_eq!(
        w.editor.measurement(&w.model),
        Some(("Length", String::new()))
    );
    w.move_to(v(7.0, 7.0, 0.0));
    assert_eq!(w.editor.measurement(&w.model), Some(("Length", f(5.0))));
    assert!(!w.click(v(7.0, 7.0, 0.0)));
    assert_eq!(w.stage(), TapeStage::Start);
    assert_eq!(w.undo_count(), 0);
    assert!(w.guides().is_empty());
    assert_eq!(w.editor.message(), None);
    assert_eq!(w.editor.measurement(&w.model), Some(("Length", f(5.0))));
}

#[test]
fn plain_measurement_previews_a_line_to_the_cursor() {
    let mut w = World::new();
    w.click(v(4.0, 3.0, 0.0));
    w.move_to(v(7.0, 7.0, 0.0));
    let preview = w.editor.preview();
    assert!(
        has_segment(&preview, v(4.0, 3.0, 0.0), v(7.0, 7.0, 0.0)),
        "{preview:?}"
    );
    w.click(v(7.0, 7.0, 0.0));
    assert!(w.editor.preview().is_empty());
}

#[test]
fn starting_on_an_edge_picks_it_up() {
    let mut w = World::new();
    w.start_on_edge();
    let (a, b) = w.editor.tape_tool().unwrap().edge().expect("edge");
    let (lo, hi) = if a.x < b.x { (a, b) } else { (b, a) };
    assert!(lo.distance(v(1.0, 0.0, 0.0)) < 1e-6, "{a:?} {b:?}");
    assert!(hi.distance(v(2.0, 0.0, 0.0)) < 1e-6, "{a:?} {b:?}");
}

#[test]
fn starting_on_a_face_is_a_plain_measurement() {
    let mut w = World::new();
    assert!(!w.click(v(1.5, 0.5, 0.0)));
    assert_eq!(w.stage(), TapeStage::End);
    assert_eq!(w.editor.tape_tool().unwrap().edge(), None);
}

#[test]
fn clicking_beside_an_edge_leaves_a_parallel_guide() {
    let mut w = World::new();
    w.start_on_edge();
    w.move_to(v(1.5, -2.0, 0.0));
    assert_eq!(w.editor.measurement(&w.model), Some(("Length", f(2.0))));
    assert!(w.click(v(1.5, -2.0, 0.0)));
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.stage(), TapeStage::Start);
    assert_eq!(w.editor.message(), None);
    let guides = w.guides();
    assert_eq!(guides.len(), 1);
    assert_line(&guides[0], -2.0);
    assert_eq!(w.editor.measurement(&w.model), Some(("Length", f(2.0))));
    assert!(w.key(EditKey::Undo));
    assert!(w.guides().is_empty());
    assert!(w.key(EditKey::Redo));
    assert_eq!(w.guides().len(), 1);
}

#[test]
fn parallel_measurement_previews_the_guide() {
    let mut w = World::new();
    w.start_on_edge();
    w.move_to(v(1.5, -2.0, 0.0));
    let preview = w.editor.preview();
    assert!(
        has_segment(&preview, v(1.0, -2.0, 0.0), v(2.0, -2.0, 0.0)),
        "{preview:?}"
    );
}

#[test]
fn typed_offset_places_the_guide() {
    let mut w = World::new();
    w.start_on_edge();
    w.move_to(v(1.5, 1.0, 0.0));
    assert!(w.enter("0.5"));
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.stage(), TapeStage::Start);
    assert_eq!(w.editor.typed(), "");
    let guides = w.guides();
    assert_eq!(guides.len(), 1);
    assert_line(&guides[0], 0.5);
    assert_eq!(w.editor.measurement(&w.model), Some(("Length", f(0.5))));
}

#[test]
fn typed_offset_without_a_start_is_refused() {
    let mut w = World::new();
    assert!(!w.enter("3"));
    assert_eq!(w.editor.message(), Some("Click a start point first"));
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn typed_offset_needs_an_edge() {
    let mut w = World::new();
    w.click(v(4.0, 3.0, 0.0));
    w.move_to(v(7.0, 7.0, 0.0));
    assert!(!w.enter("3"));
    assert_eq!(
        w.editor.message(),
        Some("Start on an edge to place a guide")
    );
    assert_eq!(w.stage(), TapeStage::End);
    assert!(w.guides().is_empty());
}

#[test]
fn typed_offset_refusals() {
    let mut w = World::new();
    w.start_on_edge();
    w.move_to(v(1.5, -2.0, 0.0));
    assert!(!w.enter("-1"));
    assert_eq!(w.editor.message(), Some("Length must be more than zero"));
    assert_eq!(w.stage(), TapeStage::End);
    assert!(!w.enter("0"));
    assert_eq!(w.editor.message(), Some("Length must be more than zero"));
    assert!(!w.enter("1x"));
    assert_eq!(w.editor.message(), Some("Not a length"));
    assert!(w.guides().is_empty());
    assert!(w.enter("1"));
    assert_eq!(w.editor.message(), None);
    assert_line(&w.guides()[0], -1.0);
}

#[test]
fn typed_offset_before_the_cursor_moves_has_no_side() {
    let mut w = World::new();
    w.start_on_edge();
    assert!(!w.enter("1"));
    assert_eq!(w.editor.message(), Some("Move the cursor to show the side"));
    assert_eq!(w.stage(), TapeStage::End);
}

#[test]
fn clicking_on_the_edges_line_has_no_side() {
    let mut w = World::new();
    w.start_on_edge();
    assert!(!w.click(v(3.0, 0.0, 0.0)));
    assert_eq!(w.editor.message(), Some("Move the cursor to show the side"));
    assert_eq!(w.stage(), TapeStage::End);
    assert_eq!(w.undo_count(), 0);
    assert!(w.click(v(3.0, -1.0, 0.0)));
    assert_eq!(w.editor.message(), None);
}

#[test]
fn point_guide_modifier_leaves_guide_points() {
    let mut w = World::new();
    w.editor.set_point_guides(true);
    assert!(w.click(v(5.0, 5.0, 0.0)));
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.stage(), TapeStage::Start);
    match w.guides()[..] {
        [Guide::Point(p)] => assert!(p.distance(v(5.0, 5.0, 0.0)) < 1e-6, "{p:?}"),
        ref other => panic!("{other:?}"),
    }
    // The flag survives a tool switch.
    w.editor.set_tool(Tool::Select);
    w.editor.set_tool(Tool::Tape);
    assert!(w.click(v(-5.0, 5.0, 0.0)));
    assert_eq!(w.guides().len(), 2);
    // And it can be turned off again.
    w.editor.set_point_guides(false);
    assert!(!w.click(v(-5.0, -5.0, 0.0)));
    assert_eq!(w.stage(), TapeStage::End);
}

#[test]
fn escape_drops_the_measurement() {
    let mut w = World::new();
    w.start_on_edge();
    assert!(w.editor.text("1"));
    assert!(!w.key(EditKey::Cancel));
    assert_eq!(w.stage(), TapeStage::Start);
    assert_eq!(w.editor.typed(), "");
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn undo_drops_the_measurement() {
    let mut w = World::new();
    w.start_on_edge();
    w.move_to(v(1.5, -2.0, 0.0));
    w.key(EditKey::Undo);
    assert_eq!(w.stage(), TapeStage::Start);
    assert!(w.guides().is_empty());
}

#[test]
fn switching_tools_forgets_the_last_distance() {
    let mut w = World::new();
    w.click(v(4.0, 3.0, 0.0));
    w.click(v(7.0, 7.0, 0.0));
    assert_eq!(w.editor.measurement(&w.model), Some(("Length", f(5.0))));
    w.editor.set_tool(Tool::Select);
    w.editor.set_tool(Tool::Tape);
    assert_eq!(
        w.editor.measurement(&w.model),
        Some(("Length", String::new()))
    );
}

#[test]
fn a_guide_does_not_touch_the_geometry() {
    let mut w = World::new();
    w.start_on_edge();
    w.click(v(1.5, -2.0, 0.0));
    let ctx = w.model.root().geometry();
    assert_eq!(ctx.edge_count(), 4);
    assert_eq!(ctx.faces().count(), 1);
    assert!(
        w.editor
            .selection()
            .iter()
            .all(|e| matches!(e, Entity::Edge(_)))
    );
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
