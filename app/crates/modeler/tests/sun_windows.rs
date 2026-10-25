// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m5-app-sun-windows: `SunWindows`, driven with egui_kittest and directly.
//! See docs/specs/m5-app-sun-windows.md.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use modeler::chrome::model_info_window_with;
use modeler::editor::Editor;
use modeler::shading::{sun_light, sun_light_for};
use modeler::shadows_panel::ShadowsEvent;
use modeler::sun_windows::SunWindows;
use modeler_kernel::model::Model;
use modeler_kernel::sun::{Geolocation, LocalTime, ShadowSettings};

struct State {
    windows: SunWindows,
    model: Model,
    editor: Editor,
    changed: usize,
}

fn settings(shadows: bool, hour: u32) -> ShadowSettings {
    ShadowSettings {
        time: LocalTime::new(2026, 6, 21, hour, 0).unwrap(),
        shadows,
        sun_shading: true,
        light: 0.75,
        dark: 0.4,
        on_ground: true,
    }
}

fn state() -> State {
    State {
        windows: SunWindows::new(),
        model: Model::new(),
        editor: Editor::new(),
        changed: 0,
    }
}

fn harness(state: State) -> Harness<'static, State> {
    let mut h = Harness::new_ui_state(
        |ui, s: &mut State| {
            let ctx = ui.ctx().clone();
            if s.windows.show(&ctx, &mut s.model, &mut s.editor) {
                s.changed += 1;
            }
        },
        state,
    );
    h.run();
    h
}

fn undo_label(s: &State) -> Option<String> {
    s.editor.undo_stack().undo_label().map(str::to_owned)
}

#[test]
fn new_has_both_windows_closed_and_the_models_sun() {
    let s = state();
    assert!(!s.windows.shadows_open && !s.windows.geo_open);
    assert_eq!(s.windows.sun_light(&s.model), sun_light(&s.model));
}

#[test]
fn preview_changes_the_sun_but_records_nothing() {
    let mut s = state();
    let want = settings(true, 15);
    let changed =
        s.windows
            .apply_shadows_event(ShadowsEvent::Preview, want, &mut s.model, &mut s.editor);
    assert!(!changed);
    assert_eq!(undo_label(&s), None);
    assert_eq!(s.model.shadow_settings(), ShadowSettings::default());
    assert_eq!(
        s.windows.sun_light(&s.model),
        sun_light_for(&s.model.geolocation(), &want)
    );
    assert!(s.windows.sun_light(&s.model).is_some());
}

#[test]
fn none_keeps_a_preview() {
    let mut s = state();
    let want = settings(true, 15);
    s.windows
        .apply_shadows_event(ShadowsEvent::Preview, want, &mut s.model, &mut s.editor);
    let changed = s.windows.apply_shadows_event(
        ShadowsEvent::None,
        settings(true, 9),
        &mut s.model,
        &mut s.editor,
    );
    assert!(!changed);
    assert_eq!(
        s.windows.sun_light(&s.model),
        sun_light_for(&s.model.geolocation(), &want)
    );
}

#[test]
fn commit_records_one_step_and_ends_the_preview() {
    let mut s = state();
    let want = settings(true, 15);
    s.windows
        .apply_shadows_event(ShadowsEvent::Preview, want, &mut s.model, &mut s.editor);
    let changed =
        s.windows
            .apply_shadows_event(ShadowsEvent::Commit, want, &mut s.model, &mut s.editor);
    assert!(changed);
    assert_eq!(s.model.shadow_settings(), want);
    assert_eq!(undo_label(&s).as_deref(), Some("Shadow Settings"));
    assert_eq!(s.windows.sun_light(&s.model), sun_light(&s.model));
}

#[test]
fn commit_of_unchanged_settings_records_nothing() {
    let mut s = state();
    let same = s.model.shadow_settings();
    s.windows.apply_shadows_event(
        ShadowsEvent::Preview,
        settings(true, 10),
        &mut s.model,
        &mut s.editor,
    );
    let changed =
        s.windows
            .apply_shadows_event(ShadowsEvent::Commit, same, &mut s.model, &mut s.editor);
    assert!(!changed);
    assert_eq!(undo_label(&s), None);
    assert_eq!(s.windows.sun_light(&s.model), sun_light(&s.model));
}

#[test]
fn closed_windows_draw_nothing() {
    let h = harness(state());
    assert!(h.query_by_label("Shadows").is_none());
    assert!(h.query_by_label("Geolocation").is_none());
}

#[test]
fn shadows_window_shows_the_models_settings() {
    let mut s = state();
    s.windows.shadows_open = true;
    let h = harness(s);
    h.get_by_label("Show Shadows");
    h.get_by_label("Time of Day");
    h.get_by_label("Close");
}

