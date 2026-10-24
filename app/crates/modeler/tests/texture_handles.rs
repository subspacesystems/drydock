// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Texture handle geometry and overlay primitives (`docs/specs/m4-app-texture-handles.md`).

use glam::{DAffine2, DVec2, DVec3, DVec4};
use modeler::overlay::{Primitive, Style};
use modeler::texture_handles::*;
use modeler_kernel::model::{BodyId, Model, UndoStack};
use modeler_kernel::paint::Side;
use modeler_kernel::texture::TextureMap;
use modeler_kernel::tools::{Target, draw_segments};
use modeler_kernel::topology::FaceId;
use modeler_kernel::units::Unit;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn close3(a: DVec3, b: DVec3) -> bool {
    (a - b).length() < 1e-9
}

fn close2(a: DVec2, b: DVec2) -> bool {
    (a - b).length() < 1e-9
}

/// s = x, t = y.
fn plain() -> TextureMap {
    TextureMap {
        u: DVec4::new(1.0, 0.0, 0.0, 0.0),
        v: DVec4::new(0.0, 1.0, 0.0, 0.0),
    }
}

fn some_face() -> FaceId {
    let mut model = Model::new();
    let p = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(1.0, 1.0, 0.0),
        v(0.0, 1.0, 0.0),
    ];
    let segs: Vec<_> = (0..4).map(|i| (p[i], p[(i + 1) % 4])).collect();
    let mut undo = UndoStack::new();
    let mut t = Target {
        model: &mut model,
        body: BodyId::Root,
        undo: &mut undo,
        unit: Unit::Meter,
    };
    draw_segments(&mut t, &segs, "Draw").unwrap();
    model.root().geometry().faces().next().unwrap().0
}

#[test]
fn plane_point_at_inverts_the_map_on_the_ground() {
    let st = DVec2::new(2.5, -1.25);
    let p = plane_point_at(&plain(), v(0.0, 0.0, 0.0), DVec3::Z, st).unwrap();
    assert!(close3(p, v(2.5, -1.25, 0.0)), "{p}");
}

#[test]
fn plane_point_at_inverts_a_turned_scaled_map_on_a_tilted_plane() {
    let normal = v(1.0, 0.0, 1.0).normalize();
    let point = v(1.0, 2.0, 3.0);
    let map = TextureMap::default_for(normal).then(
        DAffine2::from_translation(DVec2::new(0.3, -0.7))
            * DAffine2::from_angle(0.6)
            * DAffine2::from_scale(DVec2::new(2.0, 0.5)),
    );
    for st in [DVec2::ZERO, DVec2::new(1.0, 2.0), DVec2::new(-3.5, 0.25)] {
        let p = plane_point_at(&map, point, normal, st).unwrap();
        assert!((p - point).dot(normal).abs() < 1e-9, "off the plane: {p}");
        assert!(close2(map.apply(p), st), "{} vs {st}", map.apply(p));
    }
}

#[test]
fn plane_point_at_refuses_degenerate_input() {
    let flat = TextureMap {
        u: DVec4::new(1.0, 0.0, 0.0, 0.0),
        v: DVec4::new(2.0, 0.0, 0.0, 0.0),
    };
    assert!(plane_point_at(&flat, DVec3::ZERO, DVec3::Z, DVec2::ONE).is_none());
    // The map only varies along the normal, so it is constant on the plane.
    let along_normal = TextureMap {
        u: DVec4::new(0.0, 0.0, 1.0, 0.0),
        v: DVec4::new(0.0, 0.0, 0.0, 1.0),
    };
    assert!(plane_point_at(&along_normal, DVec3::ZERO, DVec3::Z, DVec2::ONE).is_none());
    assert!(plane_point_at(&plain(), DVec3::ZERO, DVec3::ZERO, DVec2::ONE).is_none());
    assert!(plane_point_at(&plain(), DVec3::ZERO, DVec3::Z, DVec2::new(f64::NAN, 0.0)).is_none());
}

#[test]
fn tile_corners_are_the_image_copy_holding_the_center() {
    let corners = tile_corners(
        &plain(),
        DVec2::new(2.0, 1.0),
        DVec3::ZERO,
        DVec3::Z,
        v(2.5, 0.5, 0.0),
    )
    .unwrap();
    let expected = [
        v(2.0, 0.0, 0.0),
        v(4.0, 0.0, 0.0),
        v(4.0, 1.0, 0.0),
        v(2.0, 1.0, 0.0),
    ];
    for (got, want) in corners.iter().zip(expected) {
        assert!(close3(*got, want), "{corners:?}");
    }
}

