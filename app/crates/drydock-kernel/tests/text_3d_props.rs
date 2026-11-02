// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Properties of 3D text on random boxes (`docs/specs/m7-text-3d.md`).

use drydock_kernel::model::*;
use drydock_kernel::solid::check_body;
use drydock_kernel::text3d::{Text3dError, TextMode, text_3d};
use drydock_kernel::topology::FaceId;
use glam::{DVec2, DVec3};
use proptest::prelude::*;

mod offset_common;
use offset_common::*;

/// One letter in a grid cell: the outer square, and optionally a counter, and optionally an
/// island in the counter.
#[derive(Clone, Copy, Debug)]
enum Letter {
    Plain,
    Counter,
    Island,
}

/// A box face as `box_faces` lists them: center, outward normal, two in-plane axes, the
/// half sizes along them, and the box's size along the normal.
struct Frame {
    center: DVec3,
    normal: DVec3,
    u: DVec3,
    w: DVec3,
    half: DVec2,
    thickness: f64,
}

fn frame(size: DVec3, face: usize) -> Frame {
    let axes = [DVec3::X, DVec3::Y, DVec3::Z];
    // bottom, top, front, back, right, left
    let (axis, sign) = [
        (2, -1.0),
        (2, 1.0),
        (1, -1.0),
        (1, 1.0),
        (0, 1.0),
        (0, -1.0),
    ][face];
    let (a, b) = [(1, 2), (0, 2), (0, 1)][axis];
    let mid = size * 0.5;
    let mut center = mid;
    center[axis] = if sign > 0.0 { size[axis] } else { 0.0 };
    Frame {
        center,
        normal: axes[axis] * sign,
        u: axes[a],
        w: axes[b],
        half: DVec2::new(mid[a], mid[b]),
        thickness: size[axis],
    }
}

fn square(f: &Frame, lo: DVec2, hi: DVec2) -> Vec<DVec3> {
    [
        DVec2::new(lo.x, lo.y),
        DVec2::new(hi.x, lo.y),
        DVec2::new(hi.x, hi.y),
        DVec2::new(lo.x, hi.y),
    ]
    .iter()
    .map(|p| f.center + f.u * p.x + f.w * p.y)
    .collect()
}

/// Loops for a grid of letters over the middle 80% of the face, and their net area.
fn letters(f: &Frame, cols: usize, rows: usize, kinds: &[Letter]) -> (Vec<Vec<DVec3>>, f64) {
    let span = f.half * 1.6;
    let cell = DVec2::new(span.x / cols as f64, span.y / rows as f64);
    let origin = -f.half * 0.8;
    let mut loops = Vec::new();
    let mut area = 0.0;
    for r in 0..rows {
        for c in 0..cols {
            let lo = origin + DVec2::new(c as f64 * cell.x, r as f64 * cell.y);
            let inset = |k: f64| (lo + cell * k, lo + cell * (1.0 - k));
            let part = |k: f64| {
                let s = cell * (1.0 - 2.0 * k);
                s.x * s.y
            };
            let (a, b) = inset(0.1);
            loops.push(square(f, a, b));
            area += part(0.1);
            let kind = kinds[(r * cols + c) % kinds.len()];
            if matches!(kind, Letter::Counter | Letter::Island) {
                let (a, b) = inset(0.3);
                loops.push(square(f, a, b).into_iter().rev().collect());
                area -= part(0.3);
            }
            if matches!(kind, Letter::Island) {
                let (a, b) = inset(0.42);
                loops.push(square(f, a, b));
                area += part(0.42);
            }
        }
    }
    (loops, area)
}

fn letter() -> impl Strategy<Value = Letter> {
    prop_oneof![
        Just(Letter::Plain),
        Just(Letter::Counter),
        Just(Letter::Island)
    ]
}

struct Case {
    model: Model,
    body: BodyId,
    face: FaceId,
    frame: Frame,
    loops: Vec<Vec<DVec3>>,
    area: f64,
}

