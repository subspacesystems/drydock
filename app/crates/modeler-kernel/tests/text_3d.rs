// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! 3D text: loops embossed on or engraved into a face (`docs/specs/m7-text-3d.md`).

use glam::{DMat4, DVec2, DVec3};
use modeler_kernel::model::*;
use modeler_kernel::pushpull::PushPullError;
use modeler_kernel::solid::{SolidReport, check_body};
use modeler_kernel::sticky::StickyError;
use modeler_kernel::text3d::{Text3dError, TextFaces, TextMode, check_text, text_3d};
use modeler_kernel::topology::*;

mod offset_common;
use offset_common::*;

const LABEL: &str = "3D Text";

/// Cubic millimeters to cubic meters.
fn mm3(x: f64) -> f64 {
    x * 1e-9
}

/// A group holding the faces; returns its body and the face ids in order.
fn part_with_ids(model: &mut Model, points: &[DVec3], faces: &[FaceSpec]) -> (BodyId, Vec<FaceId>) {
    let body = add_group(model);
    let ids = build(model, body, points, faces);
    (body, ids)
}

/// The box `0..20 x 0..20 x 0..5` mm. Faces: bottom, top, front (y = 0), back, right, left.
fn plate() -> (Model, BodyId, Vec<FaceId>) {
    let mut model = Model::new();
    let (body, ids) = part_with_ids(
        &mut model,
        &box_points(v(0.0, 0.0, 0.0), v(mm(20.0), mm(20.0), mm(5.0))),
        &plain(&box_faces(0)),
    );
    (model, body, ids)
}

/// Counter-clockwise rectangle (seen from +z) at height `z`, all in mm.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64, z: f64) -> Vec<DVec3> {
    vec![
        v(mm(x0), mm(y0), mm(z)),
        v(mm(x1), mm(y0), mm(z)),
        v(mm(x1), mm(y1), mm(z)),
        v(mm(x0), mm(y1), mm(z)),
    ]
}

/// The loop with its points in the other order (a hole, as the text outlines give it).
fn rev(l: Vec<DVec3>) -> Vec<DVec3> {
    l.into_iter().rev().collect()
}

/// Runs `text_3d` with a fresh undo stack. On success: the model validates and there is one
/// undo step labelled `LABEL`. On an error: the model is unchanged, nothing is on the stack,
/// and `check_text` gives the same error for a placement refusal, `Ok` for one found while
/// building.
fn run(
    model: &mut Model,
    body: BodyId,
    face: FaceId,
    loops: &[Vec<DVec3>],
    mode: TextMode,
    height: f64,
) -> Result<TextFaces, Text3dError> {
    let before = model.clone();
    let mut undo = UndoStack::new();
    let result = text_3d(model, body, face, loops, mode, height, &mut undo, LABEL);
    match &result {
        Ok(_) => {
            let violations = model.validate();
            assert!(violations.is_empty(), "{violations:?}");
            assert_eq!(undo.undo_count(), 1);
            assert_eq!(undo.undo_label(), Some(LABEL));
        }
        Err(e) => {
            assert!(*model == before, "model changed on {e:?}");
            assert_eq!(undo.undo_count(), 0);
            if let Some(b) = model.body(body) {
                let built = matches!(
                    e,
                    Text3dError::Intersects
                        | Text3dError::Draw(_)
                        | Text3dError::PushPull(_)
                        | Text3dError::NotSolid(_)
                        | Text3dError::Model(_)
                );
                let checked = check_text(b.geometry(), face, loops, mode, height).err();
                let want = if built { None } else { Some(e) };
                assert_eq!(checked.as_ref(), want, "check_text disagrees");
            }
        }
    }
    result
}

fn report(model: &Model, body: BodyId) -> SolidReport {
    check_body(model, body).unwrap()
}

fn face_points(model: &Model, body: BodyId, f: FaceId) -> Vec<DVec3> {
    let ctx = geometry(model, body);
    ctx.loop_points(&ctx.face(f).unwrap().outer)
}

fn all_at_z(points: &[DVec3], z: f64) -> bool {
    points.iter().all(|p| (p.z - z).abs() <= 1e-9)
}

fn centroid(points: &[DVec3]) -> DVec3 {
    points.iter().copied().sum::<DVec3>() / points.len() as f64
}

