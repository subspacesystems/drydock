// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Property tests for grow and shrink (`docs/specs/m7-clearance-offset.md`).

use drydock_kernel::model::*;
use drydock_kernel::offset::{OffsetError, offset_bodies, offset_body};
use glam::{DMat4, DVec2, DVec3};
use proptest::prelude::*;

mod offset_common;
use offset_common::*;

/// A rigid placement: rotation about a random axis, then a shift of up to 0.5 m.
fn placement() -> impl Strategy<Value = DMat4> {
    (
        -1.0f64..1.0,
        -1.0f64..1.0,
        0.1f64..1.0,
        0.0f64..std::f64::consts::TAU,
        prop::array::uniform3(-0.5f64..0.5),
    )
        .prop_map(|(x, y, z, angle, t)| {
            DMat4::from_translation(DVec3::from_array(t))
                * DMat4::from_axis_angle(v(x, y, z).normalize(), angle)
        })
}

/// A convex polygon: `n` points on a circle of radius 10..40 mm at jittered even angles.
fn convex_polygon() -> impl Strategy<Value = Vec<DVec2>> {
    (3usize..9, 10.0f64..40.0).prop_flat_map(|(n, r)| {
        prop::collection::vec(-0.3f64..0.3, n).prop_map(move |jitter| {
            let step = std::f64::consts::TAU / n as f64;
            jitter
                .iter()
                .enumerate()
                .map(|(i, j)| {
                    let a = step * (i as f64 + j);
                    DVec2::new(mm(r) * a.cos(), mm(r) * a.sin())
                })
                .collect()
        })
    })
}

fn placed(m: DMat4, points: &[DVec3]) -> Vec<DVec3> {
    points.iter().map(|&p| m.transform_point3(p)).collect()
}

fn offset_ok(model: &mut Model, body: BodyId, d: f64) -> Result<(), TestCaseError> {
    let mut log = ModelEdit::new();
    let result = offset_body(model, body, d, &mut log);
    prop_assert!(result.is_ok(), "{result:?}");
    let violations = model.validate();
    prop_assert!(violations.is_empty(), "{violations:?}");
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn grown_box_has_the_box_formula_volume_and_bounds(
        size in prop::array::uniform3(1.0f64..50.0),
        lo in prop::array::uniform3(-100.0f64..100.0),
        d in 0.05f64..1.0,
    ) {
        let (size, lo, d) = (DVec3::from_array(size) * 0.001, DVec3::from_array(lo) * 0.001, mm(d));
        let mut model = Model::new();
        let body = box_part(&mut model, lo, lo + size);
        offset_ok(&mut model, body, d)?;
        let want = (size + DVec3::splat(2.0 * d)).element_product();
        prop_assert!(close(volume(&model, body), want), "{} vs {want}", volume(&model, body));
        let (got_lo, got_hi) = bounds(&model, body);
        prop_assert!(near(got_lo, lo - DVec3::splat(d)), "{got_lo:?}");
        prop_assert!(near(got_hi, lo + size + DVec3::splat(d)), "{got_hi:?}");
    }

    #[test]
    fn placed_box_grow_then_shrink_restores_it(
        size in prop::array::uniform3(1.0f64..50.0),
        m in placement(),
        d in 0.05f64..0.45,
    ) {
        let (size, d) = (DVec3::from_array(size) * 0.001, mm(d));
        let mut model = Model::new();
        let points = placed(m, &box_points(DVec3::ZERO, size));
        let body = part(&mut model, &points, &plain(&box_faces(0)));
        let before = positions(&model, body);
        let old = geometry(&model, body).clone();
        offset_ok(&mut model, body, d)?;
        planes_moved_by(&old, geometry(&model, body), d).map_err(TestCaseError::fail)?;
        let want = (size + DVec3::splat(2.0 * d)).element_product();
        prop_assert!(close(volume(&model, body), want));
        offset_ok(&mut model, body, -d)?;
        for (id, p) in positions(&model, body) {
            prop_assert!(near(p, before[&id]), "{id:?}: {p:?} vs {:?}", before[&id]);
        }
    }

    #[test]
    fn prism_grow_then_shrink_restores_it(
        base in convex_polygon(),
        height in 5.0f64..40.0,
        m in placement(),
        d in 0.05f64..1.0,
        shrink_first in any::<bool>(),
    ) {
        let d = mm(d);
        let (points, faces) = prism(&base, 0.0, mm(height));
        let mut model = Model::new();
        let body = part(&mut model, &placed(m, &points), &plain(&faces));
        let before = positions(&model, body);
        let old = geometry(&model, body).clone();
        let first = if shrink_first { -d } else { d };
        offset_ok(&mut model, body, first)?;
        planes_moved_by(&old, geometry(&model, body), first).map_err(TestCaseError::fail)?;
        offset_ok(&mut model, body, -first)?;
        for (id, p) in positions(&model, body) {
            prop_assert!(near(p, before[&id]), "{id:?}: {p:?} vs {:?}", before[&id]);
        }
    }

    #[test]
    fn shrinking_past_half_the_thinnest_side_is_refused(
        size in prop::array::uniform3(1.0f64..20.0),
        over in 1.01f64..3.0,
        m in placement(),
    ) {
        let size = DVec3::from_array(size) * 0.001;
        let d = size.min_element() / 2.0 * over;
        let mut model = Model::new();
        let points = placed(m, &box_points(DVec3::ZERO, size));
        let body = part(&mut model, &points, &plain(&box_faces(0)));
        let before = model.clone();
        let mut undo = UndoStack::new();
        let results = offset_bodies(&mut model, &[body], -d, &mut undo, "Shrink by Clearance");
        prop_assert!(
            matches!(results[0], Err(OffsetError::Collapse { ref edges, .. }) if !edges.is_empty()),
            "{results:?}"
        );
        prop_assert!(model == before);
        prop_assert_eq!(undo.undo_count(), 0);
    }

    #[test]
    fn cavity_shrinks_by_twice_d_when_the_part_grows(
        outer in 20.0f64..40.0,
        cavity in 2.0f64..15.0,
        d in 0.05f64..0.9,
    ) {
        let (outer, cavity, d) = (mm(outer), mm(cavity), mm(d));
        let c0 = DVec3::splat((outer - cavity) / 2.0);
        let mut points = box_points(DVec3::ZERO, DVec3::splat(outer));
        points.extend(box_points(c0, c0 + DVec3::splat(cavity)));
        let mut faces = box_faces(0);
        faces.extend(box_faces(8).iter().map(|f| reversed(f)));
        let mut model = Model::new();
        let body = part(&mut model, &points, &plain(&faces));
        offset_ok(&mut model, body, d)?;
        let c = geometry(&model, body);
        let lo = c.find_vertex(c0 + DVec3::splat(d), 1e-9);
        let hi = c.find_vertex(c0 + DVec3::splat(cavity - d), 1e-9);
        prop_assert!(lo.is_some() && hi.is_some(), "cavity corners not where expected");
        let want = (outer + 2.0 * d).powi(3) - (cavity - 2.0 * d).powi(3);
        prop_assert!(close(volume(&model, body), want), "{} vs {want}", volume(&model, body));
    }
}
