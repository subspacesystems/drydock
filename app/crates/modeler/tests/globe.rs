// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m5-geo-globe: the globe's math and land outlines (`docs/specs/m5-geo-globe.md`).

use glam::DVec2;
use modeler::globe::{GlobeView, land, utc_offset_estimate, wrap_longitude};
use proptest::prelude::*;

fn close(a: DVec2, b: DVec2, tol: f64) -> bool {
    (a - b).length() <= tol
}

#[test]
fn facing_clamps_latitude_and_wraps_longitude() {
    assert_eq!(
        GlobeView::facing(40.0, -105.0),
        GlobeView {
            latitude: 40.0,
            longitude: -105.0
        }
    );
    assert_eq!(GlobeView::facing(95.0, 0.0).latitude, 90.0);
    assert_eq!(GlobeView::facing(-120.0, 0.0).latitude, -90.0);
    let v = GlobeView::facing(0.0, 190.0);
    assert!((v.longitude - -170.0).abs() < 1e-9, "{v:?}");
    let v = GlobeView::facing(0.0, -540.0);
    assert!((v.longitude.abs() - 180.0).abs() < 1e-9, "{v:?}");
}

#[test]
fn wrap_longitude_stays_in_range() {
    assert_eq!(wrap_longitude(0.0), 0.0);
    assert_eq!(wrap_longitude(180.0), 180.0);
    assert!((wrap_longitude(181.0) - -179.0).abs() < 1e-9);
    assert!((wrap_longitude(-181.0) - 179.0).abs() < 1e-9);
    assert!((wrap_longitude(725.0) - 5.0).abs() < 1e-9);
}

#[test]
fn the_middle_is_the_place_faced() {
    let v = GlobeView::facing(40.0, -105.0);
    assert!(close(v.project(40.0, -105.0).unwrap(), DVec2::ZERO, 1e-12));
    let (lat, lon) = v.unproject(DVec2::ZERO).unwrap();
    assert!((lat - 40.0).abs() < 1e-9 && (lon - -105.0).abs() < 1e-9);
}

#[test]
fn north_is_up_and_east_is_right() {
    let v = GlobeView::facing(0.0, 0.0);
    assert!(close(
        v.project(90.0, 0.0).unwrap(),
        DVec2::new(0.0, 1.0),
        1e-9
    ));
    assert!(close(
        v.project(-90.0, 0.0).unwrap(),
        DVec2::new(0.0, -1.0),
        1e-9
    ));
    assert!(close(
        v.project(0.0, 90.0).unwrap(),
        DVec2::new(1.0, 0.0),
        1e-9
    ));
    assert!(close(
        v.project(0.0, -90.0).unwrap(),
        DVec2::new(-1.0, 0.0),
        1e-9
    ));
    let p = v.project(30.0, 30.0).unwrap();
    assert!(p.x > 0.0 && p.y > 0.0, "{p}");
    // Orthographic: the height of a place is the sine of its latitude.
    assert!((v.project(30.0, 0.0).unwrap().y - 0.5).abs() < 1e-9);
}

#[test]
fn the_far_side_is_hidden() {
    let v = GlobeView::facing(0.0, 0.0);
    assert_eq!(v.project(0.0, 180.0), None);
    assert_eq!(v.project(0.0, 135.0), None);
    assert_eq!(v.project(-60.0, -170.0), None);
    let v = GlobeView::facing(90.0, 0.0);
    assert_eq!(v.project(-10.0, 45.0), None);
    assert!(v.project(10.0, 45.0).is_some());
}

#[test]
fn outside_the_disk_is_no_place() {
    let v = GlobeView::facing(12.0, 34.0);
    assert_eq!(v.unproject(DVec2::new(0.8, 0.8)), None);
    assert_eq!(v.unproject(DVec2::new(-1.01, 0.0)), None);
    assert!(v.unproject(DVec2::new(0.0, 0.999)).is_some());
}

#[test]
fn dragging_moves_the_surface_with_the_pointer() {
    let mut v = GlobeView::facing(0.0, 20.0);
    v.turn(DVec2::new(0.1, 0.0));
    // The place that was in the middle moved right by about the drag.
    let p = v.project(0.0, 20.0).unwrap();
    assert!((p.x - 0.1).abs() < 0.01 && p.y.abs() < 1e-9, "{p}");
    assert!(v.longitude < 20.0);

    let mut v = GlobeView::facing(0.0, 20.0);
    v.turn(DVec2::new(0.0, 0.1));
    let p = v.project(0.0, 20.0).unwrap();
    assert!((p.y - 0.1).abs() < 0.01 && p.x.abs() < 1e-9, "{p}");
    assert!(v.latitude < 0.0);
}