/// Faces of the body whose outer loop is exactly the corners of `l` (in any order).
fn faces_with_outer(model: &Model, body: BodyId, l: &[DVec3]) -> Vec<FaceId> {
    let ctx = geometry(model, body);
    ctx.faces()
        .filter(|(_, f)| {
            let pts = ctx.loop_points(&f.outer);
            pts.len() == l.len() && l.iter().all(|p| pts.iter().any(|q| near(*p, *q)))
        })
        .map(|(id, _)| id)
        .collect()
}

/// Rotation by +90 degrees about x: profile `(x, y)` extruded along z becomes a part whose
/// profile height `y` is world z.
fn turn_x(points: &[DVec3]) -> Vec<DVec3> {
    let m = DMat4::from_rotation_x(std::f64::consts::FRAC_PI_2);
    points.iter().map(|p| m.transform_point3(*p)).collect()
}

/// The counter-clockwise profile `base` (mm, y up) extruded 20 mm and turned so that y is
/// world z: a part spanning `y = -20..0` mm. Faces: two ends, then one per profile edge.
fn profile_part(base: &[(f64, f64)]) -> (Model, BodyId, Vec<FaceId>) {
    let base: Vec<DVec2> = base
        .iter()
        .map(|&(x, y)| DVec2::new(mm(x), mm(y)))
        .collect();
    let (points, faces) = prism(&base, 0.0, mm(20.0));
    let mut model = Model::new();
    let (body, ids) = part_with_ids(&mut model, &turn_x(&points), &plain(&faces));
    assert!(report(&model, body).is_clean());
    (model, body, ids)
}

// ---- Emboss and engrave ----

#[test]
fn emboss_adds_area_times_height() {
    let (mut model, body, faces) = plate();
    let loops = vec![rect(5.0, 5.0, 15.0, 9.0, 5.0)];
    let made = run(
        &mut model,
        body,
        faces[1],
        &loops,
        TextMode::Emboss,
        mm(1.0),
    )
    .unwrap();
    let r = report(&model, body);
    assert!(r.is_clean(), "{r:?}");
    assert!(
        close(r.volume.unwrap(), mm3(2000.0 + 40.0)),
        "{:?}",
        r.volume
    );
    assert_eq!(made.caps.len(), 1);
    assert_eq!(made.sides.len(), 4);
    assert_eq!(made.repaired, None);
    assert!(all_at_z(&face_points(&model, body, made.caps[0]), mm(6.0)));
    let (_, hi) = bounds(&model, body);
    assert!((hi.z - mm(6.0)).abs() <= 1e-9);
}

#[test]
fn engrave_removes_area_times_depth() {
    let (mut model, body, faces) = plate();
    let loops = vec![rect(5.0, 5.0, 15.0, 9.0, 5.0)];
    let made = run(
        &mut model,
        body,
        faces[1],
        &loops,
        TextMode::Engrave,
        mm(2.0),
    )
    .unwrap();
    let r = report(&model, body);
    assert!(r.is_clean(), "{r:?}");
    assert!(close(r.volume.unwrap(), mm3(2000.0 - 80.0)));
    assert_eq!(made.caps.len(), 1);
    assert_eq!(made.sides.len(), 4);
    assert!(all_at_z(&face_points(&model, body, made.caps[0]), mm(3.0)));
    let (lo, hi) = bounds(&model, body);
    assert!(near(lo, v(0.0, 0.0, 0.0)) && near(hi, v(mm(20.0), mm(20.0), mm(5.0))));
}

#[test]
fn emboss_keeps_counter_at_the_face() {
    let (mut model, body, faces) = plate();
    let counter = rect(7.0, 7.0, 13.0, 13.0, 5.0);
    let loops = vec![rect(4.0, 4.0, 16.0, 16.0, 5.0), rev(counter.clone())];
    let made = run(
        &mut model,
        body,
        faces[1],
        &loops,
        TextMode::Emboss,
        mm(1.0),
    )
    .unwrap();
    let r = report(&model, body);
    assert!(r.is_clean(), "{r:?}");
    assert!(close(r.volume.unwrap(), mm3(2000.0 + 108.0)));
    assert_eq!(made.caps.len(), 1);
    assert_eq!(made.sides.len(), 8);
    let cap = geometry(&model, body).face(made.caps[0]).unwrap().clone();
    assert_eq!(cap.inner.len(), 1);
    // The counter stays a face of its own at the old height.
    let left = faces_with_outer(&model, body, &counter);
    assert_eq!(left.len(), 1);
    assert!(!made.caps.contains(&left[0]));
}

