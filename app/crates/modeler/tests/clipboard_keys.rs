// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Ctrl+X/C/V reach Cut, Copy and Paste (`docs/specs/m4-fix-clipboard-keys.md`).

use eframe::egui::{Event, Key, Modifiers};
use modeler::chrome::clipboard_actions;
use modeler::input::Action;

fn key(key: Key, modifiers: Modifiers, pressed: bool, repeat: bool) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat,
        modifiers,
    }
}

#[test]
fn egui_clipboard_events_become_actions_in_order() {
    let events = [
        Event::Copy,
        Event::PointerGone,
        Event::Paste("anything".into()),
        Event::Cut,
        Event::Paste(String::new()),
    ];
    assert_eq!(
        clipboard_actions(&events),
        vec![Action::Copy, Action::Paste, Action::Cut, Action::Paste]
    );
}

#[test]
fn ctrl_key_presses_still_count() {
    let events = [
        key(Key::C, Modifiers::CTRL, true, false),
        key(Key::X, Modifiers::COMMAND, true, false),
        key(Key::V, Modifiers::CTRL, true, false),
    ];
    assert_eq!(
        clipboard_actions(&events),
        vec![Action::Copy, Action::Cut, Action::Paste]
    );
}

#[test]
fn other_keys_releases_repeats_and_other_modifiers_are_ignored() {
    let events = [
        key(Key::C, Modifiers::NONE, true, false),
        key(Key::C, Modifiers::CTRL, false, false),
        key(Key::C, Modifiers::CTRL, true, true),
        key(Key::C, Modifiers::CTRL | Modifiers::SHIFT, true, false),
        key(Key::V, Modifiers::CTRL | Modifiers::ALT, true, false),
        key(Key::Z, Modifiers::CTRL, true, false),
        Event::Text("c".into()),
    ];
    assert!(clipboard_actions(&events).is_empty());
}

#[test]
fn nothing_happens_without_events() {
    assert!(clipboard_actions(&[]).is_empty());
}
