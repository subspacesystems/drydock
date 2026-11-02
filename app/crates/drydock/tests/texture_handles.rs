// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Texture handle geometry and overlay primitives (`docs/specs/m4-app-texture-handles.md`).

use drydock::overlay::{Primitive, Style};
use drydock::texture_handles::*;
use drydock_kernel::model::{BodyId, Model, UndoStack};
use drydock_kernel::paint::Side;
use drydock_kernel::texture::TextureMap;
use drydock_kernel::tools::{Target, draw_segments};
use drydock_kernel::topology::FaceId;
use drydock_kernel::units::Unit;
use eframe::egui;
use glam::{DAffine2, DVec2, DVec3, DVec4};

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

fn stem_top(h: &TextureHandles) -> DVec3 {
    (h.corners[2] + h.corners[3]) / 2.0
}

#[test]
fn turn_grip_sits_above_the_top_edge() {
    let h = handles(None, None);
    assert!(close3(turn_grip_point(&h.corners), v(0.5, 1.25, 0.0)));
    // Follows a tilted, scaled copy: top-edge middle plus a quarter of the height vector.
    let corners = [
        v(0.0, 0.0, 1.0),
        v(2.0, 0.0, 1.0),
        v(2.0, 0.0, 5.0),
        v(0.0, 0.0, 5.0),
    ];
    assert!(close3(turn_grip_point(&corners), v(1.0, 0.0, 6.0)));
}

#[test]
fn primitives_outline_the_image_with_four_scale_grips_and_a_turn_grip() {
    let h = handles(None, None);
    let primitives = primitives(&h);
    assert_eq!(primitives.len(), 7, "{primitives:?}");
    assert_eq!(
        primitives[0],
        Primitive::Polygon {
            points: h.corners.to_vec(),
            style: Style::BoundsBox,
        }
    );
    for i in 0..4 {
        assert_eq!(
            primitives[1 + i],
            Primitive::Marker {
                at: h.corners[i],
                style: Style::Handle
            }
        );
    }
    assert_eq!(
        primitives[5],
        Primitive::Line {
            a: stem_top(&h),
            b: turn_grip_point(&h.corners),
            style: Style::BoundsBox,
        }
    );
    assert_eq!(
        primitives[6],
        Primitive::Marker {
            at: turn_grip_point(&h.corners),
            style: Style::Handle
        }
    );
}

#[test]
fn hovered_grip_marker_is_highlighted() {
    let h = handles(Some(Grip::Turn), None);
    let styles = marker_styles(&primitives(&h));
    assert_eq!(styles.len(), 5);
    assert_eq!(styles[4], (turn_grip_point(&h.corners), Style::HandleHover));
    assert!(styles[..4].iter().all(|(_, s)| *s == Style::Handle));

    let h = handles(Some(Grip::Scale(3)), None);
    let styles = marker_styles(&primitives(&h));
    for (i, (at, style)) in styles.iter().take(4).enumerate() {
        assert_eq!(*at, h.corners[i]);
        let want = if i == 3 {
            Style::HandleHover
        } else {
            Style::Handle
        };
        assert_eq!(*style, want, "corner {i}");
    }
    assert_eq!(styles[4].1, Style::Handle);

    let h = handles(Some(Grip::Slide), None);
    assert!(
        marker_styles(&primitives(&h))
            .iter()
            .all(|(_, style)| *style == Style::Handle)
    );
}

#[test]
fn held_grip_marker_is_grabbed() {
    let h = handles(Some(Grip::Scale(1)), Some(Grip::Scale(1)));
    let styles = marker_styles(&primitives(&h));
    for (i, (_, style)) in styles.iter().take(4).enumerate() {
        let want = if i == 1 {
            Style::HandleGrabbed
        } else {
            Style::Handle
        };
        assert_eq!(*style, want, "corner {i}");
    }
    let h = handles(None, Some(Grip::Turn));
    assert_eq!(marker_styles(&primitives(&h))[4].1, Style::HandleGrabbed);
}