#[test]
fn engrave_leaves_counter_standing() {
    let (mut model, body, faces) = plate();
    let counter = rect(7.0, 7.0, 13.0, 13.0, 5.0);
    let loops = vec![rect(4.0, 4.0, 16.0, 16.0, 5.0), rev(counter.clone())];
    let made = run(
        &mut model,
        body,
        faces[1],
        &loops,
        TextMode::Engrave,
        mm(2.0),
    )
    .unwrap();
    let r = report(&model, body);
    assert!(r.is_clean(), "{r:?}");
    assert!(close(r.volume.unwrap(), mm3(2000.0 - 216.0)));
    assert!(all_at_z(&face_points(&model, body, made.caps[0]), mm(3.0)));
    assert_eq!(faces_with_outer(&model, body, &counter).len(), 1);
}

#[test]
fn island_in_a_counter_is_a_letter() {
    let (mut model, body, faces) = plate();
    let loops = vec![
        rect(2.0, 2.0, 18.0, 18.0, 5.0),
        rev(rect(5.0, 5.0, 15.0, 15.0, 5.0)),
        rect(8.0, 8.0, 12.0, 12.0, 5.0),
    ];
    let made = run(
        &mut model,
        body,
        faces[1],
        &loops,
        TextMode::Emboss,
        mm(1.0),
    )
    .unwrap();
    let r = report(&model, body);
    assert!(r.is_clean(), "{r:?}");
    assert!(close(r.volume.unwrap(), mm3(2000.0 + 256.0 - 100.0 + 16.0)));
    assert_eq!(made.caps.len(), 2);
    assert_eq!(made.sides.len(), 12);
    for &cap in &made.caps {
        assert!(all_at_z(&face_points(&model, body, cap), mm(6.0)));
    }
    // Loop order: the outer letter first, then the island.
    let c1 = centroid(&face_points(&model, body, made.caps[1]));
    assert!(near(c1, v(mm(10.0), mm(10.0), mm(6.0))));
}

#[test]
fn letters_come_back_in_loop_order() {
    let (mut model, body, faces) = plate();
    let loops = vec![
        rect(13.0, 5.0, 16.0, 15.0, 5.0),
        rect(3.0, 5.0, 6.0, 15.0, 5.0),
        rect(8.0, 5.0, 11.0, 15.0, 5.0),
    ];
    let made = run(
        &mut model,
        body,
        faces[1],
        &loops,
        TextMode::Emboss,
        mm(0.5),
    )
    .unwrap();
    assert_eq!(made.caps.len(), 3);
    assert_eq!(made.sides.len(), 12);
    for (cap, l) in made.caps.iter().zip(&loops) {
        let c = centroid(&face_points(&model, body, *cap));
        assert!(near(c, centroid(l) + v(0.0, 0.0, mm(0.5))), "{c:?}");
    }
    assert!(close(
        report(&model, body).volume.unwrap(),
        mm3(2000.0 + 3.0 * 30.0 * 0.5)
    ));
}

#[test]
fn winding_does_not_matter() {
    let (mut model, body, faces) = plate();
    let loops = vec![
        rev(rect(4.0, 4.0, 16.0, 16.0, 5.0)),
        rect(7.0, 7.0, 13.0, 13.0, 5.0),
    ];
    run(
        &mut model,
        body,
        faces[1],
        &loops,
        TextMode::Emboss,
        mm(1.0),
    )
    .unwrap();
    let r = report(&model, body);
    assert!(r.is_clean(), "{r:?}");
    assert!(close(r.volume.unwrap(), mm3(2000.0 + 108.0)));
}

