// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Geometry kernel. World space is meters, Z up, right-handed.
//!
//! Behaviour: `docs/specs/m1-geom.md`, `docs/specs/m1-topology.md`,
//! `docs/specs/m1-sticky-edges.md`, `docs/specs/m1-model.md`,
//! `docs/specs/m1-format.md` (file format: `docs/format.md`), `docs/specs/m1-kernel-scene.md`
//! (triangulation), `docs/specs/m2-units.md` (typed lengths), `docs/specs/m2-shapes.md` (shape points),
//! `docs/specs/m2-select-box.md` (box and connected selection), `docs/specs/m2-select-click.md`
//! (click modifiers), `docs/specs/m2-inference.md` (snaps and picking),
//! `docs/specs/m2-tool-line.md` (drawing tools), `docs/specs/m3-transform.md` (move, rotate, scale, flip),
//! `docs/specs/m3-tape.md` (tape measure and guides), `docs/specs/m3-pushpull.md` (push/pull),
//! `docs/specs/m3-move-merge.md` (sticky move), `docs/specs/m3-erase.md` (erase),
//! `docs/specs/m4-paint.md` (paint and materials), `docs/specs/m4-make-component.md`
//! (make component and group), `docs/specs/m4-explode.md` (explode), `docs/specs/m4-edit-path.md`
//! (editing inside an instance), `docs/specs/m4-texture-model.md` (texture size and placement),
//! `docs/specs/m4-copy.md` (copy, clipboard and paste), `docs/specs/m5-solar.md` (sun position),
//! `docs/specs/m5-geo-model.md` (geolocation and shadow settings), `docs/specs/m7-solid-check.md`
//! (solid check), `docs/specs/m7-solid-repair.md` (solid repair).

pub mod click;
pub mod component;
pub mod copy;
pub mod editpath;
pub mod erase;
pub mod explode;
mod faces;
pub mod format;
pub mod geom;
pub mod infer;
pub mod model;
pub mod movemerge;
pub mod paint;
pub mod pushpull;
pub mod repair;
pub mod select;
pub mod shapes;
pub mod solid;
pub mod sticky;
pub mod sun;
pub mod tape;
pub mod tess;
pub mod texture;
pub mod tools;
pub mod topology;
pub mod transform;
pub mod units;
