// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Wayland startup hang (M12 sign-off 4): the pop-up's dismissed flag is shared between the
//! window state and the `Window` handle without a lock, so `request_redraw` can be called while
//! a state guard is held (it was not, and Drydock never showed its window on Wayland).
#![cfg(target_os = "linux")]

use std::sync::Mutex;

use winit::platform::wayland::PopupDismissed;

#[test]
fn a_new_flag_is_not_set() {
    assert!(!PopupDismissed::new().is_set());
    assert!(!PopupDismissed::default().is_set());
}

#[test]
fn set_and_clear() {
    let flag = PopupDismissed::new();
    flag.set(true);
    assert!(flag.is_set());
    flag.set(false);
    assert!(!flag.is_set());
}

#[test]
fn a_clone_sees_the_state_side_set() {
    let state_side = PopupDismissed::new();
    let window_side = state_side.clone();
    state_side.set(true);
    assert!(window_side.is_set());
    window_side.set(false);
    assert!(!state_side.is_set());
}

#[test]
fn reading_the_flag_never_needs_the_state_lock() {
    // The state is locked (as in request_inner_size); the window side still reads the flag.
    let flag = PopupDismissed::new();
    let state = Mutex::new(flag.clone());
    let guard = state.lock().unwrap();
    guard.set(true);
    assert!(flag.is_set());
    drop(guard);
}

#[test]
fn the_flag_is_shared_across_threads() {
    let flag = PopupDismissed::new();
    let other = flag.clone();
    std::thread::spawn(move || other.set(true)).join().unwrap();
    assert!(flag.is_set());
}
