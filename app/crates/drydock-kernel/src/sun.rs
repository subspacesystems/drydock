// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The sun: where the model is on Earth, the date and time there, and where the sun is in
//! the model's sky. The renderer turns the direction into light and shadows.
//!
//! Behaviour: `docs/specs/m5-solar.md` (solar position), `docs/specs/m5-geo-model.md`
//! (settings stored in the model).

use glam::DVec3;

/// Where the model is on Earth and how it is turned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geolocation {
    /// Degrees north of the equator, -90..=90.
    pub latitude: f64,
    /// Degrees east of Greenwich, -180..=180.
    pub longitude: f64,
    /// Degrees from the model's +Y (green) axis to true north, clockwise seen from above
    /// (from +Z). 0: north is +Y; 90: north is +X.
    pub north_angle: f64,
    /// Hours the local clock is ahead of UTC, -12..=14 (fractions allowed). No daylight
    /// saving.
    pub utc_offset: f64,
}

impl Default for Geolocation {
    fn default() -> Self {
        Geolocation {
            latitude: 45.0,
            longitude: 0.0,
            north_angle: 0.0,
            utc_offset: 0.0,
        }
    }
}

/// A date (Gregorian calendar) and clock time on the local clock of a `Geolocation`.
/// Build checked values with [`LocalTime::new`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    /// 1..=12.
    pub month: u32,
    /// 1..=days_in_month(year, month).
    pub day: u32,
    /// 0..=23.
    pub hour: u32,
    /// 0..=59.
    pub minute: u32,
}

impl Default for LocalTime {
    fn default() -> Self {
        LocalTime {
            year: 2026,
            month: 9,
            day: 21,
            hour: 14,
            minute: 0,
        }
    }
}

impl LocalTime {
    /// `None` when a field is out of range (see the field docs). Years -4000..=10000.
    pub fn new(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> Option<LocalTime> {
        let t = LocalTime {
            year,
            month,
            day,
            hour,
            minute,
        };
        t.is_valid().then_some(t)
    }

    /// Whether every field is in range (what [`LocalTime::new`] checks).
    pub fn is_valid(&self) -> bool {
        (-4000..=10000).contains(&self.year)
            && (1..=12).contains(&self.month)
            && self.day >= 1
            && self.day <= days_in_month(self.year, self.month)
            && self.hour <= 23
            && self.minute <= 59
    }

    /// 1 for January 1st, up to 365 or 366.
    pub fn day_of_year(&self) -> u32 {
        (1..self.month)
            .map(|m| days_in_month(self.year, m))
            .sum::<u32>()
            + self.day
    }

    /// The date that is day `day` (1-based) of `year`, at `hour:minute`; `None` when out of
    /// range.
    pub fn from_day_of_year(year: i32, day: u32, hour: u32, minute: u32) -> Option<LocalTime> {
        if !(-4000..=10000).contains(&year) || day == 0 {
            return None;
        }
        let mut left = day;
        for month in 1..=12 {
            let n = days_in_month(year, month);
            if left <= n {
                return LocalTime::new(year, month, left, hour, minute);
            }
            left -= n;
        }
        None
    }
}

/// Days in `month` (1..=12) of `year` in the Gregorian calendar; 0 for any other month.
pub fn days_in_month(year: i32, month: u32) -> u32 {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// The Julian day of a Gregorian date, `hours` (UTC, may be fractional, negative or past 24)
/// after its midnight.
pub fn julian_day(year: i32, month: u32, day: u32, hours: f64) -> f64 {
    let (mut y, mut m) = (year, month);
    if m <= 2 {
        y -= 1;
        m += 12;
    }
    let a = (y as f64 / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * (y as f64 + 4716.0)).floor() + (30.6001 * (m as f64 + 1.0)).floor() + day as f64 + b
        - 1524.5
        + hours / 24.0
}

/// The sun's place on the sky at one instant, independent of the observer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarCoordinates {
    /// Apparent declination, degrees.
    pub declination: f64,
    /// Apparent right ascension, degrees in 0..360.
    pub right_ascension: f64,
    /// Equation of time: apparent minus mean solar time, minutes.
    pub equation_of_time: f64,
}

/// The sun's coordinates at Julian day `jd`.
pub fn solar_coordinates(jd: f64) -> SolarCoordinates {
    let t = (jd - 2451545.0) / 36525.0;

    let mean_longitude = 280.46646 + t * (36000.76983 + t * 0.0003032);
    let mean_anomaly = 357.52911 + t * (35999.05029 - t * 0.0001537);
    let eccentricity = 0.016708634 - t * (0.000042037 + t * 0.0000001267);

    let m = mean_anomaly.to_radians();
    let center = (1.914602 - t * (0.004817 + t * 0.000014)) * m.sin()
        + (0.019993 - t * 0.000101) * (2.0 * m).sin()
        + 0.000289 * (3.0 * m).sin();
    let true_longitude = mean_longitude + center;

    let omega = 125.04 - 1934.136 * t;
    let omega_r = omega.to_radians();
    let apparent_longitude = true_longitude - 0.00569 - 0.00478 * omega_r.sin();

    let mean_obliquity =
        23.0 + (26.0 + (21.448 - t * (46.815 + t * (0.00059 - t * 0.001813))) / 60.0) / 60.0;
    let obliquity = mean_obliquity + 0.00256 * omega_r.cos();

    let lambda = apparent_longitude.to_radians();
    let eps = obliquity.to_radians();
    let declination = (eps.sin() * lambda.sin()).asin().to_degrees();

    let right_ascension = (eps.cos() * lambda.sin())
        .atan2(lambda.cos())
        .to_degrees()
        .rem_euclid(360.0);

    let y = (eps / 2.0).tan();
    let y = y * y;
    let l0 = mean_longitude.to_radians();
    let equation_of_time = 4.0
        * (y * (2.0 * l0).sin() - 2.0 * eccentricity * m.sin()
            + 4.0 * eccentricity * y * m.sin() * (2.0 * l0).cos()
            - 0.5 * y * y * (4.0 * l0).sin()
            - 1.25 * eccentricity * eccentricity * (2.0 * m).sin())
        .to_degrees();

    SolarCoordinates {
        declination,
        right_ascension,
        equation_of_time,
    }
}

/// The sun as an observer at a place sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarPosition {
    /// Degrees clockwise from true north, 0..360.
    pub azimuth: f64,
    /// Degrees above the horizon, with atmospheric refraction; negative below it.
    pub elevation: f64,
}