#[test]
fn text_on_a_side_face() {
    let (mut model, body, faces) = plate();
    // The front face is y = 0, front towards -y.
    let loops = vec![vec![
        v(mm(4.0), 0.0, mm(1.0)),
        v(mm(4.0), 0.0, mm(4.0)),
        v(mm(16.0), 0.0, mm(4.0)),
        v(mm(16.0), 0.0, mm(1.0)),
    ]];
    let made = run(
        &mut model,
        body,
        faces[2],
        &loops,
        TextMode::Emboss,
        mm(1.5),
    )
    .unwrap();
    let r = report(&model, body);
    assert!(r.is_clean(), "{r:?}");
    assert!(close(r.volume.unwrap(), mm3(2000.0 + 36.0 * 1.5)));
    let (lo, _) = bounds(&model, body);
    assert!((lo.y + mm(1.5)).abs() <= 1e-9);
    assert!(
        face_points(&model, body, made.caps[0])
            .iter()
            .all(|p| (p.y + mm(1.5)).abs() <= 1e-9)
    );
}

#[test]
fn reversed_face_embosses_away_from_the_material() {
    let mut model = Model::new();
    let mut fs = box_faces(0);
    fs[1] = reversed(&fs[1]);
    let (body, faces) = part_with_ids(
        &mut model,
        &box_points(v(0.0, 0.0, 0.0), v(mm(20.0), mm(20.0), mm(5.0))),
        &plain(&fs),
    );
    assert_eq!(report(&model, body).reversed_faces, vec![faces[1]]);
    let loops = vec![rect(5.0, 5.0, 15.0, 9.0, 5.0)];
    run(
        &mut model,
        body,
        faces[1],
        &loops,
        TextMode::Emboss,
        mm(1.0),
    )
    .unwrap();
    let r = report(&model, body);
    assert!(r.is_solid(), "{r:?}");
    assert!(close(r.volume.unwrap(), mm3(2040.0)));
    assert!((bounds(&model, body).1.z - mm(6.0)).abs() <= 1e-9);
}

#[test]
fn reversed_face_engraves_into_the_material() {
    let mut model = Model::new();
    let mut fs = box_faces(0);
    fs[1] = reversed(&fs[1]);
    let (body, faces) = part_with_ids(
        &mut model,
        &box_points(v(0.0, 0.0, 0.0), v(mm(20.0), mm(20.0), mm(5.0))),
        &plain(&fs),
    );
    let loops = vec![rect(5.0, 5.0, 15.0, 9.0, 5.0)];
    run(
        &mut model,
        body,
        faces[1],
        &loops,
        TextMode::Engrave,
        mm(1.0),
    )
    .unwrap();
    let r = report(&model, body);
    assert!(r.is_solid(), "{r:?}");
    assert!(close(r.volume.unwrap(), mm3(2000.0 - 40.0)));
    assert!((bounds(&model, body).1.z - mm(5.0)).abs() <= 1e-9);
}

#[test]
fn lone_face_has_no_depth_limit() {
    let mut model = Model::new();
    let (body, faces) = part_with_ids(
        &mut model,
        &[
            v(0.0, 0.0, 0.0),
            v(mm(20.0), 0.0, 0.0),
            v(mm(20.0), mm(20.0), 0.0),
            v(0.0, mm(20.0), 0.0),
        ],
        &plain(&[vec![0, 1, 2, 3]]),
    );
    let loops = vec![rect(5.0, 5.0, 15.0, 9.0, 0.0)];
    let made = run(
        &mut model,
        body,
        faces[0],
        &loops,
        TextMode::Engrave,
        mm(50.0),
    )
    .unwrap();
    assert!(all_at_z(
        &face_points(&model, body, made.caps[0]),
        mm(-50.0)
    ));
    assert_eq!(made.repaired, None);
}

#[test]
fn emboss_has_no_height_limit() {
    let (mut model, body, faces) = plate();
    let loops = vec![rect(5.0, 5.0, 15.0, 9.0, 5.0)];
    run(
        &mut model,
        body,
        faces[1],
        &loops,
        TextMode::Emboss,
        mm(50.0),
    )
    .unwrap();
    assert!(close(
        report(&model, body).volume.unwrap(),
        mm3(2000.0 + 2000.0)
    ));
}

