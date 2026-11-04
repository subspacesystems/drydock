// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Follow Me tool in the editor (`docs/specs/m9-followme-tool.md`): picking the profile
//! and the path with clicks, the preview, Enter to apply, Esc to cancel.

use drydock::editor::*;
use drydock::input::{EditKey, Modifiers, Tool};
use drydock_kernel::model::{BodyId, Model, UndoStack};
use drydock_kernel::sweep::{face_perimeter, path_from_edges};
use drydock_kernel::sweepface::{sweep_face, sweep_sections};
use drydock_kernel::tools::{Target, draw_segments};
use drydock_kernel::topology::{Context, EdgeId, FaceId};
use drydock_kernel::units::Unit;
use drydock_view::{Bounds, Camera, Projection, StandardView};
use glam::{DVec2, DVec3};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

const HINT_PROFILE: &str = "Click the face to sweep (the profile).";
const HINT_PATH: &str = "Click edges, or a face to follow its outline, for the path.";
const HINT_APPLY: &str = "Press Enter to sweep along the path. Esc cancels.";

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// Parallel iso camera framing -5..5 on every axis, 800x600 px.
fn iso_view() -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(StandardView::Iso);
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
    fn blank() -> World {
        let mut editor = Editor::new();
        let mut model = Model::new();
        set_unit(&mut model, Unit::Meter);
        editor.set_tool(Tool::FollowMe);
        World {
            model,
            editor,
            view: iso_view(),
        }
    }

    fn ctx(&self) -> &Context {
        self.model.root().geometry()
    }

    /// Draws closed or open polylines straight into the model (no undo step in the editor).
    fn draw(&mut self, points: &[DVec3], closed: bool) {
        let n = points.len();
        let count = if closed { n } else { n - 1 };
        let segs: Vec<_> = (0..count)
            .map(|i| (points[i], points[(i + 1) % n]))
            .collect();
        let mut undo = UndoStack::new();
        let mut t = Target {
            model: &mut self.model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: Unit::Meter,
        };
        draw_segments(&mut t, &segs, "Scene").expect("draw");
    }

    /// A 0.2 m square profile in the plane x = 0 (y 0.3..0.5, z 0..0.2) and an open path
    /// (0,0,0) - (2,0,0) - (2,2,0) on the ground, drawn as loose edges.
    fn open() -> World {
        let mut w = World::blank();
        w.draw(
            &[
                v(0.0, 0.3, 0.0),
                v(0.0, 0.5, 0.0),
                v(0.0, 0.5, 0.2),
                v(0.0, 0.3, 0.2),
            ],
            true,
        );
        w.draw(
            &[v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0), v(2.0, 2.0, 0.0)],
            false,
        );
        assert_eq!(w.ctx().face_count(), 1);
        w
    }

    /// The same profile part way along the side of a 4 m ring on the ground (the ring is a
    /// face too): profile x = 2, y -0.2..-0.1, z 0..0.1.
    fn ring() -> World {
        let mut w = World::blank();
        w.draw(
            &[
                v(2.0, -0.2, 0.0),
                v(2.0, -0.1, 0.0),
                v(2.0, -0.1, 0.1),
                v(2.0, -0.2, 0.1),
            ],
            true,
        );
        w.draw(
            &[
                v(0.0, 0.0, 0.0),
                v(4.0, 0.0, 0.0),
                v(4.0, 4.0, 0.0),
                v(0.0, 4.0, 0.0),
            ],
            true,
        );
        assert_eq!(w.ctx().face_count(), 2);
        w
    }

    fn profile(&self) -> FaceId {
        self.face_at(self.profile_point())
    }

    fn profile_point(&self) -> DVec3 {
        // The profile is the face with the smaller area, whichever scene this is.
        let ctx = self.ctx();
        let (_, face) = ctx
            .faces()
            .min_by(|a, b| {
                let area = |f: &drydock_kernel::topology::Face| {
                    let p = ctx.loop_points(&f.outer);
                    let c = p.iter().sum::<DVec3>() / p.len() as f64;
                    p.iter().map(|q| q.distance(c)).sum::<f64>()
                };
                area(a.1).total_cmp(&area(b.1))
            })
            .expect("a face");
        let p = ctx.loop_points(&face.outer);
        p.iter().sum::<DVec3>() / p.len() as f64
    }

    fn face_at(&self, centre: DVec3) -> FaceId {
        let ctx = self.ctx();
        ctx.faces()
            .find(|(_, f)| {
                let p = ctx.loop_points(&f.outer);
                (p.iter().sum::<DVec3>() / p.len() as f64).distance(centre) < 1e-6
            })
            .map(|(id, _)| id)
            .expect("face")
    }

    fn edge(&self, a: DVec3, b: DVec3) -> EdgeId {
        let ctx = self.ctx();
        let va = ctx.find_vertex(a, 1e-6).expect("vertex a");
        let vb = ctx.find_vertex(b, 1e-6).expect("vertex b");
        ctx.edge_between(va, vb).expect("edge")
    }

    fn px(&self, p: DVec3) -> DVec2 {
        self.view.project(p).expect("visible")
    }

    fn click(&mut self, p: DVec3) -> bool {
        let c = self.px(p);
        self.editor
            .pointer_move(&mut self.model, &self.view, c, NONE);
        self.editor.click(&mut self.model, &self.view, c, NONE, 1)
    }

    fn key(&mut self, key: EditKey) -> bool {
        self.editor.key(key, &mut self.model)
    }

    fn counts(&self) -> (usize, usize, usize) {
        let c = self.ctx();
        (c.face_count(), c.edge_count(), c.vertex_count())
    }

    fn undo_count(&self) -> usize {
        self.editor.undo_stack().undo_count()
    }

    fn assert_valid(&self) {
        assert!(
            self.ctx().validate().is_empty(),
            "{:?}",
            self.ctx().validate()
        );
    }
}

