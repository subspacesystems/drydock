// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! A small globe for picking a place: an orthographic view of the Earth with land outlines
//! (Natural Earth 1:110m land, public domain, converted by `tools/land_data.py`), a grid
//! every 30 degrees and a marker. Dragging turns it; a click picks a latitude and longitude.
//!
//! Behaviour: `docs/specs/m5-geo-globe.md`.

use std::sync::OnceLock;

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
        GlobeView {
            latitude: latitude.clamp(-90.0, 90.0),
            longitude: wrap_longitude(longitude),
        }
    }

    /// Where a place shows on the disk: x right (east at the middle), y up (north), radius 1.
    /// `None` when it is on the far side.
    pub fn project(&self, latitude: f64, longitude: f64) -> Option<DVec2> {
        let phi = latitude.to_radians();
        let lambda = longitude.to_radians();
        let phi0 = self.latitude.to_radians();
        let lambda0 = self.longitude.to_radians();
        let d = lambda - lambda0;
        let (sin_phi, cos_phi) = phi.sin_cos();
        let (sin_phi0, cos_phi0) = phi0.sin_cos();
        let (sin_d, cos_d) = d.sin_cos();
        let near = sin_phi0 * sin_phi + cos_phi0 * cos_phi * cos_d;
        if near < 0.0 {
            return None;
        }
        Some(DVec2::new(
            cos_phi * sin_d,
            cos_phi0 * sin_phi - sin_phi0 * cos_phi * cos_d,
        ))
    }

    /// The place (latitude, longitude) at disk point `p`, longitude in -180..=180. `None`
    /// outside the disk.
    pub fn unproject(&self, p: DVec2) -> Option<(f64, f64)> {
        let r2 = p.length_squared();
        if r2 > 1.0 {
            return None;
        }
        let z = (1.0 - r2).max(0.0).sqrt();
        let phi0 = self.latitude.to_radians();
        let lambda0 = self.longitude.to_radians();
        let (sin_phi0, cos_phi0) = phi0.sin_cos();
        let sin_phi = (z * sin_phi0 + p.y * cos_phi0).clamp(-1.0, 1.0);
        let latitude = sin_phi.asin().to_degrees();
        let longitude =
            wrap_longitude((lambda0 + p.x.atan2(z * cos_phi0 - p.y * sin_phi0)).to_degrees());
        Some((latitude, longitude))
    }

    /// Turn by a drag of `delta` disk units (x right, y up) so the surface follows the
    /// pointer.
    pub fn turn(&mut self, delta: DVec2) {
        self.longitude = wrap_longitude(self.longitude - delta.x.to_degrees());
        self.latitude = (self.latitude - delta.y.to_degrees()).clamp(-90.0, 90.0);
    }
}

/// Wrap a longitude into -180..=180.
pub fn wrap_longitude(longitude: f64) -> f64 {
    let wrapped = longitude.rem_euclid(360.0);
    if wrapped > 180.0 {
        wrapped - 360.0
    } else {
        wrapped
    }
}

/// Land outlines: rings of (latitude, longitude) in degrees, decoded once from the asset.
pub fn land() -> &'static [Vec<(f64, f64)>] {
    static LAND: OnceLock<Vec<Vec<(f64, f64)>>> = OnceLock::new();
    LAND.get_or_init(|| decode_land(LAND_110M))
}

/// UTC offset guessed from a longitude: the nearest whole hour of `longitude / 15`.
pub fn utc_offset_estimate(longitude: f64) -> f64 {
    let hours = (longitude / 15.0).round();
    if hours == 0.0 { 0.0 } else { hours }
}

fn read_u16(bytes: &[u8], pos: &mut usize) -> u16 {
    let value = u16::from_le_bytes([bytes[*pos], bytes[*pos + 1]]);
    *pos += 2;
    value
}

fn decode_land(bytes: &[u8]) -> Vec<Vec<(f64, f64)>> {
    let mut pos = 0;
    let ring_count = read_u16(bytes, &mut pos);
    let mut rings = Vec::with_capacity(usize::from(ring_count));
    for _ in 0..ring_count {
        let point_count = read_u16(bytes, &mut pos);
        let mut ring = Vec::with_capacity(usize::from(point_count));
        for _ in 0..point_count {
            let longitude = f64::from(i16::from_le_bytes([bytes[pos], bytes[pos + 1]])) / 100.0;
            pos += 2;
            let latitude = f64::from(i16::from_le_bytes([bytes[pos], bytes[pos + 1]])) / 100.0;
            pos += 2;
            ring.push((latitude, longitude));
        }
        rings.push(ring);
    }
    rings
}

