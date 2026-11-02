// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Property tests for the tape measure (`docs/specs/m3-tape.md`).

use drydock_kernel::tape::{measure, parallel_guide};
use glam::DVec3;
use proptest::prelude::*;

fn pt() -> impl Strategy<Value = DVec3> {
    (-50.0..50.0f64, -50.0..50.0f64, -50.0..50.0f64).prop_map(|(x, y, z)| DVec3::new(x, y, z))
}

proptest! {
    #[test]
    fn triangle_inequality(a in pt(), b in pt(), c in pt()) {
        prop_assert!(measure(a, c) <= measure(a, b) + measure(b, c) + 1e-9);
    }

    #[test]
    fn guide_is_parallel_and_at_the_cursor_distance(a in pt(), b in pt(), c in pt()) {
        if let Some(g) = parallel_guide(a, b, c, None) {
            let (p, d) = match g {
                drydock_kernel::tape::Guide::Line { point, dir } => (point, dir),
                _ => unreachable!(),
            };
            prop_assert!((d.length() - 1.0).abs() < 1e-9);
            prop_assert!(d.dot((b - a).normalize()) > 1.0 - 1e-9);
            // the guide passes through the cursor's foot and is as far from the edge line as the cursor
            let edge = drydock_kernel::tape::Guide::Line { point: a, dir: d };
            prop_assert!((edge.distance_to(p) - g.distance_to(a)).abs() < 1e-6);
            prop_assert!((edge.distance_to(c) - g.distance_to(a)).abs() < 1e-6);
            // the cursor lies in the plane of the edge and the guide
            prop_assert!(g.distance_to(c) <= edge.distance_to(c) + 1e-6);
        }
    }

    #[test]
    fn typed_distance_is_the_offset(a in pt(), b in pt(), c in pt(), t in 0.01..30.0f64) {
        if let Some(g) = parallel_guide(a, b, c, Some(t)) {
            prop_assert!((g.distance_to(a) - t).abs() < 1e-6);
            prop_assert!((g.distance_to(b) - t).abs() < 1e-6);
        }
    }
}
