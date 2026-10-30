// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-text-3d-ui: the plain functions of Tools > 3D Text and `Editor`'s text placement
//! (`docs/specs/m7-text-3d-ui.md`).

use glam::{DMat4, DVec2, DVec3};
use modeler::editor::{Editor, Viewport};
use modeler::input::{EditKey, Modifiers, Tool};
use modeler::look::ROBOTO;
use modeler::text_tool::*;
use modeler_dae::{TextLoop, TextOutline, text_outline};
use modeler_kernel::model::*;
use modeler_kernel::solid::check_geometry;
use modeler_kernel::text3d::{Text3dError, TextMode};
use modeler_kernel::topology::*;
use modeler_view::{Bounds, Camera, Projection, StandardView};
use std::collections::BTreeMap;

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn near3(a: DVec3, b: DVec3) -> bool {
    (a - b).length() < 1e-9
}

const FACES: [[usize; 4]; 6] = [
    [0, 3, 2, 1],
    [4, 5, 6, 7],
    [0, 1, 5, 4],
    [2, 3, 7, 6],
    [1, 2, 6, 5],
    [3, 0, 4, 7],
];

/// A box of side `s` with a corner at the origin in `body` (faces: 0 bottom, 1 top, 2 at
/// y=0, 3 at y=s, 4 at x=s, 5 at x=0). `inside_out` winds every face the other way.
fn make_box(model: &mut Model, body: BodyId, s: f64, inside_out: bool) {
    let points = [
        v(0.0, 0.0, 0.0),
        v(s, 0.0, 0.0),
        v(s, s, 0.0),
        v(0.0, s, 0.0),
        v(0.0, 0.0, s),
        v(s, 0.0, s),
        v(s, s, s),
        v(0.0, s, s),
    ];
    model
        .edit_geometry(body, &mut ModelEdit::new(), |ctx, edit| {
            let vs: Vec<VertexId> = points.iter().map(|&p| ctx.insert_vertex(p, edit)).collect();
            let mut edges = BTreeMap::new();
            for quad in FACES {
                let mut quad = quad;
                if inside_out {
                    quad.reverse();
                }
                let mut uses = Vec::new();
                for i in 0..4 {
                    let (a, b) = (quad[i], quad[(i + 1) % 4]);
                    let key = (a.min(b), a.max(b));
                    let e = *edges
                        .entry(key)
                        .or_insert_with(|| ctx.insert_edge(vs[key.0], vs[key.1], edit).unwrap());
                    uses.push(if a < b {
                        EdgeUse::forward(e)
                    } else {
                        EdgeUse::backward(e)
                    });
                }
                ctx.insert_face(
                    Face {
                        outer: Loop { uses },
                        inner: vec![],
                    },
                    edit,
                )
                .unwrap();
            }
        })
        .unwrap();
}

/// The face of `ctx` whose front normal is `n`.
fn face_facing(ctx: &Context, n: DVec3) -> FaceId {
    ctx.faces()
        .map(|(id, _)| id)
        .find(|&id| near3(ctx.face_plane(id).unwrap().normal, n))
        .expect("face")
}

fn iso_view() -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(StandardView::Iso);
    camera.projection = Projection::Parallel;
    camera.zoom_extents(
        &Bounds {
            min: DVec3::splat(-0.1),
            max: DVec3::splat(0.3),
        },
        800.0 / 600.0,
    );
    Viewport {
        camera,
        size_px: (800.0, 600.0),
    }
}

fn area_of_i(size: f64) -> f64 {
    let outline = text_outline(ROBOTO, "I", size, size / 200.0).expect("outline");
    assert_eq!(outline.loops.len(), 1);
    let p = &outline.loops[0].points;
    (0..p.len())
        .map(|i| p[i].perp_dot(p[(i + 1) % p.len()]))
        .sum::<f64>()
        .abs()
        / 2.0
}

fn options(text: &str, size_m: f64, height_m: f64, mode: TextMode) -> TextOptions {
    TextOptions {
        text: text.into(),
        size_m,
        height_m,
        mode,
    }
}

// ---- parse_mm ----------------------------------------------------------------------

#[test]
fn parse_mm_takes_numbers_with_an_optional_mm() {
    assert_eq!(parse_mm("10"), Some(10.0));
    assert_eq!(parse_mm("  2.5 mm "), Some(2.5));
    assert_eq!(parse_mm("0.01"), Some(MIN_MM));
    assert_eq!(parse_mm("1000"), Some(MAX_MM));
    assert_eq!(parse_mm("7mm"), Some(7.0));
}

