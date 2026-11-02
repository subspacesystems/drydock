// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-orbit-pivot: `ViewState` orbits about its pivot while one is set.
//! See docs/specs/m4-orbit-pivot.md.

use drydock::input::Action;
use drydock::viewstate::{Effect, ViewState};
use glam::{DVec2, DVec3};

const VP: (f64, f64) = (800.0, 600.0);

#[test]
fn a_new_view_has_no_pivot() {
    assert_eq!(ViewState::new().pivot, None);
}

#[test]
fn orbit_without_a_pivot_turns_about_the_target() {
    let mut s = ViewState::new();
    let mut want = s.camera;
    want.orbit(20.0, -8.0);
    assert_eq!(
        s.apply(Action::Orbit { dx: 20.0, dy: -8.0 }, VP),
        Effect::None
    );
    assert_eq!(s.camera, want);
}

#[test]
fn orbit_with_a_pivot_turns_about_it() {
    let pivot = DVec3::new(3.0, -2.0, 1.0);
    let mut s = ViewState::new();
    s.pivot = Some(pivot);
    let mut want = s.camera;
    want.orbit_about(pivot, 20.0, -8.0);
    assert_eq!(
        s.apply(Action::Orbit { dx: 20.0, dy: -8.0 }, VP),
        Effect::None
    );
    assert_eq!(s.camera, want);
    // The pivot stays for the rest of the drag.
    assert_eq!(s.pivot, Some(pivot));
    want.orbit_about(pivot, 5.0, 5.0);
    s.apply(Action::Orbit { dx: 5.0, dy: 5.0 }, VP);
    assert_eq!(s.camera, want);
}

#[test]
fn pan_and_zoom_ignore_the_pivot() {
    let mut with = ViewState::new();
    with.pivot = Some(DVec3::new(9.0, 9.0, 9.0));
    let mut without = ViewState::new();
    for action in [
        Action::Pan { dx: 12.0, dy: 4.0 },
        Action::Zoom {
            steps: 2.0,
            cursor_ndc: DVec2::new(0.3, -0.2),
        },
    ] {
        with.apply(action, VP);
        without.apply(action, VP);
        assert_eq!(with.camera, without.camera, "{action:?}");
    }
}