#[test]
fn one_undo_step_puts_everything_back() {
    let (mut model, body, faces) = plate();
    let before = model.clone();
    let loops = vec![
        rect(2.0, 2.0, 18.0, 18.0, 5.0),
        rev(rect(5.0, 5.0, 15.0, 15.0, 5.0)),
        rect(8.0, 8.0, 12.0, 12.0, 5.0),
    ];
    let mut undo = UndoStack::new();
    text_3d(
        &mut model,
        body,
        faces[1],
        &loops,
        TextMode::Engrave,
        mm(1.0),
        &mut undo,
        LABEL,
    )
    .unwrap();
    let after = model.clone();
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo(&mut model).unwrap().as_deref(), Some(LABEL));
    assert!(model == before);
    undo.redo(&mut model).unwrap();
    assert!(model == after);
}

#[test]
fn check_text_changes_nothing() {
    let (model, body, faces) = plate();
    let loops = vec![rect(5.0, 5.0, 15.0, 9.0, 5.0)];
    let before = model.clone();
    let ctx = geometry(&model, body);
    assert_eq!(
        check_text(ctx, faces[1], &loops, TextMode::Engrave, mm(1.0)),
        Ok(())
    );
    assert!(model == before);
}

// ---- Refusals ----

#[test]
fn missing_body_and_face() {
    let (mut model, body, faces) = plate();
    let loops = vec![rect(5.0, 5.0, 15.0, 9.0, 5.0)];
    let gone = BodyId::Definition(DefinitionId(9999));
    let mut undo = UndoStack::new();
    let before = model.clone();
    assert_eq!(
        text_3d(
            &mut model,
            gone,
            faces[1],
            &loops,
            TextMode::Emboss,
            mm(1.0),
            &mut undo,
            LABEL
        ),
        Err(Text3dError::MissingBody(gone))
    );
    assert!(model == before);

    let mut lone = Model::new();
    let (lone_body, _) = part_with_ids(
        &mut lone,
        &[v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0)],
        &plain(&[vec![0, 1, 2]]),
    );
    let missing = faces[5];
    assert!(geometry(&lone, lone_body).face(missing).is_none());
    assert_eq!(
        run(
            &mut lone,
            lone_body,
            missing,
            &loops,
            TextMode::Emboss,
            mm(1.0)
        ),
        Err(Text3dError::MissingFace(missing))
    );
    let _ = body;
}

#[test]
fn bad_height() {
    let (mut model, body, faces) = plate();
    let loops = vec![rect(5.0, 5.0, 15.0, 9.0, 5.0)];
    for h in [0.0, -mm(1.0), f64::NAN, f64::INFINITY, 1e-6] {
        for mode in [TextMode::Emboss, TextMode::Engrave] {
            assert_eq!(
                run(&mut model, body, faces[1], &loops, mode, h),
                Err(Text3dError::BadHeight),
                "{h} {mode:?}"
            );
        }
    }
}

#[test]
fn no_text() {
    let (mut model, body, faces) = plate();
    assert_eq!(
        run(&mut model, body, faces[1], &[], TextMode::Emboss, mm(1.0)),
        Err(Text3dError::NoText)
    );
}

#[test]
fn bad_loops() {
    let (mut model, body, faces) = plate();
    let good = rect(2.0, 2.0, 4.0, 4.0, 5.0);
    let two = vec![
        v(mm(10.0), mm(10.0), mm(5.0)),
        v(mm(12.0), mm(10.0), mm(5.0)),
    ];
    let bowtie = vec![
        v(mm(10.0), mm(10.0), mm(5.0)),
        v(mm(14.0), mm(14.0), mm(5.0)),
        v(mm(14.0), mm(10.0), mm(5.0)),
        v(mm(10.0), mm(14.0), mm(5.0)),
    ];
    let mut repeated = rect(10.0, 10.0, 14.0, 14.0, 5.0);
    repeated.insert(1, repeated[0]);
    let flat = vec![
        v(mm(10.0), mm(10.0), mm(5.0)),
        v(mm(12.0), mm(10.0), mm(5.0)),
        v(mm(14.0), mm(10.0), mm(5.0)),
    ];
    for bad in [two, bowtie, repeated, flat] {
        assert_eq!(
            run(
                &mut model,
                body,
                faces[1],
                &[good.clone(), bad.clone()],
                TextMode::Emboss,
                mm(1.0)
            ),
            Err(Text3dError::BadLoop(1)),
            "{bad:?}"
        );
    }
}