fn case(size_mm: (f64, f64, f64), face: usize, cols: usize, rows: usize, kinds: &[Letter]) -> Case {
    let size = DVec3::new(mm(size_mm.0), mm(size_mm.1), mm(size_mm.2));
    let mut model = Model::new();
    let body = add_group(&mut model);
    let ids = build(
        &mut model,
        body,
        &box_points(DVec3::ZERO, size),
        &plain(&box_faces(0)),
    );
    let frame = frame(size, face);
    let (loops, area) = letters(&frame, cols, rows, kinds);
    Case {
        model,
        body,
        face: ids[face],
        frame,
        loops,
        area,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Emboss adds the letters' area times the height, engrave removes it; the part stays
    /// a clean solid and every cap lies at the height from the face, away from or into the
    /// material.
    #[test]
    fn volume_changes_by_area_times_height(
        sx in 10.0..40.0f64, sy in 10.0..40.0f64, sz in 4.0..30.0f64,
        face in 0usize..6,
        cols in 1usize..4, rows in 1usize..3,
        kinds in prop::collection::vec(letter(), 1..4),
        engrave in any::<bool>(),
        k in 0.05..0.9f64,
    ) {
        let mut c = case((sx, sy, sz), face, cols, rows, &kinds);
        let before = check_body(&c.model, c.body).unwrap().volume.unwrap();
        let (mode, height, sign) = if engrave {
            (TextMode::Engrave, c.frame.thickness * k, -1.0)
        } else {
            (TextMode::Emboss, mm(10.0) * k, 1.0)
        };
        let mut undo = UndoStack::new();
        let made = text_3d(&mut c.model, c.body, c.face, &c.loops, mode, height, &mut undo, "Text")
            .unwrap();
        prop_assert!(c.model.validate().is_empty());
        let r = check_body(&c.model, c.body).unwrap();
        prop_assert!(r.is_clean(), "{:?}", r);
        let want = before + sign * c.area * height;
        prop_assert!(close(r.volume.unwrap(), want), "{} vs {}", r.volume.unwrap(), want);

        let letters = c.loops.len()
            - kinds.iter().cycle().take(cols * rows)
                .filter(|l| matches!(l, Letter::Counter)).count()
            - kinds.iter().cycle().take(cols * rows)
                .filter(|l| matches!(l, Letter::Island)).count();
        prop_assert_eq!(made.caps.len(), letters);
        let plane_d = c.frame.center.dot(c.frame.normal);
        for &cap in &made.caps {
            for p in face_points(&c.model, c.body, cap) {
                let got = p.dot(c.frame.normal) - plane_d;
                prop_assert!((got - sign * height).abs() <= 1e-9, "{} vs {}", got, sign * height);
            }
        }
    }

    /// Undo puts the model back exactly; redo brings the text back.
    #[test]
    fn undo_and_redo(
        sx in 10.0..40.0f64, sy in 10.0..40.0f64, sz in 4.0..30.0f64,
        face in 0usize..6,
        cols in 1usize..4, rows in 1usize..3,
        kinds in prop::collection::vec(letter(), 1..4),
        engrave in any::<bool>(),
    ) {
        let mut c = case((sx, sy, sz), face, cols, rows, &kinds);
        let before = c.model.clone();
        let mode = if engrave { TextMode::Engrave } else { TextMode::Emboss };
        let mut undo = UndoStack::new();
        text_3d(&mut c.model, c.body, c.face, &c.loops, mode, mm(1.0), &mut undo, "Text").unwrap();
        let after = c.model.clone();
        prop_assert_eq!(undo.undo_count(), 1);
        undo.undo(&mut c.model).unwrap();
        prop_assert!(c.model == before);
        undo.redo(&mut c.model).unwrap();
        prop_assert!(c.model == after);
    }

    /// Text moved off the face is refused before anything changes.
    #[test]
    fn moved_off_the_face_is_refused(
        sx in 10.0..40.0f64, sy in 10.0..40.0f64, sz in 4.0..30.0f64,
        face in 0usize..6,
        cols in 1usize..4, rows in 1usize..3,
        kinds in prop::collection::vec(letter(), 1..4),
        engrave in any::<bool>(),
        along_u in any::<bool>(),
    ) {
        let mut c = case((sx, sy, sz), face, cols, rows, &kinds);
        let shift = if along_u { c.frame.u * c.frame.half.x * 4.0 } else { c.frame.w * c.frame.half.y * 4.0 };
        let moved: Vec<Vec<DVec3>> =
            c.loops.iter().map(|l| l.iter().map(|p| *p + shift).collect()).collect();
        let before = c.model.clone();
        let mode = if engrave { TextMode::Engrave } else { TextMode::Emboss };
        let mut undo = UndoStack::new();
        let got = text_3d(&mut c.model, c.body, c.face, &moved, mode, mm(1.0), &mut undo, "Text");
        prop_assert_eq!(got, Err(Text3dError::CrossesBorder(0)));
        prop_assert!(c.model == before);
        prop_assert_eq!(undo.undo_count(), 0);
    }
}

fn face_points(model: &Model, body: BodyId, f: FaceId) -> Vec<DVec3> {
    let ctx = geometry(model, body);
    ctx.loop_points(&ctx.face(f).unwrap().outer)
}
