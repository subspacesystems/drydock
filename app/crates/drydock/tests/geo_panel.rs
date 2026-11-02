// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m5-app-geo-ui: the Geolocation form, driven with egui_kittest.
//! See docs/specs/m5-app-geo-ui.md.

use drydock::geo_panel::{GeoForm, geo_panel, parse_geolocation};
use drydock_kernel::sun::Geolocation;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

fn form(lat: &str, lon: &str, north: &str, utc: &str) -> GeoForm {
    GeoForm {
        latitude: lat.into(),
        longitude: lon.into(),
        north_angle: north.into(),
        utc_offset: utc.into(),
    }
}

#[test]
fn from_geolocation_prints_the_shortest_exact_text() {
    let g = Geolocation {
        latitude: 45.0,
        longitude: -122.6765,
        north_angle: 12.5,
        utc_offset: 5.5,
    };
    assert_eq!(
        GeoForm::from_geolocation(&g),
        form("45", "-122.6765", "12.5", "5.5")
    );
}

#[test]
fn default_form_is_the_default_geolocation() {
    assert_eq!(
        GeoForm::default(),
        GeoForm::from_geolocation(&Geolocation::default())
    );
    assert_eq!(
        parse_geolocation(&GeoForm::default()),
        Ok(Geolocation::default())
    );
}

#[test]
fn form_round_trips_through_text() {
    for g in [
        Geolocation::default(),
        Geolocation {
            latitude: -33.8688,
            longitude: 151.2093,
            north_angle: -30.25,
            utc_offset: 10.0,
        },
        Geolocation {
            latitude: 90.0,
            longitude: -180.0,
            north_angle: 360.0,
            utc_offset: 14.0,
        },
    ] {
        assert_eq!(parse_geolocation(&GeoForm::from_geolocation(&g)), Ok(g));
    }
}

#[test]
fn parse_trims_spaces() {
    let g = parse_geolocation(&form(" 40.5 ", "-74", "0", " -5 ")).unwrap();
    assert_eq!(g.latitude, 40.5);
    assert_eq!(g.longitude, -74.0);
    assert_eq!(g.utc_offset, -5.0);
}

#[test]
fn parse_accepts_the_range_ends() {
    assert!(parse_geolocation(&form("-90", "180", "-360", "-12")).is_ok());
    assert!(parse_geolocation(&form("90", "-180", "360", "14")).is_ok());
}

#[test]
fn parse_names_the_first_bad_field() {
    assert_eq!(
        parse_geolocation(&form("91", "0", "0", "0")),
        Err("Latitude must be a number from -90 to 90".to_string())
    );
    assert_eq!(
        parse_geolocation(&form("0", "abc", "0", "0")),
        Err("Longitude must be a number from -180 to 180".to_string())
    );
    assert_eq!(
        parse_geolocation(&form("0", "0", "361", "0")),
        Err("North angle must be a number from -360 to 360".to_string())
    );
    assert_eq!(
        parse_geolocation(&form("0", "0", "0", "14.5")),
        Err("UTC offset must be a number from -12 to 14".to_string())
    );
    // Latitude is reported before the later bad fields.
    assert_eq!(
        parse_geolocation(&form("", "999", "999", "999")),
        Err("Latitude must be a number from -90 to 90".to_string())
    );
}

#[test]
fn parse_rejects_non_finite_text() {
    for bad in ["", " ", "NaN", "inf", "-inf", "1e999", "5'", "1,5"] {
        assert!(
            parse_geolocation(&form(bad, "0", "0", "0")).is_err(),
            "{bad:?}"
        );
        assert!(
            parse_geolocation(&form("0", bad, "0", "0")).is_err(),
            "{bad:?}"
        );
        assert!(
            parse_geolocation(&form("0", "0", bad, "0")).is_err(),
            "{bad:?}"
        );
        assert!(
            parse_geolocation(&form("0", "0", "0", bad)).is_err(),
            "{bad:?}"
        );
    }
}

struct State {
    form: GeoForm,
    applied: Vec<Geolocation>,
}

fn harness(form: GeoForm) -> Harness<'static, State> {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut State| {
            if let Some(g) = geo_panel(ui, &mut state.form) {
                state.applied.push(g);
            }
        },
        State {
            form,
            applied: Vec::new(),
        },
    );
    h.run();
    h
}

fn apply_disabled(h: &Harness<'_, State>) -> bool {
    h.get_by_label("Apply").accesskit_node().is_disabled()
}

#[test]
fn panel_shows_its_widgets() {
    let h = harness(GeoForm::default());
    for label in [
        "Geolocation",
        "Latitude (deg):",
        "Longitude (deg):",
        "North angle (deg):",
        "UTC offset (h):",
        "Apply",
    ] {
        h.get_by_label(label);
    }
    assert!(!apply_disabled(&h));
}

#[test]
fn untouched_panel_applies_nothing() {
    let mut h = harness(GeoForm::default());
    h.run();
    assert!(h.state().applied.is_empty());
}

#[test]
fn apply_reports_the_form_once() {
    let mut h = harness(form("40.5", "-74", "15", "-5"));
    h.get_by_label("Apply").click();
    h.run();
    assert_eq!(
        h.state().applied,
        vec![Geolocation {
            latitude: 40.5,
            longitude: -74.0,
            north_angle: 15.0,
            utc_offset: -5.0,
        }]
    );
    h.run();
    assert_eq!(h.state().applied.len(), 1);
}

#[test]
fn a_bad_field_shows_its_message_and_disables_apply() {
    let mut h = harness(form("95", "0", "0", "0"));
    h.get_by_label_contains("Latitude must be a number from -90 to 90");
    assert!(apply_disabled(&h));
    h.get_by_label("Apply").click();
    h.run();
    assert!(h.state().applied.is_empty());
}

#[test]
fn typing_into_an_empty_field_fills_the_form() {
    let mut h = harness(form("", "0", "0", "0"));
    assert!(apply_disabled(&h));
    h.get_by_label("Latitude (deg):").focus();
    h.run();
    h.get_by_label("Latitude (deg):").type_text("47.5");
    h.run();
    assert_eq!(h.state().form.latitude, "47.5");
    assert!(!apply_disabled(&h));
    h.get_by_label("Apply").click();
    h.run();
    assert_eq!(h.state().applied.len(), 1);
    assert_eq!(h.state().applied[0].latitude, 47.5);
}
