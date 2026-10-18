// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m0-input: mouse and keyboard mapping. See docs/specs/m0-input.md.

use glam::DVec2;
use modeler::input::{
    Action, Button, Modifiers, Tool, ZOOM_DRAG_PIXELS_PER_STEP, drag, key, scroll,
};
use modeler_view::StandardView;

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};
const SHIFT: Modifiers = Modifiers {
    shift: true,
    ctrl: false,
    alt: false,
};
const CTRL: Modifiers = Modifiers {
    shift: false,
    ctrl: true,
    alt: false,
};
const ALT: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: true,
};
const CTRL_SHIFT: Modifiers = Modifiers {
    shift: true,
    ctrl: true,
    alt: false,
};

const TOOLS: [Tool; 3] = [Tool::Orbit, Tool::Pan, Tool::Zoom];
const START: DVec2 = DVec2::new(0.25, -0.5);

fn d(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

#[test]
fn middle_drag_orbits_with_any_tool() {
    for tool in TOOLS {
        assert_eq!(
            drag(tool, Button::Middle, NONE, d(12.0, -7.0), START),
            Some(Action::Orbit { dx: 12.0, dy: -7.0 }),
            "{tool:?}"
        );
    }
}

#[test]
fn shift_middle_drag_pans_with_any_tool() {
    for tool in TOOLS {
        assert_eq!(
            drag(tool, Button::Middle, SHIFT, d(-3.0, 4.5), START),
            Some(Action::Pan { dx: -3.0, dy: 4.5 }),
            "{tool:?}"
        );
    }
}

#[test]
fn left_drag_follows_the_tool() {
    assert_eq!(
        drag(Tool::Orbit, Button::Left, NONE, d(5.0, 6.0), START),
        Some(Action::Orbit { dx: 5.0, dy: 6.0 })
    );
    assert_eq!(
        drag(Tool::Orbit, Button::Left, SHIFT, d(5.0, 6.0), START),
        Some(Action::Pan { dx: 5.0, dy: 6.0 })
    );
    assert_eq!(
        drag(Tool::Pan, Button::Left, NONE, d(5.0, 6.0), START),
        Some(Action::Pan { dx: 5.0, dy: 6.0 })
    );
    assert_eq!(
        drag(Tool::Pan, Button::Left, SHIFT, d(5.0, 6.0), START),
        Some(Action::Pan { dx: 5.0, dy: 6.0 })
    );
}

#[test]
fn zoom_tool_drag_up_zooms_in_about_the_press_point() {
    let up = drag(Tool::Zoom, Button::Left, NONE, d(3.0, -50.0), START);
    assert_eq!(
        up,
        Some(Action::Zoom {
            steps: 50.0 / ZOOM_DRAG_PIXELS_PER_STEP,
            cursor_ndc: START
        })
    );
    let down = drag(Tool::Zoom, Button::Left, NONE, d(0.0, 10.0), START);
    assert_eq!(
        down,
        Some(Action::Zoom {
            steps: -10.0 / ZOOM_DRAG_PIXELS_PER_STEP,
            cursor_ndc: START
        })
    );
    let sideways = drag(Tool::Zoom, Button::Left, NONE, d(8.0, 0.0), START);
    assert_eq!(
        sideways,
        Some(Action::Zoom {
            steps: 0.0,
            cursor_ndc: START
        })
    );
}

#[test]
fn right_drag_does_nothing() {
    for tool in TOOLS {
        for mods in [NONE, SHIFT, CTRL, ALT] {
            assert_eq!(drag(tool, Button::Right, mods, d(10.0, 10.0), START), None);
        }
    }
}

#[test]
fn zero_delta_does_nothing() {
    for tool in TOOLS {
        for button in [Button::Left, Button::Middle, Button::Right] {
            for mods in [NONE, SHIFT] {
                assert_eq!(drag(tool, button, mods, DVec2::ZERO, START), None);
            }
        }
    }
}

#[test]
fn ctrl_and_alt_do_not_change_drags() {
    for tool in TOOLS {
        for button in [Button::Left, Button::Middle] {
            for (plain, extra) in [
                (NONE, CTRL),
                (NONE, ALT),
                (SHIFT, CTRL_SHIFT),
                (
                    SHIFT,
                    Modifiers {
                        shift: true,
                        ctrl: false,
                        alt: true,
                    },
                ),
            ] {
                assert_eq!(
                    drag(tool, button, extra, d(2.0, -9.0), START),
                    drag(tool, button, plain, d(2.0, -9.0), START),
                    "{tool:?} {button:?} {extra:?}"
                );
            }
        }
    }
}

#[test]
fn scroll_zooms_toward_the_cursor() {
    let cursor = d(-0.75, 0.1);
    assert_eq!(
        scroll(2.0, cursor),
        Action::Zoom {
            steps: 2.0,
            cursor_ndc: cursor
        }
    );
    assert_eq!(
        scroll(-0.5, cursor),
        Action::Zoom {
            steps: -0.5,
            cursor_ndc: cursor
        }
    );
}

#[test]
fn key_table() {
    let table = [
        ("O", NONE, Action::SelectTool(Tool::Orbit)),
        ("H", NONE, Action::SelectTool(Tool::Pan)),
        ("Z", NONE, Action::SelectTool(Tool::Zoom)),
        ("Z", SHIFT, Action::ZoomExtents),
        ("P", NONE, Action::ToggleProjection),
        ("O", CTRL, Action::Open),
        ("F2", NONE, Action::View(StandardView::Iso)),
        ("F3", NONE, Action::View(StandardView::Top)),
        ("F4", NONE, Action::View(StandardView::Front)),
        ("F5", NONE, Action::View(StandardView::Right)),
        ("F6", NONE, Action::View(StandardView::Back)),
        ("F7", NONE, Action::View(StandardView::Left)),
        ("F8", NONE, Action::View(StandardView::Bottom)),
    ];
    for (name, mods, action) in table {
        assert_eq!(key(name, mods), Some(action), "{name} {mods:?}");
    }
}

#[test]
fn unlisted_keys_and_modifier_sets_do_nothing() {
    for (name, mods) in [
        ("o", NONE),
        ("z", SHIFT),
        ("O", SHIFT),
        ("O", ALT),
        ("O", CTRL_SHIFT),
        ("H", CTRL),
        ("Z", CTRL),
        ("Z", CTRL_SHIFT),
        ("P", SHIFT),
        ("F2", SHIFT),
        ("F3", CTRL),
        ("F1", NONE),
        ("F9", NONE),
        ("A", NONE),
        ("Escape", NONE),
        ("", NONE),
    ] {
        assert_eq!(key(name, mods), None, "{name:?} {mods:?}");
    }
}