#[test]
fn tile_corners_round_down_for_negative_coordinates() {
    let corners = tile_corners(
        &plain(),
        DVec2::ONE,
        DVec3::ZERO,
        DVec3::Z,
        v(-0.5, -1.5, 0.0),
    )
    .unwrap();
    let expected = [
        v(-1.0, -2.0, 0.0),
        v(0.0, -2.0, 0.0),
        v(0.0, -1.0, 0.0),
        v(-1.0, -1.0, 0.0),
    ];
    for (got, want) in corners.iter().zip(expected) {
        assert!(close3(*got, want), "{corners:?}");
    }
}

#[test]
fn tile_corners_follow_a_turned_map() {
    // The image turned a quarter: s = y, t = -x.
    let map = plain().then(DAffine2::from_angle(-std::f64::consts::FRAC_PI_2));
    let corners = tile_corners(&map, DVec2::ONE, DVec3::ZERO, DVec3::Z, v(0.5, -0.5, 0.0)).unwrap();
    let a = map.apply(corners[0]);
    assert!(close2(a, DVec2::new(-1.0, -1.0)), "{a}");
    assert!(close2(map.apply(corners[1]), a + DVec2::new(1.0, 0.0)));
    assert!(close2(map.apply(corners[2]), a + DVec2::new(1.0, 1.0)));
    assert!(close2(map.apply(corners[3]), a + DVec2::new(0.0, 1.0)));
}

#[test]
fn tile_corners_refuse_a_bad_size() {
    for size in [
        DVec2::new(0.0, 1.0),
        DVec2::new(1.0, -1.0),
        DVec2::new(f64::NAN, 1.0),
        DVec2::new(1.0, f64::INFINITY),
    ] {
        assert!(
            tile_corners(&plain(), size, DVec3::ZERO, DVec3::Z, v(0.5, 0.5, 0.0)).is_none(),
            "{size}"
        );
    }
}

fn handles(hover: Option<Grip>, held: Option<Grip>) -> TextureHandles {
    TextureHandles {
        face: some_face(),
        side: Side::Front,
        corners: [
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(1.0, 1.0, 0.0),
            v(0.0, 1.0, 0.0),
        ],
        hover,
        held,
    }
}

fn marker_styles(primitives: &[Primitive]) -> Vec<(DVec3, Style)> {
    primitives
        .iter()
        .filter_map(|p| match p {
            Primitive::Marker { at, style } => Some((*at, *style)),
            _ => None,
        })
        .collect()
}

#[test]
fn primitives_outline_the_image_and_mark_pin_turn_and_scale() {
    let h = handles(None, None);
    let primitives = primitives(&h);
    assert_eq!(primitives.len(), 4, "{primitives:?}");
    assert_eq!(
        primitives[0],
        Primitive::Polygon {
            points: h.corners.to_vec(),
            style: Style::BoundsBox,
        }
    );
    assert_eq!(
        marker_styles(&primitives),
        vec![
            (h.corners[0], Style::Handle),
            (h.corners[1], Style::Handle),
            (h.corners[2], Style::Handle),
        ]
    );
}

#[test]
fn hovered_grip_marker_is_highlighted() {
    let h = handles(Some(Grip::Turn), None);
    assert_eq!(
        marker_styles(&primitives(&h)),
        vec![
            (h.corners[0], Style::Handle),
            (h.corners[1], Style::HandleHover),
            (h.corners[2], Style::Handle),
        ]
    );
    let h = handles(Some(Grip::Slide), None);
    assert!(
        marker_styles(&primitives(&h))
            .iter()
            .all(|(_, style)| *style == Style::Handle)
    );
}

#[test]
fn held_grip_marker_is_grabbed() {
    let h = handles(Some(Grip::Scale(2)), Some(Grip::Scale(2)));
    assert_eq!(
        marker_styles(&primitives(&h)),
        vec![
            (h.corners[0], Style::Handle),
            (h.corners[1], Style::Handle),
            (h.corners[2], Style::HandleGrabbed),
        ]
    );
    let h = handles(None, Some(Grip::Turn));
    assert_eq!(marker_styles(&primitives(&h))[1].1, Style::HandleGrabbed);
}