#[test]
fn parse_mm_refuses_everything_else() {
    for text in [
        "", "abc", "NaN", "inf", "-1", "0", "0.009", "1000.5", "1,5", "3 cm", "1 2", "mm",
        "1 mm mm",
    ] {
        assert_eq!(parse_mm(text), None, "{text:?}");
    }
}

// ---- dialog state ------------------------------------------------------------------

#[test]
fn a_new_dialog_starts_empty_with_ten_and_one() {
    let d = TextDialog::new();
    assert_eq!(d.text, "");
    assert_eq!(d.size_text, "10");
    assert_eq!(d.height_text, "1");
    assert_eq!(d.mode, TextMode::Emboss);
    assert_eq!(TextDialog::default(), d);
    assert_eq!(d.options(), None, "no text yet");
}

#[test]
fn the_dialog_reads_millimeters_as_meters() {
    let mut d = TextDialog::new();
    d.text = "Hi".into();
    d.size_text = "12 mm".into();
    d.height_text = "0.5".into();
    d.mode = TextMode::Engrave;
    assert_eq!(
        d.options(),
        Some(options("Hi", 0.012, 0.0005, TextMode::Engrave))
    );
}

#[test]
fn the_dialog_has_no_options_while_a_field_is_bad() {
    let mut d = TextDialog::new();
    d.text = "Hi".into();
    assert!(d.options().is_some());
    d.size_text = "big".into();
    assert_eq!(d.options(), None);
    d.size_text = "10".into();
    d.height_text = "0".into();
    assert_eq!(d.options(), None);
    d.height_text = "1".into();
    d.text = String::new();
    assert_eq!(d.options(), None);
}

#[test]
fn from_options_shows_the_last_options_in_millimeters() {
    let d = TextDialog::from_options(&options("Label", 0.01, 0.002, TextMode::Engrave));
    assert_eq!(d.text, "Label");
    assert_eq!(d.size_text, "10");
    assert_eq!(d.height_text, "2");
    assert_eq!(d.mode, TextMode::Engrave);
}

// ---- frame and loops ---------------------------------------------------------------

#[test]
fn text_reads_left_to_right_and_upright_seen_from_outside() {
    let mut model = Model::new();
    make_box(&mut model, BodyId::Root, 0.2, false);
    let ctx = model.root().geometry();
    let cases = [
        (v(0.0, 0.0, 1.0), v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0)),
        (v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0), v(0.0, 0.0, 1.0)),
        (v(0.0, -1.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 0.0, 1.0)),
        (v(0.0, 1.0, 0.0), v(-1.0, 0.0, 0.0), v(0.0, 0.0, 1.0)),
        (v(0.0, 0.0, -1.0), v(-1.0, 0.0, 0.0), v(0.0, 1.0, 0.0)),
    ];
    for (out, right, up) in cases {
        let frame = text_frame(ctx, face_facing(ctx, out)).expect("frame");
        assert!(near3(frame.out, out), "{out:?}: out {:?}", frame.out);
        assert!(
            near3(frame.right, right),
            "{out:?}: right {:?}",
            frame.right
        );
        assert!(near3(frame.up, up), "{out:?}: up {:?}", frame.up);
        assert!(near3(frame.right.cross(frame.up), frame.out));
    }
}

#[test]
fn out_points_away_from_the_material_on_an_inside_out_part() {
    let mut model = Model::new();
    make_box(&mut model, BodyId::Root, 0.2, true);
    let ctx = model.root().geometry();
    // Every face is wound the other way, so the top face's front normal is -Z.
    let top = face_facing(ctx, v(0.0, 0.0, -1.0));
    assert!(
        ctx.loop_points(&ctx.face(top).unwrap().outer)
            .iter()
            .all(|p| near(p.z, 0.2, 1e-9))
    );
    let frame = text_frame(ctx, top).expect("frame");
    assert!(near3(frame.out, v(0.0, 0.0, 1.0)), "{:?}", frame.out);
    assert!(near3(frame.right.cross(frame.up), frame.out));
}