#[test]
fn off_plane() {
    let (mut model, body, faces) = plate();
    let lifted = rect(10.0, 10.0, 14.0, 14.0, 5.1);
    let mut tilted = rect(10.0, 10.0, 14.0, 14.0, 5.0);
    tilted[2].z += mm(0.05);
    for bad in [lifted, tilted] {
        assert_eq!(
            run(
                &mut model,
                body,
                faces[1],
                &[rect(2.0, 2.0, 4.0, 4.0, 5.0), bad],
                TextMode::Emboss,
                mm(1.0)
            ),
            Err(Text3dError::OffPlane(1))
        );
    }
    // Checked loop by loop: a bad loop 0 is found before an off-plane loop 1.
    assert_eq!(
        run(
            &mut model,
            body,
            faces[1],
            &[
                rect(2.0, 2.0, 2.0, 4.0, 5.0),
                rect(10.0, 10.0, 14.0, 14.0, 6.0)
            ],
            TextMode::Emboss,
            mm(1.0)
        ),
        Err(Text3dError::BadLoop(0))
    );
}

#[test]
fn loops_that_cross_or_touch() {
    let (mut model, body, faces) = plate();
    let a = rect(2.0, 2.0, 6.0, 6.0, 5.0);
    let far = rect(14.0, 14.0, 16.0, 16.0, 5.0);
    let cases = [
        rect(4.0, 4.0, 8.0, 8.0, 5.0), // overlaps
        rect(6.0, 2.0, 9.0, 6.0, 5.0), // shares a side
        rect(6.0, 6.0, 9.0, 9.0, 5.0), // shares a corner
        rect(3.0, 3.0, 6.0, 5.0, 5.0), // inside, touching a side
    ];
    for c in cases {
        assert_eq!(
            run(
                &mut model,
                body,
                faces[1],
                &[a.clone(), far.clone(), c.clone()],
                TextMode::Emboss,
                mm(1.0)
            ),
            Err(Text3dError::LoopsCross(0, 2)),
            "{c:?}"
        );
    }
}

#[test]
fn face_too_small() {
    let (mut model, body, faces) = plate();
    // Longer than the face's diagonal (28.3 mm), though it is only 1 mm high.
    let long = rect(-5.0, 9.0, 25.0, 10.0, 5.0);
    for mode in [TextMode::Emboss, TextMode::Engrave] {
        assert_eq!(
            run(
                &mut model,
                body,
                faces[1],
                std::slice::from_ref(&long),
                mode,
                mm(1.0)
            ),
            Err(Text3dError::TooSmall)
        );
    }
    // Spread over several letters.
    assert_eq!(
        run(
            &mut model,
            body,
            faces[1],
            &[
                rect(-6.0, 9.0, -4.0, 10.0, 5.0),
                rect(24.0, 9.0, 26.0, 10.0, 5.0)
            ],
            TextMode::Emboss,
            mm(1.0)
        ),
        Err(Text3dError::TooSmall)
    );
}

#[test]
fn long_text_fits_along_the_diagonal() {
    let (mut model, body, faces) = plate();
    // 25 x 1 mm, turned 45 degrees about the face's center.
    let d = DVec2::new(1.0, 1.0).normalize();
    let n = DVec2::new(-d.y, d.x);
    let corner = |s: f64, t: f64| {
        let p = DVec2::new(10.0, 10.0) + d * s + n * t;
        v(mm(p.x), mm(p.y), mm(5.0))
    };
    let loops = vec![vec![
        corner(-12.5, -0.5),
        corner(12.5, -0.5),
        corner(12.5, 0.5),
        corner(-12.5, 0.5),
    ]];
    run(
        &mut model,
        body,
        faces[1],
        &loops,
        TextMode::Emboss,
        mm(1.0),
    )
    .unwrap();
    assert!(close(
        report(&model, body).volume.unwrap(),
        mm3(2000.0 + 25.0)
    ));
}

