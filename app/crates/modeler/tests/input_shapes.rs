// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Key map for the shape tools (`docs/specs/m2-app-shapes.md`).

use glam::DVec2;
use modeler::input::{Action, Button, Modifiers, Tool, drag, key};

fn mods(m: u8) -> Modifiers {
    Modifiers {
        shift: m & 1 != 0,
        ctrl: m & 2 != 0,
        alt: m & 4 != 0,
    }
}

#[test]
fn letters_pick_the_shape_tools() {
    assert_eq!(key("R", mods(0)), Some(Action::SelectTool(Tool::Rectangle)));
    assert_eq!(key("C", mods(0)), Some(Action::SelectTool(Tool::Circle)));
    assert_eq!(key("A", mods(0)), Some(Action::SelectTool(Tool::Arc)));
}

#[test]
fn shape_letters_with_modifiers_do_nothing() {
    for k in ["R", "C", "A"] {
        for m in 1..8 {
            if k == "A" && m == 1 {
                continue; // Shift+A is the three-point arc (`docs/specs/m2-arc-3pt-app.md`).
            }
            assert_eq!(key(k, mods(m)), None, "{k} {:?}", mods(m));
        }
    }
}

#[test]
fn other_keys_keep_their_meaning() {
    assert_eq!(key("L", mods(0)), Some(Action::SelectTool(Tool::Line)));
    assert_eq!(
        key("Space", mods(0)),
        Some(Action::SelectTool(Tool::Select))
    );
    assert_eq!(key("O", mods(0)), Some(Action::SelectTool(Tool::Orbit)));
}

#[test]
fn shape_tools_leave_left_drags_to_the_editor() {
    for tool in [Tool::Rectangle, Tool::Circle, Tool::Arc] {
        for m in 0..8 {
            let d = DVec2::new(5.0, 6.0);
            assert_eq!(drag(tool, Button::Left, mods(m), d, DVec2::ZERO), None);
        }
        assert!(matches!(
            drag(
                tool,
                Button::Middle,
                mods(0),
                DVec2::new(5.0, 6.0),
                DVec2::ZERO
            ),
            Some(Action::Orbit { .. })
        ));
    }
}
