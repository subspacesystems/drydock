// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Tape measure and guides: the kernel side of the Tape tool.
//!
//! Behaviour: `docs/specs/m3-tape.md`.

use glam::DVec3;

/// A construction aid. Guides are not geometry: they never touch faces or edges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Guide {
    /// An infinite line through `point`; `dir` has length 1.
    Line { point: DVec3, dir: DVec3 },
    /// A guide point.
    Point(DVec3),
}

/// Identifies a guide inside a `Guides`. Ids are never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuideId(pub u64);

/// Straight-line distance between two points, in meters.
pub fn measure(a: DVec3, b: DVec3) -> f64 {
    a.distance(b)
}

/// A guide line parallel to the edge `a`-`b`.
pub fn parallel_guide(a: DVec3, b: DVec3, toward: DVec3, typed: Option<f64>) -> Option<Guide> {
    let _ = (a, b, toward, typed);
    unimplemented!("m3-tape")
}

/// A guide point at `p`.
pub fn point_guide(p: DVec3) -> Option<Guide> {
    let _ = p;
    unimplemented!("m3-tape")
}

impl Guide {
    /// Distance from `p` to the guide.
    pub fn distance_to(&self, p: DVec3) -> f64 {
        let _ = p;
        unimplemented!("m3-tape")
    }
}

/// The guides of one body, in creation order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Guides {
    items: std::collections::BTreeMap<GuideId, Guide>,
    next: u64,
}

impl Guides {
    pub fn new() -> Self {
        unimplemented!("m3-tape")
    }
    pub fn add(&mut self, guide: Guide) -> GuideId {
        let _ = guide;
        unimplemented!("m3-tape")
    }
    pub fn remove(&mut self, id: GuideId) -> Option<Guide> {
        let _ = id;
        unimplemented!("m3-tape")
    }
    pub fn clear(&mut self) -> Vec<(GuideId, Guide)> {
        unimplemented!("m3-tape")
    }
    pub fn get(&self, id: GuideId) -> Option<&Guide> {
        let _ = id;
        unimplemented!("m3-tape")
    }
    pub fn iter(&self) -> impl Iterator<Item = (GuideId, &Guide)> {
        let _ = (&self.items, self.next);
        std::iter::empty()
    }
    pub fn len(&self) -> usize {
        unimplemented!("m3-tape")
    }
    pub fn is_empty(&self) -> bool {
        unimplemented!("m3-tape")
    }
}
