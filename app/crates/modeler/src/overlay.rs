// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Overlay primitives for the M3 tools: what the window paints over the model besides the
//! tools' own previews. Pure world-space data; the window projects and strokes it.
//!
//! Behaviour: `docs/specs/m3-ui-overlay.md`.

use glam::DVec3;
use modeler_kernel::tape::Guides;
use modeler_kernel::tools::plane::DrawPlane;
use modeler_kernel::tools::scale::{Bounds, Handle};
use modeler_kernel::topology::{Context, FaceId};

/// How the window should draw a primitive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// The face Push/Pull would act on.
    FaceHighlight,
    /// The bounding box of the Scale tool.
    BoundsBox,
    /// A scale handle.
    Handle,
    /// The scale handle under the cursor.
    HandleHover,
    /// The scale handle being dragged.
    HandleGrabbed,
    /// The protractor's circle.
    ProtractorRing,
    /// A 15 degree mark on the protractor.
    ProtractorTick,
    /// A protractor arm.
    ProtractorArm,
    /// A guide line.
    Guide,
    /// A guide point.
    GuidePoint,
}

/// One thing to draw, in world coordinates.
#[derive(Clone, Debug, PartialEq)]
pub enum Primitive {
    Line {
        a: DVec3,
        b: DVec3,
        style: Style,
    },
    /// A closed outline (the last point joins the first).
    Polygon {
        points: Vec<DVec3>,
        style: Style,
    },
    /// A small square or cross of fixed pixel size, centred on `at`.
    Marker {
        at: DVec3,
        style: Style,
    },
}

/// Outlines of `face`: the outer loop first, then each hole, all `Style::FaceHighlight`.
pub fn face_highlight(ctx: &Context, face: FaceId) -> Vec<Primitive> {
    let _ = (ctx, face);
    unimplemented!("m3-ui-overlay")
}

/// The Scale tool's box and handles.
pub fn scale_handles(
    bounds: &Bounds,
    hover: Option<Handle>,
    grabbed: Option<Handle>,
) -> Vec<Primitive> {
    let _ = (bounds, hover, grabbed);
    unimplemented!("m3-ui-overlay")
}

/// The Rotate tool's protractor.
pub fn protractor(
    center: DVec3,
    plane: DrawPlane,
    radius: f64,
    start: Option<DVec3>,
    angle: Option<f64>,
) -> Vec<Primitive> {
    let _ = (center, plane, radius, start, angle);
    unimplemented!("m3-ui-overlay")
}

/// The model's guides, lines clipped to a stretch around `around`.
pub fn guides(guides: &Guides, around: DVec3, half_length: f64) -> Vec<Primitive> {
    let _ = (guides, around, half_length);
    unimplemented!("m3-ui-overlay")
}
