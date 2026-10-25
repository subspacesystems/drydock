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
    let _ = (year, month, day, hours);
    unimplemented!("m5-solar")
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
    let _ = jd;
    unimplemented!("m5-solar")
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
    let _ = (geo, time);
    unimplemented!("m5-solar")
}

/// Where the sun is seen from `latitude`, `longitude` (degrees, east positive) at Julian
/// day `jd` (UTC).
pub fn solar_position_at(latitude: f64, longitude: f64, jd: f64) -> SolarPosition {
    let _ = (latitude, longitude, jd);
    unimplemented!("m5-solar")
}

/// Unit vector toward a sun at `position`, in model axes (Z up) for a model whose
/// true north is `north_angle` (see [`Geolocation::north_angle`]).
pub fn direction_from(position: SolarPosition, north_angle: f64) -> DVec3 {
    let _ = (position, north_angle);
    unimplemented!("m5-solar")
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
