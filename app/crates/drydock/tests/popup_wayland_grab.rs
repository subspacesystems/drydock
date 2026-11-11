// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Wayland pop-ups (M12): which serial an `xdg_popup` grabs with, and when window
//! creation stops waiting for the pop-up's first configure.
//!
//! GNOME refuses a pop-up grab made with a button-release serial and dismisses the pop-up
//! before configuring it. There is no compositor in CI, so these cover the two pure decisions
//! the local winit patch makes.
#![cfg(target_os = "linux")]

use std::time::Duration;

use winit::platform::wayland::{
    POPUP_CONFIGURE_TIMEOUT, PopupConfigureWait, PopupGrabInput, PopupGrabSerial,
    popup_configure_wait,
};

use PopupGrabInput::{ButtonPress, ButtonRelease, KeyPress, KeyRelease};

fn serial_after(events: &[PopupGrabInput]) -> Option<u32> {
    let mut tracker = PopupGrabSerial::default();
    for event in events {
        tracker.record(*event);
    }
    tracker.serial()
}

#[test]
fn no_events_no_serial() {
    assert_eq!(serial_after(&[]), None);
}

#[test]
fn a_click_grabs_with_the_press_not_the_release() {
    // The menu opens on the release (serial 3); the press was serial 2.
    assert_eq!(serial_after(&[ButtonPress(2), ButtonRelease(3)]), Some(2));
}

#[test]
fn only_releases_give_no_serial() {
    assert_eq!(serial_after(&[ButtonRelease(5)]), None);
    assert_eq!(serial_after(&[KeyRelease(6), ButtonRelease(7)]), None);
}

#[test]
fn the_latest_press_wins() {
    assert_eq!(
        serial_after(&[
            ButtonPress(2),
            ButtonRelease(3),
            ButtonPress(8),
            ButtonRelease(9)
        ]),
        Some(8)
    );
}

#[test]
fn a_key_press_after_a_click_wins() {
    assert_eq!(
        serial_after(&[ButtonPress(2), ButtonRelease(3), KeyPress(4)]),
        Some(4)
    );
}

#[test]
fn a_click_after_a_key_press_wins() {
    assert_eq!(
        serial_after(&[KeyPress(4), KeyRelease(5), ButtonPress(6)]),
        Some(6)
    );
}

#[test]
fn a_key_release_does_not_replace_a_press() {
    assert_eq!(serial_after(&[ButtonPress(2), KeyRelease(3)]), Some(2));
    assert_eq!(
        serial_after(&[KeyPress(2), KeyRelease(3), ButtonRelease(4)]),
        Some(2)
    );
}

#[test]
fn a_zero_serial_is_not_a_grab_serial() {
    assert_eq!(serial_after(&[ButtonPress(0)]), None);
    assert_eq!(serial_after(&[ButtonPress(2), KeyPress(0)]), Some(2));
}

#[test]
fn the_default_tracker_has_no_serial() {
    assert_eq!(PopupGrabSerial::default().serial(), None);
}

const TIMEOUT: Duration = Duration::from_millis(1000);

#[test]
fn the_timeout_is_about_one_second() {
    assert!(POPUP_CONFIGURE_TIMEOUT >= Duration::from_millis(500));
    assert!(POPUP_CONFIGURE_TIMEOUT <= Duration::from_secs(2));
}

#[test]
fn waiting_continues_until_something_happens() {
    assert_eq!(
        popup_configure_wait(false, false, Duration::ZERO, TIMEOUT),
        PopupConfigureWait::Wait
    );
    assert_eq!(
        popup_configure_wait(false, false, Duration::from_millis(999), TIMEOUT),
        PopupConfigureWait::Wait
    );
}

#[test]
fn a_configure_ends_the_wait() {
    assert_eq!(
        popup_configure_wait(true, false, Duration::from_millis(10), TIMEOUT),
        PopupConfigureWait::Configured
    );
}

#[test]
fn a_dismissal_ends_the_wait_without_a_configure() {
    assert_eq!(
        popup_configure_wait(false, true, Duration::from_millis(10), TIMEOUT),
        PopupConfigureWait::Dismissed
    );
}

#[test]
fn a_dismissed_pop_up_is_never_treated_as_configured() {
    assert_eq!(
        popup_configure_wait(true, true, Duration::from_millis(10), TIMEOUT),
        PopupConfigureWait::Dismissed
    );
}

#[test]
fn the_timeout_ends_the_wait() {
    assert_eq!(
        popup_configure_wait(false, false, TIMEOUT, TIMEOUT),
        PopupConfigureWait::TimedOut
    );
    assert_eq!(
        popup_configure_wait(false, false, Duration::from_secs(30), TIMEOUT),
        PopupConfigureWait::TimedOut
    );
}

#[test]
fn a_configure_beats_the_timeout() {
    assert_eq!(
        popup_configure_wait(true, false, Duration::from_secs(30), TIMEOUT),
        PopupConfigureWait::Configured
    );
}
