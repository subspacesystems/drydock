// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use glam::{DVec2, DVec3};
use modeler::input::{Action, Tool};
use modeler::viewstate::{Effect, ViewState, scene_bounds};
use modeler_dae::{Aabb, FlatScene};
use modeler_view::{Bounds, Camera, Projection, StandardView};

const VP: (f64, f64) = (800.0, 600.0);

fn bounds() -> Bounds {
    Bounds {
        min: DVec3::new(1.0, 2.0, 3.0),
        max: DVec3::new(5.0, 4.0, 9.0),
    }
}

#[test]
fn new_uses_default_camera_and_orbit_tool() {
    let s = ViewState::new();
    assert_eq!(s.camera, Camera::default());
    assert_eq!(s.tool, Tool::Orbit);
    assert!(s.bounds.is_none());
}

#[test]
fn scene_bounds_converts_aabb() {
    let mut scene = FlatScene::default();
    assert!(scene_bounds(&scene).is_none());
    scene.bounds = Some(Aabb {
        min: [1.0, 2.0, 3.0],
        max: [5.0, 4.0, 9.0],
    });
    assert_eq!(scene_bounds(&scene), Some(bounds()));
}

#[test]
fn set_bounds_frames_the_model_and_keeps_orientation() {
    let mut s = ViewState::new();
    s.apply(Action::View(StandardView::Front), VP);
    let (yaw, pitch) = (s.camera.yaw, s.camera.pitch);
    s.set_bounds(Some(bounds()), VP);
    let mut want = Camera::default();
    want.set_standard_view(StandardView::Front);
    want.zoom_extents(&bounds(), VP.0 / VP.1);
    assert_eq!(s.camera, want);
    assert_eq!((s.camera.yaw, s.camera.pitch), (yaw, pitch));
    assert_eq!(s.bounds, Some(bounds()));
}

#[test]
fn set_bounds_none_leaves_camera() {
    let mut s = ViewState::new();
    s.apply(Action::Orbit { dx: 10.0, dy: 5.0 }, VP);
    let before = s.camera;
    s.set_bounds(None, VP);
    assert_eq!(s.camera, before);
}

#[test]
fn orbit_pan_zoom_call_the_camera() {
    let mut s = ViewState::new();
    let mut c = Camera::default();
    assert_eq!(s.apply(Action::Orbit { dx: 12.0, dy: -7.0 }, VP), Effect::None);
    c.orbit(12.0, -7.0);
    assert_eq!(s.camera, c);
    assert_eq!(s.apply(Action::Pan { dx: 30.0, dy: 4.0 }, VP), Effect::None);
    c.pan(30.0, 4.0, VP.1);
    assert_eq!(s.camera, c);
    let ndc = DVec2::new(0.3, -0.2);
    s.apply(Action::Zoom { steps: 2.0, cursor_ndc: ndc }, VP);
    c.zoom(2.0, ndc, VP.0 / VP.1);
    assert_eq!(s.camera, c);
}

#[test]
fn views_projection_and_tool() {
    let mut s = ViewState::new();
    s.apply(Action::View(StandardView::Top), VP);
    assert!((s.camera.pitch - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    s.apply(Action::ToggleProjection, VP);
    assert_eq!(s.camera.projection, Projection::Parallel);
    s.apply(Action::ToggleProjection, VP);
    assert_eq!(s.camera.projection, Projection::Perspective);
    s.apply(Action::SelectTool(Tool::Zoom), VP);
    assert_eq!(s.tool, Tool::Zoom);
}

#[test]
fn zoom_extents_uses_model_bounds() {
    let mut s = ViewState::new();
    s.set_bounds(Some(bounds()), VP);
    s.apply(Action::Pan { dx: 200.0, dy: 100.0 }, VP);
    s.apply(Action::Zoom { steps: 5.0, cursor_ndc: DVec2::ZERO }, VP);
    s.apply(Action::ZoomExtents, VP);
    let mut want = s.camera;
    want.zoom_extents(&bounds(), VP.0 / VP.1);
    assert_eq!(s.camera, want);
    assert!((s.camera.target - DVec3::new(3.0, 3.0, 6.0)).length() < 1e-9);
}

#[test]
fn zoom_extents_without_model_is_a_no_op() {
    let mut s = ViewState::new();
    s.apply(Action::Orbit { dx: 3.0, dy: 3.0 }, VP);
    let before = s.camera;
    assert_eq!(s.apply(Action::ZoomExtents, VP), Effect::None);
    assert_eq!(s.camera, before);
}

#[test]
fn open_asks_for_the_dialog_and_changes_nothing() {
    let mut s = ViewState::new();
    let before = s.camera;
    assert_eq!(s.apply(Action::Open, VP), Effect::OpenDialog);
    assert_eq!(s.camera, before);
}

#[test]
fn degenerate_viewport_does_not_panic_or_produce_nan() {
    let mut s = ViewState::new();
    s.set_bounds(Some(bounds()), (0.0, 0.0));
    s.apply(Action::Pan { dx: 5.0, dy: 5.0 }, (0.0, 0.0));
    s.apply(Action::Zoom { steps: 1.0, cursor_ndc: DVec2::ZERO }, (0.0, 0.0));
    s.apply(Action::ZoomExtents, (0.0, 0.0));
    let c = s.camera;
    assert!(c.target.is_finite() && c.distance.is_finite() && c.distance > 0.0);
}
