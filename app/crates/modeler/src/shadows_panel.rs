// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Shadows window's contents: shadows and sun shading on or off, time and date sliders,
//! light and dark strength, shadows on the ground. Kept apart from `App` so tests can drive
//! it with `egui_kittest`; the panel edits a [`ShadowSettings`] copy and says when the app
//! should preview it and when to commit it as an undo step.
//!
//! Behaviour: `docs/specs/m5-app-shadows-ui.md`.

use eframe::egui;
use modeler_kernel::sun::{LocalTime, ShadowSettings};

/// What the user did to the settings this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadowsEvent {
    /// Nothing changed.
    None,
    /// A slider is being dragged: show the settings but don't record a step yet.
    Preview,
    /// The change is final (a click, a key press, or the end of a drag): record one step.
    Commit,
}

/// Minutes since midnight of `time` (0..=1439).
pub fn minutes_of_day(time: &LocalTime) -> u32 {
    let _ = time;
    unimplemented!("m5-app-shadows-ui")
}

/// `time` with the clock set to `minutes` since midnight (clamped to 1439); the date is kept.
pub fn with_minutes(time: &LocalTime, minutes: u32) -> LocalTime {
    let _ = (time, minutes);
    unimplemented!("m5-app-shadows-ui")
}

/// `time` on day `day` of its year (clamped to 1..=the year's length); the clock is kept.
pub fn with_day_of_year(time: &LocalTime, day: u32) -> LocalTime {
    let _ = (time, day);
    unimplemented!("m5-app-shadows-ui")
}

/// The clock as "HH:MM", for example "07:05".
pub fn time_label(time: &LocalTime) -> String {
    let _ = time;
    unimplemented!("m5-app-shadows-ui")
}

/// The date as "YYYY-MM-DD", for example "2026-06-21".
pub fn date_label(time: &LocalTime) -> String {
    let _ = time;
    unimplemented!("m5-app-shadows-ui")
}

/// Draw the panel into `ui`, editing `settings` in place.
pub fn shadows_panel(ui: &mut egui::Ui, settings: &mut ShadowSettings) -> ShadowsEvent {
    let _ = (ui, settings);
    unimplemented!("m5-app-shadows-ui")
}
