// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Overlay primitives (`docs/specs/m3-ui-overlay.md`).

use drydock::overlay::*;
use drydock_kernel::model::UndoStack;
use drydock_kernel::model::{BodyId, Model, ModelEdit};
use drydock_kernel::tape::{Guide, Guides};
use drydock_kernel::tools::plane::DrawPlane;
use drydock_kernel::tools::scale::{Bounds, Handle, handles};
use drydock_kernel::tools::{Target, draw_segments};
use drydock_kernel::topology::{Context, FaceId};
use drydock_kernel::units::Unit;
use glam::DVec3;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn near(a: DVec3, b: DVec3) -> bool {
    a.distance(b) < 1e-9
}

fn ground() -> DrawPlane {
    DrawPlane {
        u: DVec3::X,
        v: DVec3::Y,
    }
}

fn lines(prims: &[Primitive], style: Style) -> Vec<(DVec3, DVec3)> {
    prims
        .iter()
        .filter_map(|p| match p {
            Primitive::Line { a, b, style: s } if *s == style => Some((*a, *b)),
            _ => None,
        })
        .collect()
}

fn markers(prims: &[Primitive], style: Style) -> Vec<DVec3> {
    prims
        .iter()
        .filter_map(|p| match p {
            Primitive::Marker { at, style: s } if *s == style => Some(*at),
            _ => None,
        })
        .collect()
}

// ---- face_highlight -----------------------------------------------------------------

fn draw(model: &mut Model, loops: &[Vec<DVec3>]) {
    let mut segs = Vec::new();
    for pts in loops {
        for i in 0..pts.len() {
            segs.push((pts[i], pts[(i + 1) % pts.len()]));
        }
    }
    let mut undo = UndoStack::new();
    let mut t = Target {
        model,
        body: BodyId::Root,
        undo: &mut undo,
        unit: Unit::Meter,
    };
    draw_segments(&mut t, &segs, "Draw").expect("draw");
}

fn only_face(ctx: &Context, area_point: DVec3) -> FaceId {
    ctx.faces()
        .map(|(id, _)| id)
        .find(|id| {
            let f = ctx.face(*id).expect("face");
            let pts = ctx.loop_points(&f.outer);
            pts.iter().all(|p| (p.z - area_point.z).abs() < 1e-9)
                && pts.len() == 4
                && pts.iter().any(|p| near(*p, area_point))
        })
        .expect("face")
}

#[test]
fn highlight_is_the_outer_loop_in_loop_order() {
    let mut model = Model::new();
    draw(
        &mut model,
        &[vec![
            v(0.0, 0.0, 0.0),
            v(2.0, 0.0, 0.0),
            v(2.0, 2.0, 0.0),
            v(0.0, 2.0, 0.0),
        ]],
    );
    let ctx = model.root().geometry();
    let face = only_face(ctx, v(0.0, 0.0, 0.0));
    let out = face_highlight(ctx, face);
    assert_eq!(out.len(), 1);
    let Primitive::Polygon { points, style } = &out[0] else {
        panic!("polygon expected");
    };
    assert_eq!(*style, Style::FaceHighlight);
    let expect = ctx.loop_points(&ctx.face(face).expect("face").outer);
    assert_eq!(points.len(), 4);
    assert_eq!(points, &expect);
}

#[test]
fn highlight_lists_holes_after_the_outer_loop() {
    let mut model = Model::new();
    draw(
        &mut model,
        &[
            vec![
                v(0.0, 0.0, 0.0),
                v(4.0, 0.0, 0.0),
                v(4.0, 4.0, 0.0),
                v(0.0, 4.0, 0.0),
            ],
            vec![
                v(1.0, 1.0, 0.0),
                v(2.0, 1.0, 0.0),
                v(2.0, 2.0, 0.0),
                v(1.0, 2.0, 0.0),
            ],
        ],
    );
    let ctx = model.root().geometry();
    // The big face has one hole; the small square is a face of its own.
    let big = ctx
        .faces()
        .map(|(id, _)| id)
        .find(|id| !ctx.face(*id).expect("face").inner.is_empty())
        .expect("face with a hole");
    let out = face_highlight(ctx, big);
    assert_eq!(out.len(), 2);
    let face = ctx.face(big).expect("face");
    for (i, prim) in out.iter().enumerate() {
        let Primitive::Polygon { points, style } = prim else {
            panic!("polygon expected");
        };
        assert_eq!(*style, Style::FaceHighlight);
        let expect = if i == 0 {
            ctx.loop_points(&face.outer)
        } else {
            ctx.loop_points(&face.inner[0])
        };
        assert_eq!(points, &expect);
    }
}

