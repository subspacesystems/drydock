// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m5-app-shadows-ui: the Shadows panel and its View menu item, driven with egui_kittest.
//! See docs/specs/m5-app-shadows-ui.md.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use modeler::chrome::{MenuState, menu_bar};
use modeler::input::Action;
use modeler::shadows_panel::{
    ShadowsEvent, date_label, minutes_of_day, percent_label, shadows_panel, time_label,
    with_day_of_year, with_minutes,
};
use modeler::viewstate::{Effect, ViewState};
use modeler_kernel::sun::{LocalTime, ShadowSettings};

fn t(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> LocalTime {
    LocalTime::new(year, month, day, hour, minute).unwrap()
}

struct State {
    settings: ShadowSettings,
    events: Vec<ShadowsEvent>,
}

fn harness(settings: ShadowSettings) -> Harness<'static, State> {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut State| {
            let event = shadows_panel(ui, &mut state.settings);
            if event != ShadowsEvent::None {
                state.events.push(event);
            }
        },
        State {
            settings,
            events: Vec::new(),
        },
    );
    h.run();
    h
}

#[test]
fn minutes_of_day_counts_from_midnight() {
    assert_eq!(minutes_of_day(&t(2026, 6, 21, 0, 0)), 0);
    assert_eq!(minutes_of_day(&t(2026, 6, 21, 7, 5)), 425);
    assert_eq!(minutes_of_day(&t(2026, 6, 21, 23, 59)), 1439);
}

#[test]
fn with_minutes_sets_the_clock_and_keeps_the_date() {
    let time = t(2026, 6, 21, 9, 30);
    assert_eq!(with_minutes(&time, 0), t(2026, 6, 21, 0, 0));
    assert_eq!(with_minutes(&time, 425), t(2026, 6, 21, 7, 5));
    assert_eq!(with_minutes(&time, 1439), t(2026, 6, 21, 23, 59));
}

#[test]
fn with_minutes_clamps_to_the_last_minute() {
    assert_eq!(
        with_minutes(&t(2026, 6, 21, 9, 30), 5000),
        t(2026, 6, 21, 23, 59)
    );
}

#[test]
fn with_day_of_year_sets_the_date_and_keeps_the_clock() {
    let time = t(2026, 6, 21, 9, 30);
    assert_eq!(with_day_of_year(&time, 1), t(2026, 1, 1, 9, 30));
    assert_eq!(with_day_of_year(&time, 60), t(2026, 3, 1, 9, 30));
    assert_eq!(with_day_of_year(&time, 365), t(2026, 12, 31, 9, 30));
}

#[test]
fn with_day_of_year_knows_leap_years_and_clamps() {
    let leap = t(2028, 6, 21, 12, 0);
    assert_eq!(with_day_of_year(&leap, 60), t(2028, 2, 29, 12, 0));
    assert_eq!(with_day_of_year(&leap, 366), t(2028, 12, 31, 12, 0));
    let plain = t(2026, 6, 21, 12, 0);
    assert_eq!(with_day_of_year(&plain, 366), t(2026, 12, 31, 12, 0));
    assert_eq!(with_day_of_year(&plain, 0), t(2026, 1, 1, 12, 0));
    assert_eq!(with_day_of_year(&plain, 4000), t(2026, 12, 31, 12, 0));
}

#[test]
fn day_and_minute_setters_round_trip_every_day_of_a_leap_year() {
    let base = t(2028, 1, 1, 13, 7);
    for day in 1..=366 {
        let time = with_day_of_year(&base, day);
        assert!(time.is_valid());
        assert_eq!(time.day_of_year(), day);
        assert_eq!(minutes_of_day(&time), 13 * 60 + 7);
    }
}

#[test]
fn labels_pad_with_zeros() {
    assert_eq!(time_label(&t(2026, 6, 21, 7, 5)), "07:05");
    assert_eq!(time_label(&t(2026, 6, 21, 23, 59)), "23:59");
    assert_eq!(date_label(&t(2026, 6, 21, 7, 5)), "2026-06-21");
    assert_eq!(date_label(&t(2026, 1, 2, 7, 5)), "2026-01-02");
}

#[test]
fn percent_label_rounds_to_whole_percent() {
    assert_eq!(percent_label(0.0), "0%");
    assert_eq!(percent_label(0.8), "80%");
    assert_eq!(percent_label(0.455), "46%");
    assert_eq!(percent_label(1.0), "100%");
}

