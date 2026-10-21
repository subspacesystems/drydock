// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Inference hint text and colors (`docs/specs/m2-hints.md`).

use glam::DVec3;
use modeler::hints::{axis_color, hint_label, snap_color, snap_name};
use modeler_kernel::infer::{Inference, Snap};
use modeler_kernel::topology::{EdgeId, FaceId, VertexId};

fn inf(snap: Snap, axis: Option<usize>) -> Inference {
    Inference {
        point: DVec3::ZERO,
        snap,
        axis,
    }
}

#[test]
fn axis_colors() {
    assert_eq!(axis_color(0), [220, 50, 47]);
    assert_eq!(axis_color(1), [40, 160, 60]);
    assert_eq!(axis_color(2), [50, 90, 220]);
    assert_eq!(axis_color(3), [160, 160, 160]);
    assert_eq!(axis_color(usize::MAX), [160, 160, 160]);
}

#[test]
fn snap_colors() {
    assert_eq!(snap_color(Snap::Endpoint(VertexId(1))), [40, 200, 80]);
    assert_eq!(snap_color(Snap::Midpoint(EdgeId(1))), [60, 200, 220]);
    assert_eq!(snap_color(Snap::Center(0)), [60, 200, 220]);
    assert_eq!(snap_color(Snap::OnEdge(EdgeId(1))), [220, 70, 60]);
    assert_eq!(snap_color(Snap::OnFace(FaceId(1))), [70, 130, 230]);
    assert_eq!(snap_color(Snap::OnAxis(0)), [160, 160, 160]);
    assert_eq!(snap_color(Snap::Ground), [160, 160, 160]);
    assert_eq!(snap_color(Snap::Along), [160, 160, 160]);
}

#[test]
fn snap_names() {
    assert_eq!(snap_name(Snap::Endpoint(VertexId(7))), "Endpoint");
    assert_eq!(snap_name(Snap::Midpoint(EdgeId(7))), "Midpoint");
    assert_eq!(snap_name(Snap::Center(2)), "Center");
    assert_eq!(snap_name(Snap::OnEdge(EdgeId(7))), "On Edge");
    assert_eq!(snap_name(Snap::OnFace(FaceId(7))), "On Face");
    assert_eq!(snap_name(Snap::Ground), "On Ground");
    assert_eq!(snap_name(Snap::Along), "Locked Direction");
    assert_eq!(snap_name(Snap::OnAxis(0)), "On Red Axis");
    assert_eq!(snap_name(Snap::OnAxis(1)), "On Green Axis");
    assert_eq!(snap_name(Snap::OnAxis(2)), "On Blue Axis");
    assert_eq!(snap_name(Snap::OnAxis(5)), "On ? Axis");
}

#[test]
fn label_without_axis_uses_the_snap() {
    let l = hint_label(&inf(Snap::Endpoint(VertexId(3)), None));
    assert_eq!(l.text, "Endpoint");
    assert_eq!(l.color, [40, 200, 80]);
    let l = hint_label(&inf(Snap::OnFace(FaceId(3)), None));
    assert_eq!(l.text, "On Face");
    assert_eq!(l.color, [70, 130, 230]);
}

#[test]
fn label_on_axis_snap_uses_the_axis_color() {
    let l = hint_label(&inf(Snap::OnAxis(1), Some(1)));
    assert_eq!(l.text, "On Green Axis");
    assert_eq!(l.color, [40, 160, 60]);
}

#[test]
fn label_other_snap_on_an_axis_names_both() {
    let l = hint_label(&inf(Snap::Endpoint(VertexId(3)), Some(0)));
    assert_eq!(l.text, "Endpoint on Red Axis");
    assert_eq!(l.color, [220, 50, 47]);
    let l = hint_label(&inf(Snap::Ground, Some(2)));
    assert_eq!(l.text, "On Ground on Blue Axis");
    assert_eq!(l.color, [50, 90, 220]);
    let l = hint_label(&inf(Snap::Along, Some(1)));
    assert_eq!(l.text, "Locked Direction on Green Axis");
}

#[test]
fn label_mismatched_axis_snap_names_both() {
    // Snapped to the red axis line while the point also runs along green.
    let l = hint_label(&inf(Snap::OnAxis(0), Some(1)));
    assert_eq!(l.text, "On Red Axis on Green Axis");
    assert_eq!(l.color, [40, 160, 60]);
}