#[test]
fn highlight_of_an_unknown_face_is_empty() {
    let model = Model::new();
    assert!(face_highlight(model.root().geometry(), FaceId(99)).is_empty());
}

// ---- scale_handles ------------------------------------------------------------------

fn cube() -> Bounds {
    Bounds {
        min: v(0.0, 0.0, 0.0),
        max: v(2.0, 4.0, 6.0),
    }
}

#[test]
fn box_has_twelve_edges_along_the_axes() {
    let out = scale_handles(&cube(), None, None);
    let box_lines = lines(&out, Style::BoundsBox);
    assert_eq!(box_lines.len(), 12);
    let mut along = [0, 0, 0];
    for (a, b) in &box_lines {
        let d = (*b - *a).abs();
        let axis = (0..3).filter(|i| d[*i] > 1e-9).collect::<Vec<_>>();
        assert_eq!(axis.len(), 1, "edge is axis aligned");
        along[axis[0]] += 1;
        let lo = a.min(*b);
        let hi = a.max(*b);
        assert!(lo.cmpge(cube().min).all() && hi.cmple(cube().max).all());
    }
    assert_eq!(along, [4, 4, 4]);
    // Lines come first, markers after.
    let first_marker = out
        .iter()
        .position(|p| matches!(p, Primitive::Marker { .. }))
        .expect("markers");
    assert_eq!(first_marker, 12);
}

#[test]
fn box_edges_are_distinct() {
    let box_lines = lines(&scale_handles(&cube(), None, None), Style::BoundsBox);
    for i in 0..box_lines.len() {
        for j in i + 1..box_lines.len() {
            let (a, b) = box_lines[i];
            let (c, d) = box_lines[j];
            assert!(!(near(a, c) && near(b, d)) && !(near(a, d) && near(b, c)));
        }
    }
}

#[test]
fn flat_box_has_four_edges_and_a_line_has_one() {
    let flat = Bounds {
        min: v(0.0, 0.0, 1.0),
        max: v(2.0, 3.0, 1.0),
    };
    assert_eq!(
        lines(&scale_handles(&flat, None, None), Style::BoundsBox).len(),
        4
    );
    let line = Bounds {
        min: v(0.0, 0.0, 1.0),
        max: v(2.0, 0.0, 1.0),
    };
    assert_eq!(
        lines(&scale_handles(&line, None, None), Style::BoundsBox).len(),
        1
    );
    let point = Bounds {
        min: v(1.0, 1.0, 1.0),
        max: v(1.0, 1.0, 1.0),
    };
    assert!(lines(&scale_handles(&point, None, None), Style::BoundsBox).is_empty());
}

#[test]
fn one_marker_per_handle_in_handle_order() {
    let b = cube();
    let out = scale_handles(&b, None, None);
    let got = markers(&out, Style::Handle);
    let expect: Vec<_> = handles(&b).into_iter().map(|h| b.point(h)).collect();
    assert_eq!(got.len(), 26);
    assert_eq!(got, expect);
    assert!(markers(&out, Style::HandleHover).is_empty());
    assert!(markers(&out, Style::HandleGrabbed).is_empty());
}

#[test]
fn flat_box_offers_fewer_handles() {
    let flat = Bounds {
        min: v(0.0, 0.0, 1.0),
        max: v(2.0, 3.0, 1.0),
    };
    let out = scale_handles(&flat, None, None);
    assert_eq!(markers(&out, Style::Handle).len(), handles(&flat).len());
    assert_eq!(handles(&flat).len(), 8);
}

#[test]
fn hover_restyles_one_marker() {
    let b = cube();
    let h = Handle::new(1, 1, 1);
    let out = scale_handles(&b, Some(h), None);
    assert_eq!(markers(&out, Style::HandleHover), vec![b.point(h)]);
    assert_eq!(markers(&out, Style::Handle).len(), 25);
}

#[test]
fn grab_wins_over_hover_and_keeps_the_marker_count() {
    let b = cube();
    let hover = Handle::new(1, 1, 1);
    let grab = Handle::new(-1, 0, 0);
    let out = scale_handles(&b, Some(hover), Some(grab));
    assert_eq!(markers(&out, Style::HandleGrabbed), vec![b.point(grab)]);
    assert_eq!(markers(&out, Style::HandleHover), vec![b.point(hover)]);
    assert_eq!(markers(&out, Style::Handle).len(), 24);
    // Same handle hovered and grabbed: grabbed.
    let out = scale_handles(&b, Some(grab), Some(grab));
    assert_eq!(markers(&out, Style::HandleGrabbed), vec![b.point(grab)]);
    assert!(markers(&out, Style::HandleHover).is_empty());
}