#[test]
fn panel_shows_its_widgets_and_the_current_time_and_date() {
    let settings = ShadowSettings {
        time: t(2026, 6, 21, 7, 5),
        ..ShadowSettings::default()
    };
    let h = harness(settings);
    for label in [
        "Shadows",
        "Show Shadows",
        "Sun Shading",
        "Shadows on Ground",
        "Time of Day",
        "Date",
        "Light",
        "Dark",
        "07:05",
        "2026-06-21",
        "80%",
        "45%",
    ] {
        h.get_by_label(label);
    }
}

#[test]
fn an_untouched_panel_reports_nothing() {
    let mut h = harness(ShadowSettings::default());
    h.run();
    assert!(h.state().events.is_empty());
    assert_eq!(h.state().settings, ShadowSettings::default());
}

#[test]
fn clicking_show_shadows_toggles_it_and_commits() {
    let mut h = harness(ShadowSettings::default());
    h.get_by_label("Show Shadows").click();
    h.run();
    assert!(h.state().settings.shadows);
    assert_eq!(h.state().events, vec![ShadowsEvent::Commit]);
    h.get_by_label("Show Shadows").click();
    h.run();
    assert!(!h.state().settings.shadows);
    assert_eq!(h.state().events, vec![ShadowsEvent::Commit; 2]);
}

#[test]
fn clicking_sun_shading_and_ground_toggle_them_and_commit() {
    let mut h = harness(ShadowSettings::default());
    h.get_by_label("Sun Shading").click();
    h.run();
    assert!(h.state().settings.sun_shading);
    h.get_by_label("Shadows on Ground").click();
    h.run();
    assert!(!h.state().settings.on_ground);
    assert_eq!(h.state().events, vec![ShadowsEvent::Commit; 2]);
    let want = ShadowSettings {
        sun_shading: true,
        on_ground: false,
        ..ShadowSettings::default()
    };
    assert_eq!(h.state().settings, want);
}

#[test]
fn arrow_keys_on_the_time_slider_step_the_clock_and_commit() {
    let settings = ShadowSettings {
        time: t(2026, 6, 21, 9, 30),
        ..ShadowSettings::default()
    };
    let mut h = harness(settings);
    h.get_by_label("Time of Day").focus();
    h.run();
    h.key_press(eframe::egui::Key::ArrowRight);
    h.run();
    let after = h.state().settings.time;
    assert_eq!(minutes_of_day(&after), 9 * 60 + 31);
    assert_eq!((after.year, after.month, after.day), (2026, 6, 21));
    assert_eq!(h.state().events, vec![ShadowsEvent::Commit]);
}

#[test]
fn arrow_keys_on_the_date_slider_step_the_day_and_keep_the_clock() {
    let settings = ShadowSettings {
        time: t(2026, 6, 21, 9, 30),
        ..ShadowSettings::default()
    };
    let mut h = harness(settings);
    h.get_by_label("Date").focus();
    h.run();
    h.key_press(eframe::egui::Key::ArrowRight);
    h.run();
    assert_eq!(h.state().settings.time, t(2026, 6, 22, 9, 30));
    assert_eq!(h.state().events, vec![ShadowsEvent::Commit]);
}

#[test]
fn arrow_keys_on_light_and_dark_change_only_that_value() {
    let mut h = harness(ShadowSettings::default());
    h.get_by_label("Light").focus();
    h.run();
    h.key_press(eframe::egui::Key::ArrowLeft);
    h.run();
    let s = h.state().settings;
    assert!(s.light < ShadowSettings::default().light);
    assert_eq!(s.dark, ShadowSettings::default().dark);
    assert!((0.0..=1.0).contains(&s.light));
    assert_eq!(h.state().events, vec![ShadowsEvent::Commit]);
}

// The View menu item.

fn choose_shadows(shadows: bool) -> Vec<Action> {
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = seen.clone();
    let state = MenuState {
        shadows,
        ..MenuState::default()
    };
    let mut h = Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            sink.borrow_mut().push(action);
        }
    });
    h.run();
    h.get_by_label("View").click();
    h.run();
    h.get_by_label("Shadows").click();
    h.run();
    let out = seen.borrow().clone();
    drop(h);
    out
}

#[test]
fn view_menu_has_a_shadows_item_that_toggles_the_window() {
    assert_eq!(choose_shadows(false), vec![Action::ToggleShadows]);
    assert_eq!(choose_shadows(true), vec![Action::ToggleShadows]);
}

#[test]
fn toggle_shadows_asks_the_app_to_toggle_the_window() {
    let mut view = ViewState::default();
    assert_eq!(
        view.apply(Action::ToggleShadows, (800.0, 600.0)),
        Effect::ToggleShadows
    );
}