/// Where the sun is seen from `geo` at local time `time`.
pub fn solar_position(geo: &Geolocation, time: &LocalTime) -> SolarPosition {
    let utc_hours = time.hour as f64 + time.minute as f64 / 60.0 - geo.utc_offset;
    let jd = julian_day(time.year, time.month, time.day, utc_hours);
    solar_position_at(geo.latitude, geo.longitude, jd)
}

/// Where the sun is seen from `latitude`, `longitude` (degrees, east positive) at Julian
/// day `jd` (UTC).
pub fn solar_position_at(latitude: f64, longitude: f64, jd: f64) -> SolarPosition {
    let coords = solar_coordinates(jd);

    let minutes_since_midnight = (jd + 0.5).rem_euclid(1.0) * 1440.0;
    let true_solar_time = minutes_since_midnight + coords.equation_of_time + 4.0 * longitude;
    let hour_angle = true_solar_time / 4.0 - 180.0;

    let lat = latitude.to_radians();
    let dec = coords.declination.to_radians();
    let ha = hour_angle.to_radians();

    let cos_zenith = (lat.sin() * dec.sin() + lat.cos() * dec.cos() * ha.cos()).clamp(-1.0, 1.0);
    let zenith = cos_zenith.acos();
    let geometric_elevation = 90.0 - zenith.to_degrees();

    let east = -dec.cos() * ha.sin();
    let north = dec.sin() * lat.cos() - dec.cos() * lat.sin() * ha.cos();
    let azimuth = east.atan2(north).to_degrees().rem_euclid(360.0);

    let elevation = geometric_elevation + refraction(geometric_elevation);

    SolarPosition { azimuth, elevation }
}

/// Atmospheric refraction in degrees for a geometric elevation in degrees, at standard
/// pressure and temperature.
fn refraction(elevation: f64) -> f64 {
    if elevation > 85.0 {
        return 0.0;
    }
    let te = elevation.to_radians().tan();
    let arcseconds = if elevation > 5.0 {
        58.1 / te - 0.07 / (te * te * te) + 0.000086 / (te * te * te * te * te)
    } else if elevation > -0.575 {
        1735.0
            + elevation * (-518.2 + elevation * (103.4 + elevation * (-12.79 + elevation * 0.711)))
    } else {
        -20.772 / te
    };
    arcseconds / 3600.0
}

/// Unit vector toward a sun at `position`, in model axes (Z up) for a model whose
/// true north is `north_angle` (see [`Geolocation::north_angle`]).
pub fn direction_from(position: SolarPosition, north_angle: f64) -> DVec3 {
    let azimuth = (position.azimuth + north_angle).to_radians();
    let elevation = position.elevation.to_radians();
    DVec3::new(
        azimuth.sin() * elevation.cos(),
        azimuth.cos() * elevation.cos(),
        elevation.sin(),
    )
}

/// Unit vector toward the sun seen from `geo` at `time`, in model axes.
pub fn sun_direction(geo: &Geolocation, time: &LocalTime) -> DVec3 {
    direction_from(solar_position(geo, time), geo.north_angle)
}

/// How the sun lights the model, stored with it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowSettings {
    /// The date and time on the location's clock.
    pub time: LocalTime,
    /// Faces cast shadows.
    pub shadows: bool,
    /// Faces are shaded by the sun's direction even while `shadows` is off.
    pub sun_shading: bool,
    /// Share of a face's color that direct sunlight adds, 0..=1.
    pub light: f64,
    /// Share of a face's color it keeps in shade, 0..=1.
    pub dark: f64,
    /// Shadows also fall on the ground (z = 0) where there's no face.
    pub on_ground: bool,
}

impl Default for ShadowSettings {
    fn default() -> Self {
        ShadowSettings {
            time: LocalTime::default(),
            shadows: false,
            sun_shading: false,
            light: 0.8,
            dark: 0.45,
            on_ground: true,
        }
    }
}
