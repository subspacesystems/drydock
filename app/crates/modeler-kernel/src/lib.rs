// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Geometry kernel. World space is meters, Z up, right-handed.
//!
//! Behaviour: `docs/specs/m1-geom.md`, `docs/specs/m1-topology.md`,
//! `docs/specs/m1-sticky-edges.md`.

pub mod geom;
pub mod sticky;
pub mod topology;
