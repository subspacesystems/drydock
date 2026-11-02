// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m5-geo-globe: picking a place on the globe, driven with egui_kittest
//! (`docs/specs/m5-geo-globe.md`).

use drydock::editor::Editor;
use drydock::geo_panel::{GeoForm, geo_panel_with_globe};
use drydock::globe::{GlobeView, utc_offset_estimate};
use drydock::sun_windows::SunWindows;
use drydock_kernel::model::{Model, ModelEdit};
use drydock_kernel::sun::Geolocation;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glam::DVec2;

struct State {
    form: GeoForm,
    view: GlobeView,
    applied: Vec<Geolocation>,
}

fn form(lat: &str, lon: &str, north: &str, utc: &str) -> GeoForm {
    GeoForm {
        latitude: lat.into(),
        longitude: lon.into(),
        north_angle: north.into(),
        utc_offset: utc.into(),
    }
}

fn harness(form: GeoForm, view: GlobeView) -> Harness<'static, State> {
    let mut h = Harness::new_ui_state(
        |ui, s: &mut State| {
            if let Some(g) = geo_panel_with_globe(ui, &mut s.form, &mut s.view) {
                s.applied.push(g);
            }
        },
        State {
            form,
            view,
            applied: Vec::new(),
        },
    );
    h.run();
    h
}

/// The globe's disk: center and radius in points.
fn disk(h: &Harness<'_, State>) -> (egui::Pos2, f32) {
    let r = h.get_by_label("Globe").rect();
    (r.center(), r.width().min(r.height()) / 2.0)
}

/// Screen position of disk point `p` (x right, y up).
fn at(h: &Harness<'_, State>, p: DVec2) -> egui::Pos2 {
    let (c, r) = disk(h);
    egui::pos2(c.x + p.x as f32 * r, c.y - p.y as f32 * r)
}

fn press(h: &mut Harness<'_, State>, pos: egui::Pos2, pressed: bool) {
    h.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    });
}

fn click(h: &mut Harness<'_, State>, pos: egui::Pos2) {
    h.event(egui::Event::PointerMoved(pos));
    h.run();
    press(h, pos, true);
    h.run();
    press(h, pos, false);
    h.run();
}

fn drag(h: &mut Harness<'_, State>, from: egui::Pos2, to: egui::Pos2) {
    h.event(egui::Event::PointerMoved(from));
    h.run();
    press(h, from, true);
    h.run();
    for i in 1..=8 {
        let t = i as f32 / 8.0;
        h.event(egui::Event::PointerMoved(from + (to - from) * t));
        h.run();
    }
    press(h, to, false);
    h.run();
}

fn number(text: &str) -> f64 {
    text.parse()
        .unwrap_or_else(|_| panic!("not a number: {text:?}"))
}

/// Picked numbers are printed rounded to 4 decimals.
fn assert_picked(text: &str, want: f64) {
    let got = number(text);
    assert!(
        (got - want).abs() <= 0.5e-4 + 1e-9,
        "field {text:?}, want about {want}"
    );
    let decimals = text.split('.').nth(1).map_or(0, str::len);
    assert!(decimals <= 4, "more than 4 decimals: {text:?}");
}

#[test]
fn the_globe_shows_above_the_fields() {
    let h = harness(GeoForm::default(), GlobeView::facing(0.0, 0.0));
    let globe = h.get_by_label("Globe").rect();
    assert!(
        globe.width() >= 150.0 && globe.height() >= 150.0,
        "globe {globe:?}"
    );
    assert!(
        (globe.width() - globe.height()).abs() <= 2.0,
        "not square: {globe:?}"
    );
    let latitude = h.get_by_label("Latitude (deg):").rect();
    assert!(
        globe.bottom() <= latitude.top() + 1.0,
        "globe {globe:?}, field {latitude:?}"
    );
    h.get_by_label("Apply");
}

#[test]
fn clicking_the_middle_picks_the_place_faced() {
    let mut h = harness(
        form("0", "0", "15", "3"),
        GlobeView::facing(47.6062, -122.3321),
    );
    let middle = at(&h, DVec2::ZERO);
    click(&mut h, middle);
    let f = h.state().form.clone();
    assert_picked(&f.latitude, 47.6062);
    assert_picked(&f.longitude, -122.3321);
    assert_eq!(number(&f.utc_offset), utc_offset_estimate(-122.3321));
    assert_eq!(f.north_angle, "15", "north angle is left alone");
    assert!(h.state().applied.is_empty(), "a click only fills the form");
    assert_eq!(
        h.get_by_label("Latitude (deg):").value().as_deref(),
        Some(f.latitude.as_str())
    );
}

#[test]
fn clicking_off_center_picks_the_place_under_the_pointer() {
    let view = GlobeView::facing(20.0, 10.0);
    let mut h = harness(GeoForm::default(), view);
    let p = DVec2::new(0.5, -0.3);
    let pos = at(&h, p);
    click(&mut h, pos);
    // The place under the exact pointer position (points are rounded to pixels).
    let (c, r) = disk(&h);
    let exact = DVec2::new(((pos.x - c.x) / r) as f64, ((c.y - pos.y) / r) as f64);
    let (lat, lon) = view.unproject(exact).unwrap();
    let f = h.state().form.clone();
    assert_picked(&f.latitude, lat);
    assert_picked(&f.longitude, lon);
    assert_eq!(number(&f.utc_offset), utc_offset_estimate(lon));
}