fn has_segment(segs: &[(DVec3, DVec3)], a: DVec3, b: DVec3) -> bool {
    segs.iter().any(|&(p, q)| {
        (p.distance(a) < 1e-6 && q.distance(b) < 1e-6)
            || (p.distance(b) < 1e-6 && q.distance(a) < 1e-6)
    })
}

/// What the kernel makes of the same sweep on an identical scene: the numbers to match.
fn reference(mut w: World, edges: &[EdgeId]) -> (usize, usize, usize) {
    let profile = w.profile();
    let path = path_from_edges(w.ctx(), edges).expect("path");
    let mut undo = UndoStack::new();
    sweep_face(
        &mut w.model,
        BodyId::Root,
        profile,
        &path,
        &mut undo,
        "Follow Me",
    )
    .expect("reference sweep");
    w.counts()
}

const OPEN_A: (DVec3, DVec3) = (DVec3::new(0.0, 0.0, 0.0), DVec3::new(2.0, 0.0, 0.0));
const OPEN_B: (DVec3, DVec3) = (DVec3::new(2.0, 0.0, 0.0), DVec3::new(2.0, 2.0, 0.0));
/// Where to click on each open-path edge, away from the profile and from the vertices.
const OPEN_A_AT: DVec3 = DVec3::new(1.2, 0.0, 0.0);
const OPEN_B_AT: DVec3 = DVec3::new(2.0, 1.2, 0.0);

fn open_with_path() -> (World, Vec<EdgeId>) {
    let mut w = World::open();
    let edges = vec![w.edge(OPEN_A.0, OPEN_A.1), w.edge(OPEN_B.0, OPEN_B.1)];
    let at = w.profile_point();
    w.click(at);
    w.click(OPEN_A_AT);
    w.click(OPEN_B_AT);
    (w, edges)
}