#[test]
fn grips_have_distinct_cursors() {
    use egui::CursorIcon::*;
    assert_eq!(Grip::Slide.cursor(), Move);
    assert_eq!(Grip::Turn.cursor(), Grab);
    assert_eq!(Grip::Scale(0).cursor(), ResizeNwSe);
    assert_eq!(Grip::Scale(2).cursor(), ResizeNwSe);
    assert_eq!(Grip::Scale(1).cursor(), ResizeNeSw);
    assert_eq!(Grip::Scale(3).cursor(), ResizeNeSw);
}

#[test]
fn help_text_is_short_and_follows_the_state() {
    let idle = help_text(&handles(None, None));
    assert_eq!(
        idle,
        "Click a corner to scale, the top grip to turn, the image to slide. Esc when done."
    );
    assert_eq!(help_text(&handles(Some(Grip::Turn), None)), idle);
    assert_eq!(
        help_text(&handles(None, Some(Grip::Slide))),
        "Click to drop the image here. Esc cancels."
    );
    assert_eq!(
        help_text(&handles(None, Some(Grip::Turn))),
        "Click to set the angle. Esc cancels."
    );
    let scale = "Click to set the size (Shift keeps proportions). Esc cancels.";
    for i in 0..4 {
        assert_eq!(help_text(&handles(None, Some(Grip::Scale(i)))), scale);
    }
    for text in [idle, scale] {
        assert!(text.len() <= 90, "{text}");
    }
}

/// One pixel per meter, y flipped: pixel (x, 100 - y).
fn project(p: DVec3) -> Option<DVec2> {
    Some(DVec2::new(p.x, 100.0 - p.y))
}

fn px(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, 100.0 - y)
}

fn square_loop() -> Vec<DVec3> {
    vec![
        v(0.0, 0.0, 0.0),
        v(40.0, 0.0, 0.0),
        v(40.0, 40.0, 0.0),
        v(0.0, 40.0, 0.0),
    ]
}

#[test]
fn snap_point_prefers_a_vertex_then_an_edge() {
    let loops = [square_loop()];
    let (at, kind) = snap_point(&project, px(43.0, 4.0), &loops).unwrap();
    assert_eq!(kind, SnapKind::Vertex);
    assert!(close3(at, v(40.0, 0.0, 0.0)), "{at}");

    // 8 px from the right edge, 20 px from the nearest vertex.
    let (at, kind) = snap_point(&project, px(48.0, 20.0), &loops).unwrap();
    assert_eq!(kind, SnapKind::Edge);
    assert!(close3(at, v(40.0, 20.0, 0.0)), "{at}");

    // A vertex within range beats an edge that is nearer.
    let (at, kind) = snap_point(&project, px(41.0, 9.0), &loops).unwrap();
    assert_eq!(kind, SnapKind::Vertex);
    assert!(close3(at, v(40.0, 0.0, 0.0)), "{at}");
}

#[test]
fn snap_point_needs_the_cursor_within_range() {
    let loops = [square_loop()];
    assert_eq!(snap_point(&project, px(52.0, 20.0), &loops), None);
    assert_eq!(snap_point(&project, px(20.0, 20.0), &loops), None);
    assert_eq!(snap_point(&project, px(20.0, 20.0), &[]), None);
    assert_eq!(snap_point(&|_| None, px(40.0, 0.0), &loops), None);
}

#[test]
fn snap_point_sees_holes_and_clamps_to_the_edge() {
    let hole = vec![
        v(10.0, 10.0, 0.0),
        v(10.0, 30.0, 0.0),
        v(30.0, 30.0, 0.0),
        v(30.0, 10.0, 0.0),
    ];
    let loops = [square_loop(), hole];
    let (at, kind) = snap_point(&project, px(20.0, 34.0), &loops).unwrap();
    assert_eq!(kind, SnapKind::Edge);
    assert!(close3(at, v(20.0, 30.0, 0.0)), "{at}");
    // Past the end of an edge but beyond reach of its vertex: no snap.
    assert_eq!(snap_point(&project, px(-12.0, -12.0), &loops), None);
}