const SEA: egui::Color32 = egui::Color32::from_rgb(32, 74, 112);
const GRID: egui::Color32 = egui::Color32::from_rgba_premultiplied(220, 230, 240, 45);
const LAND_LINE: egui::Color32 = egui::Color32::from_rgb(226, 220, 198);
const LIMB: egui::Color32 = egui::Color32::from_rgb(180, 195, 210);
const MARKER: egui::Color32 = egui::Color32::from_rgb(220, 40, 40);

fn to_screen(center: egui::Pos2, radius: f32, p: DVec2) -> egui::Pos2 {
    egui::pos2(
        center.x + p.x as f32 * radius,
        center.y - p.y as f32 * radius,
    )
}

/// Draw a path, splitting it wherever it passes to the far side.
fn draw_path(
    painter: &egui::Painter,
    view: &GlobeView,
    center: egui::Pos2,
    radius: f32,
    points: &[(f64, f64)],
    stroke: egui::Stroke,
) {
    let mut previous = None;
    for &(latitude, longitude) in points {
        previous = match view.project(latitude, longitude) {
            Some(p) => {
                let screen = to_screen(center, radius, p);
                if let Some(last) = previous {
                    painter.line_segment([last, screen], stroke);
                }
                Some(screen)
            }
            None => None,
        };
    }
}

/// Draw a ring (not repeating its first point) as connected land outlines.
fn draw_ring(
    painter: &egui::Painter,
    view: &GlobeView,
    center: egui::Pos2,
    radius: f32,
    ring: &[(f64, f64)],
    stroke: egui::Stroke,
) {
    if ring.len() < 2 {
        return;
    }
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        if let (Some(pa), Some(pb)) = (view.project(a.0, a.1), view.project(b.0, b.1)) {
            painter.line_segment(
                [to_screen(center, radius, pa), to_screen(center, radius, pb)],
                stroke,
            );
        }
    }
}

fn latitude_circle(latitude: f64) -> Vec<(f64, f64)> {
    (-180..=180)
        .step_by(3)
        .map(|longitude| (latitude, f64::from(longitude)))
        .collect()
}

fn longitude_circle(longitude: f64) -> Vec<(f64, f64)> {
    (-90..=90)
        .step_by(3)
        .map(|latitude| (f64::from(latitude), longitude))
        .collect()
}

/// Draw the globe `size` points across. `marker` is drawn when it is on the near side.
/// Dragging turns `view`. Returns the place clicked, if any, this frame.
pub fn globe(
    ui: &mut egui::Ui,
    view: &mut GlobeView,
    marker: Option<(f64, f64)>,
    size: f32,
) -> Option<(f64, f64)> {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click_and_drag());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Globe"));

    let center = rect.center();
    let radius = size * 0.5;
    let painter = ui.painter().with_clip_rect(rect);

    painter.circle_filled(center, radius, SEA);

    let grid_stroke = egui::Stroke::new(1.0, GRID);
    for latitude in [-60.0, -30.0, 0.0, 30.0, 60.0] {
        draw_path(
            &painter,
            view,
            center,
            radius,
            &latitude_circle(latitude),
            grid_stroke,
        );
    }
    for longitude in (-180..180).step_by(30) {
        draw_path(
            &painter,
            view,
            center,
            radius,
            &longitude_circle(f64::from(longitude)),
            grid_stroke,
        );
    }

    let land_stroke = egui::Stroke::new(1.0, LAND_LINE);
    for ring in land() {
        draw_ring(&painter, view, center, radius, ring, land_stroke);
    }

    painter.circle_stroke(center, radius, egui::Stroke::new(1.0, LIMB));

    if let Some((latitude, longitude)) = marker
        && let Some(p) = view.project(latitude, longitude)
    {
        let at = to_screen(center, radius, p);
        painter.circle_filled(at, 4.0, MARKER);
        painter.circle_stroke(at, 4.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
    }

    if response.dragged() {
        let delta = if response.drag_started() {
            response
                .total_drag_delta()
                .unwrap_or_else(|| response.drag_delta())
        } else {
            response.drag_delta()
        };
        view.turn(DVec2::new(
            f64::from(delta.x) / f64::from(radius),
            -f64::from(delta.y) / f64::from(radius),
        ));
    }

    if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
    {
        let p = DVec2::new(
            f64::from((pos.x - center.x) / radius),
            f64::from((center.y - pos.y) / radius),
        );
        if p.length() <= 1.0 {
            return view.unproject(p);
        }
    }

    None
}