#[test]
fn a_handle_the_box_does_not_have_changes_nothing() {
    let flat = Bounds {
        min: v(0.0, 0.0, 1.0),
        max: v(2.0, 3.0, 1.0),
    };
    let bogus = Handle::new(0, 0, 1);
    let out = scale_handles(&flat, Some(bogus), Some(bogus));
    assert_eq!(out, scale_handles(&flat, None, None));
}

// ---- protractor ---------------------------------------------------------------------

fn ring_point(center: DVec3, plane: DrawPlane, r: f64, deg: f64) -> DVec3 {
    let t = deg.to_radians();
    center + r * (t.cos() * plane.u + t.sin() * plane.v)
}

#[test]
fn ring_is_24_chords_in_the_plane() {
    let c = v(1.0, 2.0, 3.0);
    let out = protractor(c, ground(), 2.0, None, None);
    let ring = lines(&out, Style::ProtractorRing);
    assert_eq!(ring.len(), 24);
    for (k, (a, b)) in ring.iter().enumerate() {
        assert!(near(*a, ring_point(c, ground(), 2.0, 15.0 * k as f64)));
        assert!(near(
            *b,
            ring_point(c, ground(), 2.0, 15.0 * (k + 1) as f64)
        ));
    }
    assert_eq!(out.len(), 48, "no arms without a start");
}

#[test]
fn ticks_sit_on_the_ring_and_point_at_the_center() {
    let c = v(1.0, 2.0, 3.0);
    let out = protractor(c, ground(), 2.0, None, None);
    let ticks = lines(&out, Style::ProtractorTick);
    assert_eq!(ticks.len(), 24);
    for (k, (a, b)) in ticks.iter().enumerate() {
        let p = ring_point(c, ground(), 2.0, 15.0 * k as f64);
        assert!(near(*b, p));
        assert!(near(*a, c + 0.9 * (p - c)));
    }
}

#[test]
fn order_is_ring_ticks_arms() {
    let out = protractor(DVec3::ZERO, ground(), 1.0, Some(DVec3::X), Some(30.0));
    assert_eq!(out.len(), 50);
    let style_at = |i: usize| match &out[i] {
        Primitive::Line { style, .. } => *style,
        _ => panic!("line expected"),
    };
    assert_eq!(style_at(0), Style::ProtractorRing);
    assert_eq!(style_at(23), Style::ProtractorRing);
    assert_eq!(style_at(24), Style::ProtractorTick);
    assert_eq!(style_at(47), Style::ProtractorTick);
    assert_eq!(style_at(48), Style::ProtractorArm);
    assert_eq!(style_at(49), Style::ProtractorArm);
}

#[test]
fn start_arm_is_normalised_and_flattened() {
    let c = v(1.0, 1.0, 0.0);
    // Longer than the radius and tilted out of the plane.
    let out = protractor(c, ground(), 2.0, Some(v(5.0, 0.0, 3.0)), None);
    let arms = lines(&out, Style::ProtractorArm);
    assert_eq!(arms.len(), 1);
    assert!(near(arms[0].0, c));
    assert!(near(arms[0].1, c + v(2.0, 0.0, 0.0)));
}

#[test]
fn second_arm_turns_counter_clockwise_seen_from_the_front() {
    let c = DVec3::ZERO;
    let out = protractor(c, ground(), 2.0, Some(DVec3::X), Some(90.0));
    let arms = lines(&out, Style::ProtractorArm);
    assert_eq!(arms.len(), 2);
    assert!(near(arms[0].1, v(2.0, 0.0, 0.0)));
    assert!(near(arms[1].0, c));
    assert!(near(arms[1].1, v(0.0, 2.0, 0.0)));
    // Negative angles turn the other way.
    let out = protractor(c, ground(), 2.0, Some(DVec3::X), Some(-90.0));
    let arms = lines(&out, Style::ProtractorArm);
    assert!(near(arms[1].1, v(0.0, -2.0, 0.0)));
}

#[test]
fn plane_orientation_decides_the_turn() {
    // u = y, v = z: normal is +x. A quarter turn takes y to z.
    let plane = DrawPlane {
        u: DVec3::Y,
        v: DVec3::Z,
    };
    let out = protractor(DVec3::ZERO, plane, 1.0, Some(DVec3::Y), Some(90.0));
    let arms = lines(&out, Style::ProtractorArm);
    assert!(near(arms[1].1, v(0.0, 0.0, 1.0)));
    // Ring starts along u.
    let ring = lines(&out, Style::ProtractorRing);
    assert!(near(ring[0].0, v(0.0, 1.0, 0.0)));
}