#[test]
fn turning_stops_at_the_poles_and_wraps_around() {
    let mut v = GlobeView::facing(80.0, 0.0);
    v.turn(DVec2::new(0.0, -2.0));
    assert_eq!(v.latitude, 90.0);
    let mut v = GlobeView::facing(-80.0, 0.0);
    v.turn(DVec2::new(0.0, 2.0));
    assert_eq!(v.latitude, -90.0);
    let mut v = GlobeView::facing(0.0, -170.0);
    v.turn(DVec2::new(0.5, 0.0));
    assert!((-180.0..=180.0).contains(&v.longitude), "{v:?}");
    assert!(v.longitude > 0.0, "turned past the date line: {v:?}");
}

#[test]
fn land_outlines_are_loaded() {
    let rings = land();
    assert!(rings.len() >= 100, "{} rings", rings.len());
    let points: usize = rings.iter().map(Vec::len).sum();
    assert!(points >= 4000, "{points} points");
    for ring in rings {
        assert!(ring.len() >= 3);
        for &(lat, lon) in ring {
            assert!((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon));
        }
    }
}

/// Even-odd test in the latitude/longitude plane (fine away from the date line).
fn on_land(lat: f64, lon: f64) -> bool {
    let mut inside = false;
    for ring in land() {
        let mut j = ring.len() - 1;
        for i in 0..ring.len() {
            let (yi, xi) = ring[i];
            let (yj, xj) = ring[j];
            if (yi > lat) != (yj > lat) && lon < (xj - xi) * (lat - yi) / (yj - yi) + xi {
                inside = !inside;
            }
            j = i;
        }
    }
    inside
}

#[test]
fn land_is_where_the_continents_are() {
    for (name, lat, lon) in [
        ("Paris", 48.86, 2.35),
        ("Denver", 39.74, -104.99),
        ("Sydney", -33.7, 150.9),
        ("Nairobi", -1.29, 36.82),
        ("Sao Paulo", -23.55, -46.63),
        ("Novosibirsk", 55.0, 82.9),
    ] {
        assert!(on_land(lat, lon), "{name} should be on land");
    }
    for (name, lat, lon) in [
        ("mid Pacific", 0.0, -150.0),
        ("mid Atlantic", 30.0, -40.0),
        ("Indian Ocean", -20.0, 75.0),
        ("Southern Ocean", -55.0, 0.0),
    ] {
        assert!(!on_land(lat, lon), "{name} should be sea");
    }
}

#[test]
fn utc_offset_is_the_nearest_hour_of_longitude() {
    for (lon, want) in [
        (0.0, 0.0),
        (7.4, 0.0),
        (7.6, 1.0),
        (-104.99, -7.0),
        (151.21, 10.0),
        (-80.0, -5.0),
        (180.0, 12.0),
        (-180.0, -12.0),
    ] {
        assert_eq!(utc_offset_estimate(lon), want, "longitude {lon}");
    }
}

fn arb_view() -> impl Strategy<Value = GlobeView> {
    (-90.0..=90.0f64, -180.0..=180.0f64).prop_map(|(a, b)| GlobeView::facing(a, b))
}

proptest! {
    #[test]
    fn project_then_unproject_gives_the_place(
        v in arb_view(),
        lat in -89.9..89.9f64,
        lon in -180.0..180.0f64,
    ) {
        if let Some(p) = v.project(lat, lon) {
            prop_assert!(p.length() <= 1.0 + 1e-12);
            // Right at the limb the inverse is ill-conditioned.
            if p.length() < 0.999 {
                let (a, b) = v.unproject(p).unwrap();
                prop_assert!((a - lat).abs() < 1e-6, "lat {lat} -> {a}");
                let d = wrap_longitude(b - lon).abs();
                prop_assert!(d < 1e-6, "lon {lon} -> {b}");
            }
        }
    }

    #[test]
    fn unproject_then_project_gives_the_point(
        v in arb_view(),
        r in 0.0..0.999f64,
        angle in 0.0..std::f64::consts::TAU,
    ) {
        let p = DVec2::new(r * angle.cos(), r * angle.sin());
        let (lat, lon) = v.unproject(p).unwrap();
        prop_assert!((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon));
        let q = v.project(lat, lon).unwrap();
        prop_assert!(close(p, q, 1e-9), "{p} -> ({lat}, {lon}) -> {q}");
    }
}
