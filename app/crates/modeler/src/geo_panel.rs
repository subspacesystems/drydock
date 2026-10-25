// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Geolocation section of Model Info: typed latitude, longitude, north angle and UTC
//! offset, checked before they are applied. Kept apart from `App` so tests can drive it with
//! `egui_kittest`.
//!
//! Behaviour: `docs/specs/m5-app-geo-ui.md`.

use eframe::egui;
use modeler_kernel::sun::Geolocation;

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
        let _ = geolocation;
        unimplemented!("m5-app-geo-ui")
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
    let _ = form;
    unimplemented!("m5-app-geo-ui")
}

/// Draw the section into `ui`. `Some` on the frame the user applies a valid form (the Apply
/// button, or Enter in a field).
pub fn geo_panel(ui: &mut egui::Ui, form: &mut GeoForm) -> Option<Geolocation> {
    let _ = (ui, form);
    unimplemented!("m5-app-geo-ui")
}
