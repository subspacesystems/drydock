// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Geolocation section of Model Info: typed latitude, longitude, north angle and UTC
//! offset, checked before they are applied. Kept apart from `App` so tests can drive it with
//! `egui_kittest`.
//!
//! Behaviour: `docs/specs/m5-app-geo-ui.md`.

use drydock_kernel::sun::Geolocation;
use eframe::egui;

use crate::globe::GlobeView;

/// The text typed into the four fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeoForm {
    pub latitude: String,
    pub longitude: String,
    pub north_angle: String,
    pub utc_offset: String,
}

impl GeoForm {
    /// The form showing `geolocation`: each number printed with `{}` (shortest text that
    /// reads back exactly, so 45.0 shows as "45").
    pub fn from_geolocation(geolocation: &Geolocation) -> GeoForm {
        GeoForm {
            latitude: format!("{}", geolocation.latitude),
            longitude: format!("{}", geolocation.longitude),
            north_angle: format!("{}", geolocation.north_angle),
            utc_offset: format!("{}", geolocation.utc_offset),
        }
    }
}

impl Default for GeoForm {
    /// The form of `Geolocation::default()`.
    fn default() -> GeoForm {
        GeoForm::from_geolocation(&Geolocation::default())
    }
}

/// Read the form. The error is the message to show, naming the first bad field in the order
/// latitude, longitude, north angle, UTC offset.
pub fn parse_geolocation(form: &GeoForm) -> Result<Geolocation, String> {
    Ok(Geolocation {
        latitude: parse_field("Latitude", &form.latitude, -90.0, 90.0)?,
        longitude: parse_field("Longitude", &form.longitude, -180.0, 180.0)?,
        north_angle: parse_field("North angle", &form.north_angle, -360.0, 360.0)?,
        utc_offset: parse_field("UTC offset", &form.utc_offset, -12.0, 14.0)?,
    })
}

fn parse_field(name: &str, text: &str, low: f64, high: f64) -> Result<f64, String> {
    let value = text.trim().parse::<f64>().ok();
    match value {
        Some(value) if value.is_finite() && (low..=high).contains(&value) => Ok(value),
        _ => Err(format!("{name} must be a number from {low} to {high}")),
    }
}

/// Draw the section into `ui`. `Some` on the frame the user applies a valid form (the Apply
/// button, or Enter in a field).
pub fn geo_panel(ui: &mut egui::Ui, form: &mut GeoForm) -> Option<Geolocation> {
    let submitted = geo_fields(ui, form);
    apply_row(ui, form, submitted)
}

/// The four labelled fields and the error line of [`geo_panel`], without the Apply button
/// (the window's button row has it). True when Enter was pressed in a field.
pub fn geo_fields(ui: &mut egui::Ui, form: &mut GeoForm) -> bool {
    ui.heading("Geolocation");
    geo_inputs(ui, form)
}

/// The globe above [`geo_fields`] (the globe part of [`geo_panel_with_globe`]). True when
/// Enter was pressed in a field.
pub fn geo_fields_with_globe(ui: &mut egui::Ui, form: &mut GeoForm, view: &mut GlobeView) -> bool {
    ui.heading("Geolocation");
    show_globe(ui, form, view);
    ui.label("Drag to turn, click to pick. UTC offset is estimated from longitude.");
    geo_inputs(ui, form)
}

/// The globe above [`geo_panel`]: clicking it fills the latitude, longitude and an estimated
/// UTC offset into `form` (nothing is applied until the user does). Behaviour:
/// `docs/specs/m5-geo-globe.md`.
pub fn geo_panel_with_globe(
    ui: &mut egui::Ui,
    form: &mut GeoForm,
    view: &mut GlobeView,
) -> Option<Geolocation> {
    let submitted = geo_fields_with_globe(ui, form, view);
    apply_row(ui, form, submitted)
}

/// The four fields and the error line, without the heading.
fn geo_inputs(ui: &mut egui::Ui, form: &mut GeoForm) -> bool {
    let mut submitted = field_row(ui, "Latitude (deg)", &mut form.latitude);
    submitted |= field_row(ui, "Longitude (deg)", &mut form.longitude);
    submitted |= field_row(ui, "North angle (deg)", &mut form.north_angle);
    submitted |= field_row(ui, "UTC offset (h)", &mut form.utc_offset);

    if let Err(message) = parse_geolocation(form) {
        ui.colored_label(egui::Color32::RED, message);
    }
    submitted
}

/// The globe and the form's marker, filling in a picked place.
fn show_globe(ui: &mut egui::Ui, form: &mut GeoForm, view: &mut GlobeView) {
    let marker = parse_geolocation(form)
        .ok()
        .map(|geolocation| (geolocation.latitude, geolocation.longitude));
    if let Some((latitude, longitude)) = crate::globe::globe(ui, view, marker, 220.0) {
        form.latitude = format!("{}", round_to_4(latitude));
        form.longitude = format!("{}", round_to_4(longitude));
        form.utc_offset = format!("{}", crate::globe::utc_offset_estimate(longitude));
    }
}

/// The Apply button row of [`geo_panel`]. `submitted` is Enter in a field.
fn apply_row(ui: &mut egui::Ui, form: &mut GeoForm, submitted: bool) -> Option<Geolocation> {
    let parsed = parse_geolocation(form);
    let apply = crate::dialog::button_row(
        ui,
        &[crate::dialog::DialogButton {
            label: "Apply",
            default: true,
            enabled: parsed.is_ok(),
        }],
    );
    if parsed.is_ok() && (submitted || apply == Some(0)) {
        parsed.ok()
    } else {
        None
    }
}

fn round_to_4(value: f64) -> f64 {
    let rounded = (value * 10_000.0).round() / 10_000.0;
    if rounded == 0.0 { 0.0 } else { rounded }
}

/// One label and its text field, returning whether Enter was pressed in the field.
fn field_row(ui: &mut egui::Ui, label: &str, text: &mut String) -> bool {
    ui.horizontal(|ui| {
        let label = ui.label(crate::dialog::field_label(label));
        let response = ui
            .add(egui::TextEdit::singleline(text))
            .labelled_by(label.id);
        response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
    })
    .inner
}