#[test]
fn a_missing_face_has_no_frame() {
    let mut model = Model::new();
    make_box(&mut model, BodyId::Root, 0.2, false);
    assert!(text_frame(model.root().geometry(), FaceId(9999)).is_none());
}

fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> TextLoop {
    TextLoop {
        points: vec![
            DVec2::new(x0, y0),
            DVec2::new(x1, y0),
            DVec2::new(x1, y1),
            DVec2::new(x0, y1),
        ],
        hole: false,
        parent: None,
    }
}

#[test]
fn place_loops_centers_the_bounding_box_on_the_point() {
    let outline = TextOutline {
        loops: vec![square(0.0, 0.0, 2.0, 1.0), square(0.5, 0.25, 1.0, 0.5)],
        width: 2.0,
        line_height: 1.0,
        lines: 1,
    };
    let frame = TextFrame {
        right: v(0.0, 1.0, 0.0),
        up: v(0.0, 0.0, 1.0),
        out: v(1.0, 0.0, 0.0),
    };
    let center = v(0.2, 0.3, 0.4);
    let loops = place_loops(&outline, &frame, center);
    assert_eq!(loops.len(), 2);
    assert_eq!(loops[0].len(), 4);
    assert_eq!(loops[1].len(), 4);
    // The box is (0,0)-(2,1), middle (1, 0.5): outline (0,0) is one `right` and half an
    // `up` behind the point.
    assert!(near3(loops[0][0], v(0.2, -0.7, -0.1)), "{:?}", loops[0][0]);
    assert!(near3(loops[0][2], v(0.2, 1.3, 0.9)), "{:?}", loops[0][2]);
    assert!(near3(loops[1][0], v(0.2, -0.2, 0.15)), "{:?}", loops[1][0]);
}

#[test]
fn place_loops_of_nothing_is_nothing() {
    let outline = TextOutline {
        loops: vec![],
        width: 0.0,
        line_height: 1.0,
        lines: 1,
    };
    let frame = TextFrame {
        right: DVec3::X,
        up: DVec3::Y,
        out: DVec3::Z,
    };
    assert!(place_loops(&outline, &frame, DVec3::ZERO).is_empty());
}

#[test]
fn a_refusal_says_what_the_kernel_said() {
    assert_eq!(
        refusal(&Text3dError::TooSmall),
        "Can't place text: the face is too small for the text"
    );
    assert_eq!(
        refusal(&Text3dError::BadHeight),
        "Can't place text: the height must be a length above zero"
    );
}

// ---- the editor places text ----------------------------------------------------------

const SIDE: f64 = 0.2;

struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
}

impl World {
    fn new() -> World {
        let mut model = Model::new();
        make_box(&mut model, BodyId::Root, SIDE, false);
        World {
            model,
            editor: Editor::new(),
            view: iso_view(),
        }
    }

    fn ctx(&self) -> &Context {
        self.model.root().geometry()
    }

    fn px(&self, p: DVec3) -> DVec2 {
        self.view.project(p).expect("visible")
    }

    fn hover(&mut self, p: DVec3) {
        let c = self.px(p);
        self.editor
            .pointer_move(&mut self.model, &self.view, c, NONE);
    }

    fn click(&mut self, p: DVec3) -> bool {
        self.hover(p);
        let c = self.px(p);
        self.editor.click(&mut self.model, &self.view, c, NONE, 1)
    }

    fn volume(&self) -> f64 {
        check_geometry(self.ctx()).volume.expect("a solid")
    }

    fn top_center() -> DVec3 {
        v(SIDE / 2.0, SIDE / 2.0, SIDE)
    }
}

#[test]
fn beginning_text_waits_for_a_click_and_shows_nothing_before_the_cursor_arrives() {
    let mut w = World::new();
    assert!(w.editor.text_pending().is_none());
    let o = options("I", 0.02, 0.002, TextMode::Emboss);
    assert!(w.editor.begin_text(&w.model, o.clone()));
    assert_eq!(w.editor.text_pending(), Some(&o));
    assert!(w.editor.text_preview(&w.model).is_empty());
    assert_eq!(w.editor.undo_stack().undo_count(), 0);
}

