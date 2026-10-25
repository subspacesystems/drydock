// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m5-solar.md`: calendar, Julian day, solar coordinates,
//! solar position and the sun's direction in model axes.

use glam::{DMat3, DVec3};
use modeler_kernel::sun::{
    Geolocation, LocalTime, SolarPosition, direction_from, julian_day, solar_coordinates,
    solar_position, solar_position_at, sun_direction,
};
use proptest::prelude::*;

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

/// Smallest difference between two angles in degrees.
fn angle_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

fn geo(latitude: f64, longitude: f64, utc_offset: f64) -> Geolocation {
    Geolocation {
        latitude,
        longitude,
        north_angle: 0.0,
        utc_offset,
    }
}

fn time(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> LocalTime {
    LocalTime::new(year, month, day, hour, minute).expect("valid test time")
}

// -- Julian day ---------------------------------------------------------------------------

#[test]
fn julian_day_matches_published_values() {
    // Standard epochs and textbook examples of the Gregorian-calendar conversion.
    assert!(close(julian_day(2000, 1, 1, 12.0), 2451545.0, 1e-9));
    assert!(close(julian_day(1987, 1, 27, 0.0), 2446822.5, 1e-9));
    assert!(close(julian_day(1987, 6, 19, 12.0), 2446966.0, 1e-9));
    assert!(close(julian_day(1988, 1, 27, 0.0), 2447187.5, 1e-9));
    assert!(close(julian_day(1988, 6, 19, 12.0), 2447332.0, 1e-9));
    assert!(close(julian_day(1900, 1, 1, 0.0), 2415020.5, 1e-9));
    assert!(close(julian_day(1600, 1, 1, 0.0), 2305447.5, 1e-9));
    assert!(close(julian_day(1600, 12, 31, 0.0), 2305812.5, 1e-9));
    assert!(close(julian_day(1957, 10, 4, 19.44), 2436116.31, 1e-6));
}

#[test]
fn julian_day_hours_add_fractions_of_a_day() {
    let base = julian_day(2026, 10, 25, 0.0);
    assert!(close(julian_day(2026, 10, 25, 6.0), base + 0.25, 1e-9));
    assert!(close(julian_day(2026, 10, 25, -12.0), base - 0.5, 1e-9));
    assert!(close(julian_day(2026, 10, 25, 36.0), base + 1.5, 1e-9));
    assert!(close(julian_day(2026, 10, 26, 0.0), base + 1.0, 1e-9));
}

// -- solar coordinates --------------------------------------------------------------------

#[test]
fn solar_coordinates_match_the_textbook_example() {
    // 1992 October 13, 0h: apparent declination -7.78507 degrees, apparent right ascension
    // 198.38083 degrees, equation of time 13 min 42.6 s.
    let c = solar_coordinates(2448908.5);
    assert!(
        close(c.declination, -7.78507, 0.01),
        "declination {}",
        c.declination
    );
    assert!(
        close(c.right_ascension, 198.38083, 0.01),
        "right ascension {}",
        c.right_ascension
    );
    assert!(
        close(c.equation_of_time, 13.71, 0.1),
        "equation of time {}",
        c.equation_of_time
    );
}

#[test]
fn declination_reaches_the_tropics_at_the_solstices() {
    let june = solar_coordinates(julian_day(2026, 6, 21, 12.0));
    let december = solar_coordinates(julian_day(2026, 12, 21, 12.0));
    let march = solar_coordinates(julian_day(2026, 3, 20, 14.75));
    assert!(close(june.declination, 23.44, 0.05), "{}", june.declination);
    assert!(
        close(december.declination, -23.44, 0.05),
        "{}",
        december.declination
    );
    assert!(close(march.declination, 0.0, 0.05), "{}", march.declination);
    for c in [june, december, march] {
        assert!((0.0..360.0).contains(&c.right_ascension));
    }
}

#[test]
fn equation_of_time_stays_within_its_yearly_range() {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    for day in 0..365 {
        let e = solar_coordinates(julian_day(2026, 1, 1, 12.0) + day as f64).equation_of_time;
        min = min.min(e);
        max = max.max(e);
    }
    // About -14.2 min in February and +16.4 min in early November.
    assert!(close(min, -14.2, 0.3), "min {min}");
    assert!(close(max, 16.4, 0.3), "max {max}");
}

// -- solar position -----------------------------------------------------------------------

#[test]
fn solar_position_matches_the_published_reference_case() {
    // 2003 October 17, 12:30:30 local time at UTC-7, latitude 39.742476, longitude
    // -105.1786: zenith 50.11162 degrees, azimuth 194.34024 degrees (east of north).
    let jd = julian_day(2003, 10, 17, 19.0 + 30.0 / 60.0 + 30.0 / 3600.0);
    let p = solar_position_at(39.742476, -105.1786, jd);
    assert!(
        close(90.0 - p.elevation, 50.11162, 0.05),
        "elevation {}",
        p.elevation
    );
    assert!(close(p.azimuth, 194.34024, 0.05), "azimuth {}", p.azimuth);
}

#[test]
fn solar_position_uses_the_local_clock() {
    let g = geo(39.742476, -105.1786, -7.0);
    let p = solar_position(&g, &time(2003, 10, 17, 12, 30));
    let q = solar_position_at(39.742476, -105.1786, julian_day(2003, 10, 17, 19.5));
    assert!(close(p.elevation, q.elevation, 1e-9));
    assert!(close(p.azimuth, q.azimuth, 1e-9));
    // Half-hour offsets are allowed.
    let india = geo(28.6, 77.2, 5.5);
    let r = solar_position(&india, &time(2026, 3, 1, 12, 0));
    let s = solar_position_at(28.6, 77.2, julian_day(2026, 3, 1, 6.5));
    assert!(close(r.elevation, s.elevation, 1e-9));
    assert!(close(r.azimuth, s.azimuth, 1e-9));
}

#[test]
fn equator_sun_passes_overhead_at_the_equinox() {
    let g = geo(0.0, 0.0, 0.0);
    let best = (0..24 * 60)
        .map(|m| solar_position(&g, &time(2026, 3, 20, m / 60, m % 60)).elevation)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(best > 89.5, "highest {best}");
}

#[test]
fn midnight_sun_at_the_north_pole_in_june() {
    let g = geo(90.0, 0.0, 0.0);
    for hour in 0..24 {
        let p = solar_position(&g, &time(2026, 6, 21, hour, 0));
        assert!(close(p.elevation, 23.44, 0.15), "{hour}h: {}", p.elevation);
    }
}

#[test]
fn night_has_the_sun_below_the_horizon() {
    let g = geo(45.0, 0.0, 0.0);
    let p = solar_position(&g, &time(2026, 12, 21, 0, 0));
    assert!(p.elevation < -20.0, "{}", p.elevation);
    assert!((0.0..360.0).contains(&p.azimuth));
}

#[test]
fn northern_sun_moves_from_east_through_south_to_west() {
    let g = geo(45.0, 0.0, 0.0);
    let morning = solar_position(&g, &time(2026, 6, 21, 7, 0));
    let noon = solar_position(&g, &time(2026, 6, 21, 12, 0));
    let evening = solar_position(&g, &time(2026, 6, 21, 17, 0));
    assert!(
        morning.azimuth > 45.0 && morning.azimuth < 135.0,
        "{morning:?}"
    );
    assert!(angle_diff(noon.azimuth, 180.0) < 10.0, "{noon:?}");
    assert!(
        evening.azimuth > 225.0 && evening.azimuth < 315.0,
        "{evening:?}"
    );
    // Highest at noon: 90 - 45 + 23.44.
    assert!(close(noon.elevation, 68.4, 0.5), "{noon:?}");
}

#[test]
fn southern_noon_sun_is_in_the_north() {
    let g = geo(-34.0, 0.0, 0.0);
    let noon = solar_position(&g, &time(2026, 6, 21, 12, 0));
    assert!(angle_diff(noon.azimuth, 0.0) < 10.0, "{noon:?}");
    assert!(close(noon.elevation, 90.0 - 34.0 - 23.44, 0.5), "{noon:?}");
}

#[test]
fn refraction_lifts_the_sun_near_the_horizon() {
    // Refraction is about half a degree at the horizon and negligible high up. Compare the
    // geometric elevation computed from the declination and hour angle by hand.
    let lat: f64 = 45.0;
    let jd = julian_day(2026, 6, 21, 12.0);
    let c = solar_coordinates(jd);
    let high = solar_position_at(lat, 0.0, jd);
    let geometric_high = {
        let tst = 12.0 * 60.0 + c.equation_of_time;
        let ha = (tst / 4.0 - 180.0).to_radians();
        let (l, d) = (lat.to_radians(), c.declination.to_radians());
        (l.sin() * d.sin() + l.cos() * d.cos() * ha.cos())
            .asin()
            .to_degrees()
    };
    assert!(
        (high.elevation - geometric_high) > 0.0 && (high.elevation - geometric_high) < 0.02,
        "{} vs {geometric_high}",
        high.elevation
    );
    // Find a time when the geometric sun is about at the horizon in the evening.
    let mut found = false;
    for minute in 0..(24 * 60) {
        let jd = julian_day(2026, 6, 21, minute as f64 / 60.0);
        let c = solar_coordinates(jd);
        let tst = minute as f64 + c.equation_of_time;
        let ha = (tst / 4.0 - 180.0).to_radians();
        let (l, d) = (lat.to_radians(), c.declination.to_radians());
        let geometric = (l.sin() * d.sin() + l.cos() * d.cos() * ha.cos())
            .asin()
            .to_degrees();
        if minute > 12 * 60 && geometric.abs() < 0.2 {
            let apparent = solar_position_at(lat, 0.0, jd).elevation;
            let lift = apparent - geometric;
            assert!(lift > 0.3 && lift < 0.7, "lift {lift} at {geometric}");
            found = true;
            break;
        }
    }
    assert!(found);
}

// -- direction in model axes --------------------------------------------------------------

#[test]
fn direction_points_by_azimuth_and_elevation() {
    let p = |azimuth, elevation| SolarPosition { azimuth, elevation };
    let close_v = |a: DVec3, b: DVec3| a.distance(b) < 1e-12;
    assert!(close_v(direction_from(p(0.0, 0.0), 0.0), DVec3::Y));
    assert!(close_v(direction_from(p(90.0, 0.0), 0.0), DVec3::X));
    assert!(close_v(direction_from(p(180.0, 0.0), 0.0), -DVec3::Y));
    assert!(close_v(direction_from(p(270.0, 0.0), 0.0), -DVec3::X));
    assert!(close_v(direction_from(p(123.0, 90.0), 0.0), DVec3::Z));
    let d = direction_from(p(180.0, 30.0), 0.0);
    assert!(close_v(
        d,
        DVec3::new(0.0, -(30f64.to_radians().cos()), 0.5)
    ));
}

#[test]
fn north_angle_turns_north_clockwise() {
    let north = SolarPosition {
        azimuth: 0.0,
        elevation: 0.0,
    };
    let close_v = |a: DVec3, b: DVec3| a.distance(b) < 1e-12;
    assert!(close_v(direction_from(north, 90.0), DVec3::X));
    assert!(close_v(direction_from(north, -90.0), -DVec3::X));
    assert!(close_v(direction_from(north, 180.0), -DVec3::Y));
}

#[test]
fn sun_direction_combines_position_and_north_angle() {
    let mut g = geo(51.5, -0.1, 1.0);
    g.north_angle = 30.0;
    let t = time(2026, 7, 1, 16, 45);
    let expected = direction_from(solar_position(&g, &t), 30.0);
    assert!(sun_direction(&g, &t).distance(expected) < 1e-12);
}

#[test]
fn defaults_are_valid() {
    let g = Geolocation::default();
    assert!((-90.0..=90.0).contains(&g.latitude));
    assert!((-180.0..=180.0).contains(&g.longitude));
    assert!(LocalTime::default().is_valid());
    let d = sun_direction(&g, &LocalTime::default());
    assert!(close(d.length(), 1.0, 1e-12));
    // Mid-afternoon in September at 45 degrees north: up and to the south-west.
    assert!(d.z > 0.3 && d.y < 0.0 && d.x < 0.0, "{d:?}");
}

// -- properties ---------------------------------------------------------------------------

fn any_time() -> impl Strategy<Value = LocalTime> {
    (1900i32..2100, 1u32..=366, 0u32..24, 0u32..60)
        .prop_filter_map("day in year", |(y, d, h, m)| {
            LocalTime::from_day_of_year(y, d, h, m)
        })
}

proptest! {
    #[test]
    fn consecutive_days_are_one_julian_day_apart(t in any_time()) {
        let jd = julian_day(t.year, t.month, t.day, 0.0);
        let next = LocalTime::from_day_of_year(t.year, t.day_of_year() + 1, 0, 0)
            .or_else(|| LocalTime::from_day_of_year(t.year + 1, 1, 0, 0))
            .unwrap();
        prop_assert!(close(julian_day(next.year, next.month, next.day, 0.0) - jd, 1.0, 1e-9));
    }

    #[test]
    fn positions_are_in_range(
        lat in -90.0f64..=90.0,
        lon in -180.0f64..=180.0,
        offset in -12.0f64..=14.0,
        north in -360.0f64..=360.0,
        t in any_time(),
    ) {
        let g = Geolocation { latitude: lat, longitude: lon, north_angle: north, utc_offset: offset };
        let p = solar_position(&g, &t);
        prop_assert!((0.0..360.0).contains(&p.azimuth), "{:?}", p);
        prop_assert!((-90.0..=91.0).contains(&p.elevation), "{:?}", p);
        let d = sun_direction(&g, &t);
        prop_assert!(close(d.length(), 1.0, 1e-9));
        prop_assert!(close(d.z, p.elevation.to_radians().sin(), 1e-9));
    }

    #[test]
    fn north_angle_is_a_turn_about_z(
        azimuth in 0.0f64..360.0,
        elevation in -90.0f64..=90.0,
        north in -720.0f64..720.0,
    ) {
        let p = SolarPosition { azimuth, elevation };
        let turned = DMat3::from_rotation_z(-north.to_radians()) * direction_from(p, 0.0);
        prop_assert!(direction_from(p, north).distance(turned) < 1e-9);
    }

    #[test]
    fn same_solar_time_gives_nearly_the_same_sun(
        lat in -60.0f64..=60.0,
        lon in -150.0f64..=150.0,
        t in any_time(),
    ) {
        // Fifteen degrees east with the clock one hour ahead is the same local solar time,
        // one hour earlier in UTC.
        let a = solar_position(&geo(lat, lon, lon / 15.0), &t);
        let b = solar_position(&geo(lat, lon + 15.0, lon / 15.0 + 1.0), &t);
        prop_assert!(close(a.elevation, b.elevation, 0.05), "{:?} {:?}", a, b);
        if a.elevation.abs() < 85.0 {
            prop_assert!(angle_diff(a.azimuth, b.azimuth) < 0.2, "{:?} {:?}", a, b);
        }
    }
}