#[test]
fn clicking_a_checkbox_in_the_shadows_window_commits_one_step() {
    let mut s = state();
    s.windows.shadows_open = true;
    let before = s.model.shadow_settings();
    let mut h = harness(s);
    h.get_by_label("Show Shadows").click();
    h.run();
    let s = h.state();
    assert_eq!(s.model.shadow_settings().shadows, !before.shadows);
    assert_eq!(undo_label(s).as_deref(), Some("Shadow Settings"));
    assert_eq!(s.changed, 1);
}

#[test]
fn the_shadows_window_follows_undo() {
    let mut s = state();
    s.windows.shadows_open = true;
    let mut h = harness(s);
    h.get_by_label("Show Shadows").click();
    h.run();
    // Change the model behind the window's back; the window must show it next frame.
    let target = settings(true, 8);
    {
        let st = h.state_mut();
        st.editor.set_shadow_settings(&mut st.model, target);
    }
    h.run();
    h.get_by_label("Show Shadows");
    assert_eq!(h.state().model.shadow_settings(), target);
    assert_eq!(h.state().changed, 1, "only the click counts as a change");
}

#[test]
fn close_button_closes_the_shadows_window() {
    let mut s = state();
    s.windows.shadows_open = true;
    let mut h = harness(s);
    h.get_by_label("Close").click();
    h.run();
    assert!(!h.state().windows.shadows_open);
    assert!(h.query_by_label("Show Shadows").is_none());
}

#[test]
fn closing_the_shadows_window_drops_a_preview() {
    let mut s = state();
    s.windows.shadows_open = true;
    s.windows.apply_shadows_event(
        ShadowsEvent::Preview,
        settings(true, 17),
        &mut s.model,
        &mut s.editor,
    );
    let mut h = harness(s);
    h.get_by_label("Close").click();
    h.run();
    let s = h.state();
    assert_eq!(s.windows.sun_light(&s.model), sun_light(&s.model));
}

#[test]
fn geolocation_window_shows_the_models_location() {
    let mut s = state();
    let g = Geolocation {
        latitude: 47.5,
        longitude: -122.25,
        north_angle: 10.0,
        utc_offset: -8.0,
    };
    let mut log = modeler_kernel::model::ModelEdit::new();
    s.model.set_geolocation(g, &mut log).unwrap();
    s.windows.geo_open = true;
    let h = harness(s);
    h.get_by_label("Latitude (deg)");
    assert_eq!(
        h.get_by_label("Latitude (deg)").value().as_deref(),
        Some("47.5")
    );
    assert_eq!(
        h.get_by_label("Longitude (deg)").value().as_deref(),
        Some("-122.25")
    );
}

#[test]
fn applying_the_geolocation_sets_it_in_one_step_and_closes() {
    let mut s = state();
    s.windows.geo_open = true;
    let mut h = harness(s);
    h.get_by_label("Latitude (deg)").focus();
    h.get_by_label("Latitude (deg)").type_text("47.5");
    h.run();
    let typed: f64 = h
        .get_by_label("Latitude (deg)")
        .value()
        .unwrap()
        .parse()
        .unwrap();
    assert_ne!(typed, Geolocation::default().latitude);
    h.get_by_label("Apply").click();
    h.run();
    let s = h.state();
    assert_eq!(s.model.geolocation().latitude, typed);
    assert_eq!(undo_label(s).as_deref(), Some("Set Location"));
    assert_eq!(s.changed, 1);
    assert!(!s.windows.geo_open);
}

#[test]
fn a_bad_geolocation_changes_nothing_and_stays_open() {
    let mut s = state();
    s.windows.geo_open = true;
    let mut h = harness(s);
    h.get_by_label("Latitude (deg)").focus();
    h.get_by_label("Latitude (deg)").type_text("95");
    h.run();
    h.get_by_label_contains("Latitude must be a number from -90 to 90");
    let s = h.state();
    assert_eq!(undo_label(s), None);
    assert!(s.windows.geo_open);
}

#[test]
fn reopening_geolocation_shows_the_current_values_not_stale_text() {
    let mut s = state();
    s.windows.geo_open = true;
    let mut h = harness(s);
    h.get_by_label("Latitude (deg)").focus();
    h.get_by_label("Latitude (deg)").type_text("12");
    h.run();
    h.get_by_label("Close").click();
    h.run();
    assert!(!h.state().windows.geo_open);
    h.state_mut().windows.geo_open = true;
    h.run();
    let want = format!("{}", Geolocation::default().latitude);
    assert_eq!(
        h.get_by_label("Latitude (deg)").value().as_deref(),
        Some(want.as_str())
    );
}

#[test]
fn model_info_window_with_draws_the_extra_widgets_above_close() {
    let mut h = Harness::new_ui_state(
        |ui, open: &mut bool| {
            let ctx = ui.ctx().clone();
            model_info_window_with(&ctx, open, &["3 faces".to_owned()], |ui| {
                let _ = ui.button("Geolocation...");
            });
        },
        true,
    );
    h.run();
    h.get_by_label("3 faces");
    h.get_by_label("Geolocation...");
    h.get_by_label("Close").click();
    h.run();
    assert!(!*h.state());
}
