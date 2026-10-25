// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! A small globe for picking a place: an orthographic view of the Earth with land outlines
//! (Natural Earth 1:110m land, public domain, converted by `tools/land_data.py`), a grid
//! every 30 degrees and a marker. Dragging turns it; a click picks a latitude and longitude.
//!
//! Behaviour: `docs/specs/m5-geo-globe.md`.

use eframe::egui;
use glam::DVec2;

/// The outline asset; format in `tools/land_data.py`.
const LAND_110M: &[u8] = include_bytes!("../assets/land_110m.bin");

/// Which way the globe faces: the place at the middle of the disk, degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobeView {
    /// -90..=90.
    pub latitude: f64,
    /// -180..=180.
    pub longitude: f64,
}

impl GlobeView {
    /// Facing (`latitude`, `longitude`): latitude clamped to -90..=90, longitude wrapped
    /// into -180..=180.
    pub fn facing(latitude: f64, longitude: f64) -> GlobeView {
        let _ = (latitude, longitude);
        unimplemented!("m5-geo-globe")
    }

    /// Where a place shows on the disk: x right (east at the middle), y up (north), radius 1.
    /// `None` when it is on the far side.
    pub fn project(&self, latitude: f64, longitude: f64) -> Option<DVec2> {
        let _ = (latitude, longitude);
        unimplemented!("m5-geo-globe")
    }

    /// The place (latitude, longitude) at disk point `p`, longitude in -180..=180. `None`
    /// outside the disk.
    pub fn unproject(&self, p: DVec2) -> Option<(f64, f64)> {
        let _ = p;
        unimplemented!("m5-geo-globe")
    }

    /// Turn by a drag of `delta` disk units (x right, y up) so the surface follows the
    /// pointer.
    pub fn turn(&mut self, delta: DVec2) {
        let _ = delta;
        unimplemented!("m5-geo-globe")
    }
}

/// Wrap a longitude into -180..=180.
pub fn wrap_longitude(longitude: f64) -> f64 {
    let _ = longitude;
    unimplemented!("m5-geo-globe")
}

/// Land outlines: rings of (latitude, longitude) in degrees, decoded once from the asset.
pub fn land() -> &'static [Vec<(f64, f64)>] {
    let _ = LAND_110M;
    unimplemented!("m5-geo-globe")
}

/// UTC offset guessed from a longitude: the nearest whole hour of `longitude / 15`.
pub fn utc_offset_estimate(longitude: f64) -> f64 {
    let _ = longitude;
    unimplemented!("m5-geo-globe")
}

/// Draw the globe `size` points across. `marker` is drawn when it is on the near side.
/// Dragging turns `view`. Returns the place clicked, if any, this frame.
pub fn globe(
    ui: &mut egui::Ui,
    view: &mut GlobeView,
    marker: Option<(f64, f64)>,
    size: f32,
) -> Option<(f64, f64)> {
    let _ = (ui, view, marker, size);
    unimplemented!("m5-geo-globe")
}