#[test]
fn crosses_the_border() {
    let (mut model, body, faces) = plate();
    let a = rect(2.0, 2.0, 4.0, 4.0, 5.0);
    let cases = [
        rect(16.0, 8.0, 22.0, 12.0, 5.0),  // hangs over x = 20
        rect(0.0, 8.0, 4.0, 12.0, 5.0),    // touches x = 0
        rect(-6.0, 8.0, -2.0, 12.0, 5.0),  // off the face altogether
        rect(16.0, 16.0, 20.0, 20.0, 5.0), // in the corner
    ];
    for c in cases {
        assert_eq!(
            run(
                &mut model,
                body,
                faces[1],
                &[a.clone(), c.clone()],
                TextMode::Emboss,
                mm(1.0)
            ),
            Err(Text3dError::CrossesBorder(1)),
            "{c:?}"
        );
    }
}

#[test]
fn edge_across_a_notch_crosses_the_border() {
    // L-shaped plate: the square 10..20 x 10..20 is cut away.
    let base: Vec<DVec2> = [
        (0.0, 0.0),
        (20.0, 0.0),
        (20.0, 10.0),
        (10.0, 10.0),
        (10.0, 20.0),
        (0.0, 20.0),
    ]
    .iter()
    .map(|&(x, y)| DVec2::new(mm(x), mm(y)))
    .collect();
    let (points, fs) = prism(&base, 0.0, mm(5.0));
    let mut model = Model::new();
    let (body, faces) = part_with_ids(&mut model, &points, &plain(&fs));
    // Every corner is on the plate, but the edge from (18, 8) to (5, 18) passes the notch.
    let tri = vec![
        v(mm(12.0), mm(5.0), mm(5.0)),
        v(mm(18.0), mm(8.0), mm(5.0)),
        v(mm(5.0), mm(18.0), mm(5.0)),
    ];
    assert_eq!(
        run(
            &mut model,
            body,
            faces[1],
            &[tri],
            TextMode::Engrave,
            mm(1.0)
        ),
        Err(Text3dError::CrossesBorder(0))
    );
    // The same plate takes a letter that stays on it.
    let made = run(
        &mut model,
        body,
        faces[1],
        &[rect(2.0, 12.0, 8.0, 18.0, 5.0)],
        TextMode::Engrave,
        mm(1.0),
    )
    .unwrap();
    assert_eq!(made.caps.len(), 1);
    assert!(close(
        report(&model, body).volume.unwrap(),
        mm3(1500.0 - 36.0)
    ));
}

#[test]
fn engrave_too_deep() {
    let (mut model, body, faces) = plate();
    let loops = vec![rect(5.0, 5.0, 15.0, 9.0, 5.0)];
    for depth in [5.0, 6.0] {
        match run(
            &mut model,
            body,
            faces[1],
            &loops,
            TextMode::Engrave,
            mm(depth),
        ) {
            Err(Text3dError::TooDeep { thickness }) => {
                assert!((thickness - mm(5.0)).abs() <= 1e-9, "{thickness}")
            }
            other => panic!("{depth}: {other:?}"),
        }
    }
    run(
        &mut model,
        body,
        faces[1],
        &loops,
        TextMode::Engrave,
        mm(4.9),
    )
    .unwrap();
}

#[test]
fn thickness_is_measured_under_the_text() {
    // Profile: 10 mm tall; underneath, the right half is cut up to 6 mm. The top is the
    // profile edge (20, 10) -> (0, 10), face 2 + 4.
    let profile = [
        (0.0, 0.0),
        (10.0, 0.0),
        (10.0, 6.0),
        (20.0, 6.0),
        (20.0, 10.0),
        (0.0, 10.0),
    ];
    let (mut model, body, faces) = profile_part(&profile);
    let top = faces[6];
    assert!(all_at_z(&face_points(&model, body, top), mm(10.0)));
    let right = rect(12.0, -15.0, 18.0, -5.0, 10.0);
    match run(
        &mut model,
        body,
        top,
        std::slice::from_ref(&right),
        TextMode::Engrave,
        mm(4.5),
    ) {
        Err(Text3dError::TooDeep { thickness }) => {
            assert!((thickness - mm(4.0)).abs() <= 1e-9, "{thickness}")
        }
        other => panic!("{other:?}"),
    }
    // A letter spanning both halves is limited by the thin one.
    let both = rect(5.0, -15.0, 15.0, -5.0, 10.0);
    assert!(matches!(
        run(&mut model, body, top, &[both], TextMode::Engrave, mm(4.5)),
        Err(Text3dError::TooDeep { .. })
    ));
    let left = rect(2.0, -15.0, 8.0, -5.0, 10.0);
    run(&mut model, body, top, &[left], TextMode::Engrave, mm(4.5)).unwrap();
    assert!(report(&model, body).is_clean());
}

