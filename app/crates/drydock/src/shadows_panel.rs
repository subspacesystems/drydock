// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Shadows window's contents: shadows and sun shading on or off, time and date sliders,
//! light and dark strength, shadows on the ground. Kept apart from `App` so tests can drive
//! it with `egui_kittest`; the panel edits a [`ShadowSettings`] copy and says when the app
//! should preview it and when to commit it as an undo step.
//!
//! Behaviour: `docs/specs/m5-app-shadows-ui.md`.

use drydock_kernel::sun::{LocalTime, ShadowSettings, days_in_month};
use eframe::egui;

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
    time.hour * 60 + time.minute
}

/// `time` with the clock set to `minutes` since midnight (clamped to 1439); the date is kept.
pub fn with_minutes(time: &LocalTime, minutes: u32) -> LocalTime {
    let minutes = minutes.min(1439);
    LocalTime::new(time.year, time.month, time.day, minutes / 60, minutes % 60)
        .expect("valid date with a valid clock")
}

/// `time` on day `day` of its year (clamped to 1..=the year's length); the clock is kept.
pub fn with_day_of_year(time: &LocalTime, day: u32) -> LocalTime {
    let year_days = (1..=12)
        .map(|month| days_in_month(time.year, month))
        .sum::<u32>();
    LocalTime::from_day_of_year(time.year, day.clamp(1, year_days), time.hour, time.minute)
        .expect("valid day of the year with a valid clock")
}

/// The clock as "HH:MM", for example "07:05".
pub fn time_label(time: &LocalTime) -> String {
    format!("{:02}:{:02}", time.hour, time.minute)
}

/// A 0..=1 share as a whole percent, for example 0.8 gives "80%".
pub fn percent_label(share: f64) -> String {
    format!("{:.0}%", share * 100.0)
}

/// The date as "YYYY-MM-DD", for example "2026-06-21".
pub fn date_label(time: &LocalTime) -> String {
    format!("{:04}-{:02}-{:02}", time.year, time.month, time.day)
}

/// Draw the panel into `ui`, editing `settings` in place.
pub fn shadows_panel(ui: &mut egui::Ui, settings: &mut ShadowSettings) -> ShadowsEvent {
    fn slider(response: egui::Response, event: &mut ShadowsEvent) {
        if response.drag_stopped() {
            *event = ShadowsEvent::Commit;
        } else if response.changed() {
            *event = if response.dragged() {
                ShadowsEvent::Preview
            } else {
                ShadowsEvent::Commit
            };
        }
    }

    let mut event = ShadowsEvent::None;

    if ui.checkbox(&mut settings.shadows, "Show Shadows").clicked() {
        event = ShadowsEvent::Commit;
    }
    if ui
        .checkbox(&mut settings.sun_shading, "Sun Shading")
        .clicked()
    {
        event = ShadowsEvent::Commit;
    }
    if ui
        .checkbox(&mut settings.on_ground, "Shadows on Ground")
        .clicked()
    {
        event = ShadowsEvent::Commit;
    }

    ui.horizontal(|ui| {
        let mut minutes = minutes_of_day(&settings.time);
        let response = ui.add(
            egui::Slider::new(&mut minutes, 0..=1439)
                .text("Time of Day")
                .show_value(false),
        );
        if response.changed() {
            settings.time = with_minutes(&settings.time, minutes);
        }
        slider(response, &mut event);
        ui.label(time_label(&settings.time));
    });

    ui.horizontal(|ui| {
        let mut day = settings.time.day_of_year();
        let response = ui.add(
            egui::Slider::new(&mut day, 1..=366)
                .text("Date")
                .show_value(false),
        );
        if response.changed() {
            settings.time = with_day_of_year(&settings.time, day);
        }
        slider(response, &mut event);
        ui.label(date_label(&settings.time));
    });

    ui.horizontal(|ui| {
        let response = ui.add(
            egui::Slider::new(&mut settings.light, 0.0..=1.0)
                .text("Light")
                .show_value(false),
        );
        slider(response, &mut event);
        ui.label(percent_label(settings.light));
    });

    ui.horizontal(|ui| {
        let response = ui.add(
            egui::Slider::new(&mut settings.dark, 0.0..=1.0)
                .text("Dark")
                .show_value(false),
        );
        slider(response, &mut event);
        ui.label(percent_label(settings.dark));
    });

    event
}
