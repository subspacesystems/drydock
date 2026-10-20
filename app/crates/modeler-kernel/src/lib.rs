// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Geometry kernel. World space is meters, Z up, right-handed.
//!
//! Behaviour: `docs/specs/m1-geom.md`, `docs/specs/m1-topology.md`,
//! `docs/specs/m1-sticky-edges.md`, `docs/specs/m1-model.md`,
//! `docs/specs/m1-format.md` (file format: `docs/format.md`), `docs/specs/m1-kernel-scene.md`
//! (triangulation), `docs/specs/m2-units.md` (typed lengths).

mod faces;
pub mod format;
pub mod geom;
pub mod model;
pub mod sticky;
pub mod tess;
pub mod topology;
pub mod units;