const RING: [(DVec3, DVec3); 4] = [
    (DVec3::new(0.0, 0.0, 0.0), DVec3::new(4.0, 0.0, 0.0)),
    (DVec3::new(4.0, 0.0, 0.0), DVec3::new(4.0, 4.0, 0.0)),
    (DVec3::new(4.0, 4.0, 0.0), DVec3::new(0.0, 4.0, 0.0)),
    (DVec3::new(0.0, 4.0, 0.0), DVec3::new(0.0, 0.0, 0.0)),
];
const RING_AT: [DVec3; 4] = [
    DVec3::new(3.0, 0.0, 0.0),
    DVec3::new(4.0, 1.0, 0.0),
    DVec3::new(1.0, 4.0, 0.0),
    DVec3::new(0.0, 3.0, 0.0),
];
/// Inside the ring face, away from the profile.
const RING_FACE_AT: DVec3 = DVec3::new(1.5, 2.5, 0.0);

// ---- hints and picking the profile ----

#[test]
fn the_hint_follows_the_steps() {
    let mut w = World::open();
    assert_eq!(w.editor.follow_hint(), Some(HINT_PROFILE));
    let at = w.profile_point();
    w.click(at);
    assert_eq!(w.editor.follow_hint(), Some(HINT_PATH));
    w.click(OPEN_A_AT);
    assert_eq!(w.editor.follow_hint(), Some(HINT_APPLY));
    w.key(EditKey::Cancel);
    assert_eq!(w.editor.follow_hint(), Some(HINT_PROFILE));
}

#[test]
fn there_is_no_hint_or_state_for_other_tools() {
    let mut w = World::open();
    w.editor.set_tool(Tool::Line);
    assert_eq!(w.editor.follow_hint(), None);
    assert_eq!(w.editor.follow_profile(), None);
    assert!(w.editor.follow_path().is_empty());
}