#[test]
fn snap_point_works_in_3d() {
    // The edge runs up the z axis; the projection drops z.
    let wall = vec![
        v(0.0, 0.0, 0.0),
        v(0.0, 0.0, 40.0),
        v(30.0, 0.0, 40.0),
        v(30.0, 0.0, 0.0),
    ];
    let side = |p: DVec3| Some(DVec2::new(p.x, 100.0 - p.z));
    let (at, kind) = snap_point(&side, DVec2::new(34.0, 100.0 - 20.0), &[wall]).unwrap();
    assert_eq!(kind, SnapKind::Edge);
    assert!(close3(at, v(30.0, 0.0, 20.0)), "{at}");
}

#[test]
fn snap_corner_picks_the_closest_corner_vertex_pair() {
    let vertices = square_loop();
    let corners = [
        v(3.0, 4.0, 0.0),
        v(13.0, 4.0, 0.0),
        v(13.0, 14.0, 0.0),
        v(3.0, 14.0, 0.0),
    ];
    let (i, vertex) = snap_corner(&project, &corners, &vertices).unwrap();
    assert_eq!(i, 0);
    assert!(close3(vertex, v(0.0, 0.0, 0.0)));

    // Corner 2 is closer to (40, 40) than corner 0 is to (0, 0).
    let corners = [
        v(3.0, 4.0, 0.0),
        v(30.0, 4.0, 0.0),
        v(41.0, 42.0, 0.0),
        v(3.0, 30.0, 0.0),
    ];
    let (i, vertex) = snap_corner(&project, &corners, &vertices).unwrap();
    assert_eq!(i, 2);
    assert!(close3(vertex, v(40.0, 40.0, 0.0)));

    let far = [
        v(20.0, 50.0, 0.0),
        v(30.0, 50.0, 0.0),
        v(30.0, 60.0, 0.0),
        v(20.0, 60.0, 0.0),
    ];
    assert_eq!(snap_corner(&project, &far, &vertices), None);
    assert_eq!(snap_corner(&project, &corners, &[]), None);
}

fn assert_angle(got: f64, want_deg: f64) {
    assert!(
        (got - want_deg.to_radians()).abs() < 1e-9,
        "{} degrees, want {want_deg}",
        got.to_degrees()
    );
}

#[test]
fn snap_turn_lines_the_image_up_with_an_edge() {
    let axis = [DVec2::X, DVec2::Y];
    assert_angle(snap_turn(2.0f64.to_radians(), &axis), 0.0);
    assert_angle(snap_turn(-2.5f64.to_radians(), &axis), 0.0);
    assert_angle(snap_turn(88.0f64.to_radians(), &axis), 90.0);
    assert_angle(snap_turn(182.0f64.to_radians(), &axis), 180.0);
    assert_angle(snap_turn(-91.5f64.to_radians(), &axis), -90.0);
}

#[test]
fn snap_turn_leaves_other_angles_alone() {
    let axis = [DVec2::X, DVec2::Y];
    for deg in [4.0, 10.0, 45.0, 53.13, 86.0, -20.0] {
        assert_angle(snap_turn(f64::to_radians(deg), &axis), deg);
    }
    assert_angle(snap_turn(f64::to_radians(1.0), &[]), 1.0);
    assert_angle(
        snap_turn(
            f64::to_radians(1.0),
            &[DVec2::ZERO, DVec2::new(f64::NAN, 1.0)],
        ),
        1.0,
    );
    assert!(snap_turn(f64::NAN, &axis).is_nan());
}

#[test]
fn snap_turn_follows_slanted_edges() {
    // An edge at 30 degrees in texture space lines up after turning by -30 (or 60, ...).
    let dir = [DVec2::new(30.0f64.to_radians().cos(), 30.0f64.to_radians().sin()) * 3.0];
    assert_angle(snap_turn((-28.0f64).to_radians(), &dir), -30.0);
    assert_angle(snap_turn(62.0f64.to_radians(), &dir), 60.0);
    assert_angle(snap_turn(10.0f64.to_radians(), &dir), 10.0);
}