#[test]
fn the_preview_follows_the_cursor_over_a_face() {
    let mut w = World::new();
    let faces = w.ctx().face_count();
    assert!(
        w.editor
            .begin_text(&w.model, options("I", 0.02, 0.002, TextMode::Emboss))
    );
    w.hover(World::top_center());
    let preview = w.editor.text_preview(&w.model);
    assert!(preview.len() >= 4, "{} segments", preview.len());
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    for (a, b) in &preview {
        for p in [a, b] {
            assert!(near(p.z, SIDE, 1e-9), "off the top face: {p:?}");
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
    }
    assert!(near3((lo + hi) / 2.0, World::top_center()), "{lo:?} {hi:?}");
    // The segments chain into closed loops: every endpoint is used twice.
    for (a, _) in &preview {
        let used = preview
            .iter()
            .flat_map(|(p, q)| [p, q])
            .filter(|p| near3(**p, *a))
            .count();
        assert_eq!(used, 2, "{a:?}");
    }
    assert_eq!(w.ctx().face_count(), faces, "hover changes nothing");
    assert_eq!(w.editor.undo_stack().undo_count(), 0);
}

#[test]
fn the_preview_has_the_size_asked_for() {
    let mut w = World::new();
    w.editor
        .begin_text(&w.model, options("I", 0.02, 0.002, TextMode::Emboss));
    w.hover(World::top_center());
    let preview = w.editor.text_preview(&w.model);
    // "I" is a plain upright bar: its height is the font's cap height, a bit over
    // 0.7 em and well under 1 em.
    let ys: Vec<f64> = preview.iter().flat_map(|(a, b)| [a.y, b.y]).collect();
    let height =
        ys.iter().cloned().fold(f64::MIN, f64::max) - ys.iter().cloned().fold(f64::MAX, f64::min);
    assert!(height > 0.6 * 0.02 && height < 0.9 * 0.02, "{height}");
}

#[test]
fn a_click_embosses_the_text_in_one_undo_step() {
    let mut w = World::new();
    let cube = SIDE * SIDE * SIDE;
    assert!(near(w.volume(), cube, 1e-12));
    w.editor
        .begin_text(&w.model, options("I", 0.02, 0.002, TextMode::Emboss));
    assert!(w.click(World::top_center()));
    assert_eq!(w.editor.undo_stack().undo_count(), 1);
    assert_eq!(w.editor.undo_stack().undo_label(), Some("3D Text"));
    assert!(w.editor.text_pending().is_none());
    assert!(w.editor.message().is_none());
    assert!(w.ctx().face_count() > 6);
    let report = check_geometry(w.ctx());
    assert!(report.is_solid() && report.is_clean(), "{report:?}");
    let grown = w.volume() - cube;
    let expect = area_of_i(0.02) * 0.002;
    assert!(near(grown, expect, expect * 0.01), "{grown} vs {expect}");

    w.editor.key(EditKey::Undo, &mut w.model);
    assert_eq!(w.ctx().face_count(), 6);
    assert!(near(w.volume(), cube, 1e-12));
}

#[test]
fn a_click_engraves_when_asked() {
    let mut w = World::new();
    let cube = SIDE * SIDE * SIDE;
    w.editor
        .begin_text(&w.model, options("I", 0.02, 0.002, TextMode::Engrave));
    assert!(w.click(World::top_center()));
    assert_eq!(w.editor.undo_stack().undo_count(), 1);
    let report = check_geometry(w.ctx());
    assert!(report.is_solid(), "{report:?}");
    let cut = cube - w.volume();
    let expect = area_of_i(0.02) * 0.002;
    assert!(near(cut, expect, expect * 0.01), "{cut} vs {expect}");
}

#[test]
fn text_goes_on_the_side_faces_too() {
    let mut w = World::new();
    w.editor
        .begin_text(&w.model, options("I", 0.02, 0.002, TextMode::Emboss));
    assert!(w.click(v(SIDE, SIDE / 2.0, SIDE / 2.0)));
    assert!(check_geometry(w.ctx()).is_solid());
    let max_x = w
        .ctx()
        .vertices()
        .map(|(_, p)| p.position.x)
        .fold(f64::MIN, f64::max);
    assert!(near(max_x, SIDE + 0.002, 1e-9), "{max_x}");
}

#[test]
fn text_that_does_not_fit_is_refused_with_the_reason_and_stays_pending() {
    let mut w = World::new();
    let o = options("I", 0.5, 0.002, TextMode::Emboss);
    w.editor.begin_text(&w.model, o.clone());
    assert!(!w.click(World::top_center()));
    assert_eq!(
        w.editor.message(),
        Some("Can't place text: the face is too small for the text")
    );
    assert_eq!(w.editor.undo_stack().undo_count(), 0);
    assert_eq!(w.ctx().face_count(), 6);
    assert_eq!(w.editor.text_pending(), Some(&o), "try another spot");
}

#[test]
fn an_engraving_through_the_part_is_refused() {
    let mut w = World::new();
    w.editor
        .begin_text(&w.model, options("I", 0.02, 0.5, TextMode::Engrave));
    assert!(!w.click(World::top_center()));
    assert_eq!(
        w.editor.message(),
        Some("Can't place text: the engraving is as deep as the part is thick")
    );
    assert_eq!(w.ctx().face_count(), 6);
    assert_eq!(w.editor.undo_stack().undo_count(), 0);
}

#[test]
fn a_click_on_nothing_says_to_click_a_face() {
    let mut w = World::new();
    let o = options("I", 0.02, 0.002, TextMode::Emboss);
    w.editor.begin_text(&w.model, o.clone());
    let corner = DVec2::new(3.0, 3.0);
    w.editor.pointer_move(&mut w.model, &w.view, corner, NONE);
    assert!(w.editor.text_preview(&w.model).is_empty());
    assert!(!w.editor.click(&mut w.model, &w.view, corner, NONE, 1));
    assert_eq!(
        w.editor.message(),
        Some("Click a face of the part being edited")
    );
    assert_eq!(w.editor.text_pending(), Some(&o));
    assert_eq!(w.editor.undo_stack().undo_count(), 0);
}

#[test]
fn escape_and_other_tools_drop_the_pending_text() {
    let mut w = World::new();
    let o = options("I", 0.02, 0.002, TextMode::Emboss);
    w.editor.begin_text(&w.model, o.clone());
    w.hover(World::top_center());
    assert!(!w.editor.key(EditKey::Cancel, &mut w.model));
    assert!(w.editor.text_pending().is_none());
    assert!(w.editor.text_preview(&w.model).is_empty());

    w.editor.begin_text(&w.model, o);
    w.editor.set_tool(Tool::Line);
    assert!(w.editor.text_pending().is_none());
}

// ---- inside a scaled group -------------------------------------------------------------

fn world_with_group(scale: DVec3) -> (World, InstanceId) {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let def = model.add_definition(
        DefinitionInfo {
            name: "Part".into(),
            group: true,
        },
        &mut log,
    );
    make_box(&mut model, BodyId::Definition(def), SIDE / 2.0, false);
    let inst = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: def,
                transform: DMat4::from_scale(scale),
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    (
        World {
            model,
            editor: Editor::new(),
            view: iso_view(),
        },
        inst,
    )
}

#[test]
fn sizes_are_world_lengths_inside_a_scaled_group() {
    let (mut w, inst) = world_with_group(DVec3::splat(2.0));
    assert!(w.editor.open_instance(&w.model, inst));
    w.editor
        .begin_text(&w.model, options("I", 0.02, 0.002, TextMode::Emboss));
    // The group's top face is at world z = SIDE.
    assert!(w.click(World::top_center()));
    assert_eq!(w.editor.undo_stack().undo_count(), 1);
    let def = w
        .model
        .root()
        .instances()
        .next()
        .expect("instance")
        .1
        .definition;
    let report = modeler_kernel::solid::check_body(&w.model, BodyId::Definition(def)).unwrap();
    assert!(report.is_solid(), "{report:?}");
    // Local text is half the world size: its area is a quarter, its height half.
    let half = SIDE / 2.0;
    let expect = half * half * half + area_of_i(0.01) * 0.001;
    let got = report.volume.unwrap();
    assert!(near(got, expect, expect * 1e-3), "{got} vs {expect}");
}

#[test]
fn a_group_stretched_unevenly_is_refused() {
    let (mut w, inst) = world_with_group(DVec3::new(2.0, 1.0, 2.0));
    assert!(w.editor.open_instance(&w.model, inst));
    w.editor
        .begin_text(&w.model, options("I", 0.02, 0.002, TextMode::Emboss));
    assert!(!w.click(v(SIDE / 2.0, SIDE / 4.0, SIDE)));
    assert_eq!(
        w.editor.message(),
        Some("Can't place text: the part is stretched unevenly")
    );
    assert_eq!(w.editor.undo_stack().undo_count(), 0);
}