#[test]
fn bad_inputs_give_less() {
    assert!(protractor(DVec3::ZERO, ground(), 0.0, None, None).is_empty());
    assert!(protractor(DVec3::ZERO, ground(), -1.0, None, None).is_empty());
    assert!(protractor(DVec3::ZERO, ground(), f64::NAN, None, None).is_empty());
    assert!(protractor(DVec3::ZERO, ground(), f64::INFINITY, None, None).is_empty());
    let none = protractor(DVec3::ZERO, ground(), 1.0, Some(DVec3::ZERO), Some(10.0));
    assert!(lines(&none, Style::ProtractorArm).is_empty());
    let along_normal = protractor(DVec3::ZERO, ground(), 1.0, Some(DVec3::Z), Some(10.0));
    assert!(lines(&along_normal, Style::ProtractorArm).is_empty());
    let nan_angle = protractor(DVec3::ZERO, ground(), 1.0, Some(DVec3::X), Some(f64::NAN));
    assert_eq!(lines(&nan_angle, Style::ProtractorArm).len(), 1);
    let nan_start = protractor(
        DVec3::ZERO,
        ground(),
        1.0,
        Some(v(f64::NAN, 0.0, 0.0)),
        Some(10.0),
    );
    assert!(lines(&nan_start, Style::ProtractorArm).is_empty());
}

#[test]
fn angle_without_start_draws_no_arms() {
    let out = protractor(DVec3::ZERO, ground(), 1.0, None, Some(45.0));
    assert!(lines(&out, Style::ProtractorArm).is_empty());
}

// ---- guides -------------------------------------------------------------------------

fn guide_set(list: &[Guide]) -> Guides {
    let mut g = Guides::new();
    for guide in list {
        g.add(*guide);
    }
    g
}

#[test]
fn line_guide_is_centred_on_the_closest_point() {
    let g = guide_set(&[Guide::Line {
        point: v(0.0, 3.0, 0.0),
        dir: DVec3::X,
    }]);
    let out = guides(&g, v(5.0, 0.0, 7.0), 10.0);
    assert_eq!(out.len(), 1);
    let Primitive::Line { a, b, style } = &out[0] else {
        panic!("line expected");
    };
    assert_eq!(*style, Style::Guide);
    assert!(near(*a, v(-5.0, 3.0, 0.0)));
    assert!(near(*b, v(15.0, 3.0, 0.0)));
}

#[test]
fn point_guides_are_markers_and_order_is_creation_order() {
    let g = guide_set(&[
        Guide::Point(v(1.0, 1.0, 1.0)),
        Guide::Line {
            point: DVec3::ZERO,
            dir: DVec3::Z,
        },
        Guide::Point(v(2.0, 2.0, 2.0)),
    ]);
    let out = guides(&g, DVec3::ZERO, 1.0);
    assert_eq!(out.len(), 3);
    assert_eq!(
        out[0],
        Primitive::Marker {
            at: v(1.0, 1.0, 1.0),
            style: Style::GuidePoint
        }
    );
    assert!(matches!(
        out[1],
        Primitive::Line {
            style: Style::Guide,
            ..
        }
    ));
    assert_eq!(
        out[2],
        Primitive::Marker {
            at: v(2.0, 2.0, 2.0),
            style: Style::GuidePoint
        }
    );
}

#[test]
fn no_length_drops_lines_but_keeps_points() {
    let g = guide_set(&[
        Guide::Line {
            point: DVec3::ZERO,
            dir: DVec3::Z,
        },
        Guide::Point(v(2.0, 2.0, 2.0)),
    ]);
    for half in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let out = guides(&g, DVec3::ZERO, half);
        assert_eq!(out.len(), 1, "half_length {half}");
        assert!(matches!(out[0], Primitive::Marker { .. }));
    }
}

#[test]
fn empty_guides_give_nothing() {
    assert!(guides(&Guides::new(), DVec3::ZERO, 5.0).is_empty());
}

#[test]
fn guides_in_a_model_are_drawn() {
    let mut model = Model::new();
    let mut edit = ModelEdit::new();
    model
        .add_guide(BodyId::Root, Guide::Point(v(1.0, 2.0, 3.0)), &mut edit)
        .expect("guide");
    let out = guides(model.root().guides(), DVec3::ZERO, 5.0);
    assert_eq!(markers(&out, Style::GuidePoint), vec![v(1.0, 2.0, 3.0)]);
}
