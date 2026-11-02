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
    use crate::geom::LINEAR_TOL;

    if !a.is_finite() || !b.is_finite() || !toward.is_finite() {
        return None;
    }

    let edge = b - a;
    let length = edge.length();
    if !length.is_finite() || length < LINEAR_TOL {
        return None;
    }
    let dir = edge / length;

    if let Some(distance) = typed
        && (!distance.is_finite() || distance <= LINEAR_TOL)
    {
        return None;
    }

    let offset = toward - a;
    let perpendicular = offset - dir * dir.dot(offset);
    let side_distance = perpendicular.length();
    if !side_distance.is_finite() || side_distance < LINEAR_TOL {
        return None;
    }
    let side = perpendicular / side_distance;

    let distance = typed.unwrap_or(side_distance);
    Some(Guide::Line {
        point: a + side * distance,
        dir,
    })
}

/// A guide point at `p`.
pub fn point_guide(p: DVec3) -> Option<Guide> {
    if p.is_finite() {
        Some(Guide::Point(p))
    } else {
        None
    }
}

impl Guide {
    /// Distance from `p` to the guide.
    pub fn distance_to(&self, p: DVec3) -> f64 {
        match *self {
            Guide::Line { point, dir } => (p - point).cross(dir).length(),
            Guide::Point(point) => p.distance(point),
        }
    }
}

/// The guides of one body, in creation order. Equality compares the guides and their ids,
/// not the id counter.
#[derive(Clone, Debug, Default)]
pub struct Guides {
    items: std::collections::BTreeMap<GuideId, Guide>,
    next: u64,
}

impl Guides {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, guide: Guide) -> GuideId {
        let id = GuideId(self.next);
        self.next += 1;
        self.items.insert(id, guide);
        id
    }
    /// Insert `guide` under a given id (undo, redo, file loading). Returns false and changes
    /// nothing if the id is taken. Later `add`s never return this id.
    pub fn insert(&mut self, id: GuideId, guide: Guide) -> bool {
        if self.items.contains_key(&id) {
            return false;
        }
        self.items.insert(id, guide);
        self.next = self.next.max(id.0.saturating_add(1));
        true
    }
    pub fn remove(&mut self, id: GuideId) -> Option<Guide> {
        self.items.remove(&id)
    }
    pub fn clear(&mut self) -> Vec<(GuideId, Guide)> {
        std::mem::take(&mut self.items).into_iter().collect()
    }
    pub fn get(&self, id: GuideId) -> Option<&Guide> {
        self.items.get(&id)
    }
    pub fn iter(&self) -> impl Iterator<Item = (GuideId, &Guide)> {
        self.items.iter().map(|(id, guide)| (*id, guide))
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl PartialEq for Guides {
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items
    }
}