#[test]
fn emboss_into_other_geometry_intersects() {
    // A C-shaped profile: a lower jaw (top at 4 mm) under an upper jaw (underside at 8 mm).
    let profile = [
        (0.0, 0.0),
        (20.0, 0.0),
        (20.0, 4.0),
        (8.0, 4.0),
        (8.0, 8.0),
        (20.0, 8.0),
        (20.0, 12.0),
        (0.0, 12.0),
    ];
    let (mut model, body, faces) = profile_part(&profile);
    // The lower jaw's top is the profile edge (20, 4) -> (8, 4): face 2 + 2.
    let jaw = faces[4];
    assert!(all_at_z(&face_points(&model, body, jaw), mm(4.0)));
    let loops = vec![rect(11.0, -15.0, 17.0, -5.0, 4.0)];
    assert_eq!(
        run(&mut model, body, jaw, &loops, TextMode::Emboss, mm(5.0)),
        Err(Text3dError::Intersects)
    );
    let volume = report(&model, body).volume.unwrap();
    run(&mut model, body, jaw, &loops, TextMode::Emboss, mm(3.0)).unwrap();
    let r = report(&model, body);
    assert!(r.is_clean(), "{r:?}");
    assert!(close(r.volume.unwrap(), volume + mm3(60.0 * 3.0)));
}

#[test]
fn emboss_up_to_a_facing_face_joins_it() {
    // Same C: a letter pushed exactly to the upper jaw's underside meets it the way
    // push/pull does (a post between the jaws), and the part stays one solid.
    let profile = [
        (0.0, 0.0),
        (20.0, 0.0),
        (20.0, 4.0),
        (8.0, 4.0),
        (8.0, 8.0),
        (20.0, 8.0),
        (20.0, 12.0),
        (0.0, 12.0),
    ];
    let (mut model, body, faces) = profile_part(&profile);
    let volume = report(&model, body).volume.unwrap();
    let loops = vec![rect(11.0, -15.0, 17.0, -5.0, 4.0)];
    run(
        &mut model,
        body,
        faces[4],
        &loops,
        TextMode::Emboss,
        mm(4.0),
    )
    .unwrap();
    let r = report(&model, body);
    assert!(r.is_solid(), "{r:?}");
    assert_eq!(r.shells, 1);
    assert!(close(r.volume.unwrap(), volume + mm3(60.0 * 4.0)));
}

// ---- Messages ----

#[test]
fn every_refusal_says_why() {
    let cases = [
        (Text3dError::MissingBody(BodyId::Root), "the part is gone"),
        (
            Text3dError::MissingFace(FaceId(0)),
            "pick a flat face to put the text on",
        ),
        (
            Text3dError::BadHeight,
            "the height must be a length above zero",
        ),
        (Text3dError::NoText, "there is no text to place"),
        (Text3dError::BadLoop(0), "a letter outline crosses itself"),
        (Text3dError::OffPlane(0), "the text is not flat on the face"),
        (
            Text3dError::LoopsCross(0, 1),
            "two letters touch or overlap",
        ),
        (Text3dError::TooSmall, "the face is too small for the text"),
        (
            Text3dError::CrossesBorder(0),
            "the text runs over the edge of the face",
        ),
        (
            Text3dError::TooDeep { thickness: 0.001 },
            "the engraving is as deep as the part is thick",
        ),
        (
            Text3dError::Intersects,
            "the letters would cut into other geometry",
        ),
        (
            Text3dError::Draw(StickyError::Degenerate),
            "the letters could not be drawn on the face",
        ),
        (
            Text3dError::PushPull(PushPullError::BadDistance),
            "push/pull refused a letter",
        ),
        (
            Text3dError::NotSolid(Box::default()),
            "the part would no longer be a solid",
        ),
        (
            Text3dError::Model(ModelError::Mismatch),
            "the model could not be changed",
        ),
    ];
    for (e, text) in cases {
        assert_eq!(e.to_string(), text);
    }
}