#[test]
fn clicking_a_face_picks_the_profile_and_changes_nothing() {
    let mut w = World::open();
    let before = w.counts();
    let at = w.profile_point();
    assert!(!w.click(at));
    assert_eq!(w.editor.follow_profile(), Some(w.profile()));
    assert!(w.editor.follow_path().is_empty());
    assert_eq!(w.counts(), before);
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn clicking_empty_space_picks_nothing() {
    let mut w = World::open();
    assert!(!w.click(v(-4.0, 4.0, 0.0)));
    assert_eq!(w.editor.follow_profile(), None);
    assert_eq!(w.editor.follow_hint(), Some(HINT_PROFILE));
}

#[test]
fn an_edge_before_the_profile_is_not_a_path() {
    let mut w = World::open();
    w.click(OPEN_A_AT);
    assert_eq!(w.editor.follow_profile(), None);
    assert!(w.editor.follow_path().is_empty());
}

// ---- picking the path ----

#[test]
fn edge_clicks_collect_the_path_in_click_order_and_a_second_click_removes_one() {
    let mut w = World::open();
    let (a, b) = (w.edge(OPEN_A.0, OPEN_A.1), w.edge(OPEN_B.0, OPEN_B.1));
    let at = w.profile_point();
    w.click(at);
    w.click(OPEN_B_AT);
    w.click(OPEN_A_AT);
    assert_eq!(w.editor.follow_path(), vec![b, a]);
    w.click(OPEN_B_AT);
    assert_eq!(w.editor.follow_path(), vec![a]);
    assert_eq!(w.editor.follow_profile(), Some(w.profile()));
    w.click(OPEN_A_AT);
    assert!(w.editor.follow_path().is_empty());
    assert_eq!(w.editor.follow_hint(), Some(HINT_PATH));
}

#[test]
fn the_profile_and_its_edges_are_not_a_path() {
    let mut w = World::open();
    let at = w.profile_point();
    w.click(at);
    w.click(at);
    assert!(w.editor.follow_path().is_empty());
    assert_eq!(w.editor.follow_profile(), Some(w.profile()));
    assert!(w.editor.message().is_some());
}

#[test]
fn clicking_a_face_picks_its_outline_as_the_path() {
    let mut w = World::ring();
    let at = w.profile_point();
    w.click(at);
    assert!(!w.click(RING_FACE_AT));
    let ring_face = w.face_at(v(2.0, 2.0, 0.0));
    let expected = face_perimeter(w.ctx(), ring_face).expect("outline").edges;
    let mut got = w.editor.follow_path();
    assert_eq!(got.len(), 4);
    got.sort();
    let mut want = expected;
    want.sort();
    assert_eq!(got, want);
    assert_eq!(w.editor.follow_hint(), Some(HINT_APPLY));
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn a_face_pick_replaces_edges_picked_before() {
    let mut w = World::ring();
    let at = w.profile_point();
    w.click(at);
    w.click(RING_AT[0]);
    assert_eq!(w.editor.follow_path().len(), 1);
    w.click(RING_FACE_AT);
    assert_eq!(w.editor.follow_path().len(), 4);
}

// ---- the preview ----

#[test]
fn the_preview_is_empty_until_there_is_a_path() {
    let mut w = World::open();
    assert!(w.editor.preview().is_empty());
    let at = w.profile_point();
    w.click(at);
    assert!(w.editor.preview().is_empty());
}

#[test]
fn the_preview_draws_each_section_and_joins_them_without_touching_the_model() {
    let (w, edges) = open_with_path();
    let before = w.counts();
    let path = path_from_edges(w.ctx(), &edges).expect("path");
    let sections = sweep_sections(w.ctx(), w.profile(), &path).expect("sections");
    assert!(sections.len() >= 3);
    let preview = w.editor.preview();
    for section in &sections {
        for lp in section {
            for i in 0..lp.len() {
                assert!(
                    has_segment(&preview, lp[i], lp[(i + 1) % lp.len()]),
                    "missing section edge"
                );
            }
        }
    }
    for pair in sections.windows(2) {
        for (l0, l1) in pair[0].iter().zip(&pair[1]) {
            for (p, q) in l0.iter().zip(l1) {
                assert!(has_segment(&preview, *p, *q), "missing joining line");
            }
        }
    }
    assert_eq!(w.counts(), before);
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn the_preview_follows_the_path_as_edges_change() {
    let (mut w, _) = open_with_path();
    let long = w.editor.preview().len();
    w.click(OPEN_B_AT);
    let short = w.editor.preview().len();
    assert!(short > 0 && short < long, "{short} !< {long}");
}

#[test]
fn a_preview_of_a_bad_path_is_empty() {
    // The second edge is not connected to the first one.
    let mut w = World::open();
    w.draw(&[v(-3.0, -3.0, 0.0), v(-3.0, -1.0, 0.0)], false);
    let at = w.profile_point();
    w.click(at);
    w.click(OPEN_A_AT);
    w.click(v(-3.0, -2.0, 0.0));
    assert_eq!(w.editor.follow_path().len(), 2);
    assert!(w.editor.preview().is_empty());
}

// ---- applying ----

#[test]
fn enter_sweeps_the_profile_along_an_open_path_as_one_undo_step() {
    let (mut w, edges) = open_with_path();
    let before = w.counts();
    let want = reference(
        {
            let mut r = World::open();
            r.editor.set_tool(Tool::FollowMe);
            r
        },
        &edges,
    );
    assert!(w.key(EditKey::Enter));
    assert_eq!(w.counts(), want);
    assert_ne!(w.counts(), before);
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Follow Me"));
    w.assert_valid();
    // The tool is ready for the next sweep.
    assert_eq!(w.editor.tool(), Tool::FollowMe);
    assert_eq!(w.editor.follow_profile(), None);
    assert!(w.editor.follow_path().is_empty());
    assert!(w.editor.preview().is_empty());
    assert_eq!(w.editor.follow_hint(), Some(HINT_PROFILE));
    // One undo puts everything back.
    assert!(w.key(EditKey::Undo));
    assert_eq!(w.counts(), before);
    w.assert_valid();
    assert!(w.key(EditKey::Redo));
    assert_eq!(w.counts(), want);
}

#[test]
fn four_edges_sweep_a_closed_path() {
    let mut w = World::ring();
    let edges: Vec<EdgeId> = RING.iter().map(|(a, b)| w.edge(*a, *b)).collect();
    let want = reference(World::ring(), &edges);
    let at = w.profile_point();
    w.click(at);
    for p in RING_AT {
        w.click(p);
    }
    assert_eq!(w.editor.follow_path().len(), 4);
    assert!(!w.editor.preview().is_empty());
    assert!(w.key(EditKey::Enter));
    assert_eq!(w.counts(), want);
    assert_eq!(w.undo_count(), 1);
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Follow Me"));
    w.assert_valid();
}

#[test]
fn a_second_click_on_the_path_face_sweeps_a_closed_path() {
    let mut w = World::ring();
    let edges: Vec<EdgeId> = RING.iter().map(|(a, b)| w.edge(*a, *b)).collect();
    let want = reference(World::ring(), &edges);
    let at = w.profile_point();
    w.click(at);
    w.click(RING_FACE_AT);
    let before = w.counts();
    assert!(!w.editor.preview().is_empty());
    assert_eq!(w.counts(), before);
    assert_eq!(w.undo_count(), 0);
    assert!(w.click(RING_FACE_AT), "the second click applies");
    assert_eq!(w.counts(), want);
    assert_eq!(w.undo_count(), 1);
    w.assert_valid();
    assert_eq!(w.editor.follow_profile(), None);
}

#[test]
fn enter_without_a_path_does_nothing() {
    let mut w = World::open();
    let before = w.counts();
    assert!(!w.key(EditKey::Enter));
    let at = w.profile_point();
    w.click(at);
    assert!(!w.key(EditKey::Enter));
    assert_eq!(w.counts(), before);
    assert_eq!(w.undo_count(), 0);
    assert_eq!(w.editor.follow_profile(), Some(w.profile()));
}

#[test]
fn a_path_with_a_gap_is_refused_with_a_message_and_kept_for_fixing() {
    let mut w = World::open();
    w.draw(&[v(-3.0, -3.0, 0.0), v(-3.0, -1.0, 0.0)], false);
    let at = w.profile_point();
    w.click(at);
    w.click(OPEN_A_AT);
    w.click(OPEN_B_AT);
    w.click(v(-3.0, -2.0, 0.0));
    let before = w.counts();
    assert!(!w.key(EditKey::Enter));
    assert!(w.editor.message().is_some());
    assert_eq!(w.counts(), before);
    assert_eq!(w.undo_count(), 0);
    assert_eq!(w.editor.follow_path().len(), 3);
    // Taking the stray edge out again makes it work.
    w.click(v(-3.0, -2.0, 0.0));
    assert_eq!(w.editor.follow_path().len(), 2);
    let ok = w.key(EditKey::Enter);
    assert!(ok, "{:?}", w.editor.message());
    assert_eq!(w.undo_count(), 1);
    w.assert_valid();
}

// ---- cancelling ----

#[test]
fn esc_drops_the_path_and_the_profile() {
    let (mut w, _) = open_with_path();
    let before = w.counts();
    assert!(!w.key(EditKey::Cancel));
    assert_eq!(w.editor.follow_profile(), None);
    assert!(w.editor.follow_path().is_empty());
    assert!(w.editor.preview().is_empty());
    assert_eq!(w.counts(), before);
    assert_eq!(w.undo_count(), 0);
    assert_eq!(w.editor.tool(), Tool::FollowMe);
    // And Enter now has nothing to apply.
    assert!(!w.key(EditKey::Enter));
    assert_eq!(w.undo_count(), 0);
}

#[test]
fn esc_with_only_a_profile_and_with_nothing_is_harmless() {
    let mut w = World::open();
    assert!(!w.key(EditKey::Cancel));
    let at = w.profile_point();
    w.click(at);
    assert!(!w.key(EditKey::Cancel));
    assert_eq!(w.editor.follow_profile(), None);
}

#[test]
fn changing_tool_drops_the_pick() {
    let (mut w, _) = open_with_path();
    w.editor.set_tool(Tool::Select);
    w.editor.set_tool(Tool::FollowMe);
    assert_eq!(w.editor.follow_profile(), None);
    assert!(w.editor.follow_path().is_empty());
    assert!(w.editor.preview().is_empty());
}

fn set_unit(model: &mut Model, unit: Unit) {
    model
        .set_length_format(
            drydock_kernel::units::LengthFormat::Decimal { unit, decimals: 2 },
            &mut drydock_kernel::model::ModelEdit::new(),
        )
        .unwrap();
}