#[test]
fn clicking_outside_the_disk_picks_nothing() {
    let before = form("1", "2", "3", "4");
    let mut h = harness(before.clone(), GlobeView::facing(0.0, 0.0));
    let corner = at(&h, DVec2::new(0.97, 0.97));
    click(&mut h, corner);
    assert_eq!(h.state().form, before);
}

#[test]
fn dragging_turns_the_globe_without_picking() {
    let before = form("1", "2", "3", "4");
    let mut h = harness(before.clone(), GlobeView::facing(0.0, 30.0));
    let from = at(&h, DVec2::ZERO);
    let to = at(&h, DVec2::new(0.25, 0.0));
    drag(&mut h, from, to);
    assert_eq!(h.state().form, before, "a drag is not a pick");
    let v = h.state().view;
    // About 0.25 rad (14.3 degrees) west in the middle now.
    assert!(
        (v.longitude - (30.0 - 14.3)).abs() < 3.0,
        "view after drag: {v:?}"
    );
    assert!(v.latitude.abs() < 1.0, "{v:?}");
}

#[test]
fn a_picked_place_is_applied_with_the_rest_of_the_form() {
    let mut h = harness(
        form("0", "0", "-20", "0"),
        GlobeView::facing(-33.8688, 151.2093),
    );
    let middle = at(&h, DVec2::ZERO);
    click(&mut h, middle);
    h.get_by_label("Apply").click();
    h.run();
    let applied = &h.state().applied;
    assert_eq!(applied.len(), 1);
    let g = applied[0];
    assert!((g.latitude - -33.8688).abs() <= 1e-4);
    assert!((g.longitude - 151.2093).abs() <= 1e-4);
    assert_eq!(g.north_angle, -20.0);
    assert_eq!(g.utc_offset, 10.0);
}

#[test]
fn a_bad_form_still_lets_the_globe_pick() {
    let mut h = harness(form("abc", "", "0", "0"), GlobeView::facing(10.0, 20.0));
    let middle = at(&h, DVec2::ZERO);
    click(&mut h, middle);
    let f = h.state().form.clone();
    assert_picked(&f.latitude, 10.0);
    assert_picked(&f.longitude, 20.0);
}

// --- In the app's Geolocation window ---

struct App {
    windows: SunWindows,
    model: Model,
    editor: Editor,
}

fn app_harness(g: Geolocation) -> Harness<'static, App> {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    model.set_geolocation(g, &mut log).unwrap();
    let mut windows = SunWindows::new();
    windows.geo_open = true;
    let mut h = Harness::new_ui_state(
        |ui, s: &mut App| {
            let ctx = ui.ctx().clone();
            s.windows.show(&ctx, &mut s.model, &mut s.editor);
        },
        App {
            windows,
            model,
            editor: Editor::new(),
        },
    );
    h.run();
    h
}

fn app_click_middle(h: &mut Harness<'_, App>) {
    let pos = h.get_by_label("Globe").rect().center();
    h.event(egui::Event::PointerMoved(pos));
    h.run();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run();
    }
}

#[test]
fn the_geolocation_window_has_a_globe_facing_the_models_place() {
    let g = Geolocation {
        latitude: 51.5,
        longitude: -0.125,
        north_angle: 0.0,
        utc_offset: 0.0,
    };
    let mut h = app_harness(g);
    app_click_middle(&mut h);
    assert_picked(
        h.get_by_label("Latitude (deg):")
            .value()
            .as_deref()
            .unwrap(),
        51.5,
    );
    assert_picked(
        h.get_by_label("Longitude (deg):")
            .value()
            .as_deref()
            .unwrap(),
        -0.125,
    );
    assert!(
        h.state().windows.geo_open,
        "picking does not apply or close"
    );
    assert_eq!(h.state().model.geolocation(), g);
}

#[test]
fn reopening_the_window_faces_the_current_place_again() {
    let g = Geolocation {
        latitude: 35.0,
        longitude: 139.75,
        north_angle: 0.0,
        utc_offset: 9.0,
    };
    let mut h = app_harness(g);
    let globe = h.get_by_label("Globe").rect();
    // Turn the globe away, close the window, open it again.
    h.event(egui::Event::PointerMoved(globe.center()));
    h.run();
    h.event(egui::Event::PointerButton {
        pos: globe.center(),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    for i in 1..=6 {
        h.event(egui::Event::PointerMoved(
            globe.center() + egui::vec2(10.0 * i as f32, 5.0 * i as f32),
        ));
        h.run();
    }
    let end = globe.center() + egui::vec2(60.0, 30.0);
    h.event(egui::Event::PointerButton {
        pos: end,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    h.get_by_label("Close").click();
    h.run();
    assert!(!h.state().windows.geo_open);
    h.state_mut().windows.geo_open = true;
    h.run();
    h.run();
    app_click_middle(&mut h);
    assert_picked(
        h.get_by_label("Latitude (deg):")
            .value()
            .as_deref()
            .unwrap(),
        35.0,
    );
    assert_picked(
        h.get_by_label("Longitude (deg):")
            .value()
            .as_deref()
            .unwrap(),
        139.75,
    );
}
