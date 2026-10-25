// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Calendar helpers of `modeler_kernel::sun` (`docs/specs/m5-solar.md`), implemented on main
//! ahead of the task so the shadow settings can check their dates.

use modeler_kernel::sun::{LocalTime, days_in_month};
use proptest::prelude::*;

fn time(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> LocalTime {
    LocalTime::new(year, month, day, hour, minute).expect("valid test time")
}

// -- calendar -----------------------------------------------------------------------------

#[test]
fn days_in_month_follows_the_gregorian_calendar() {
    let common = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for (i, n) in common.iter().enumerate() {
        assert_eq!(
            days_in_month(2026, i as u32 + 1),
            *n,
            "2026 month {}",
            i + 1
        );
    }
    assert_eq!(days_in_month(2024, 2), 29);
    assert_eq!(days_in_month(2000, 2), 29);
    assert_eq!(days_in_month(1900, 2), 28);
    assert_eq!(days_in_month(2100, 2), 28);
    assert_eq!(days_in_month(2026, 0), 0);
    assert_eq!(days_in_month(2026, 13), 0);
}

#[test]
fn local_time_new_checks_every_field() {
    assert!(LocalTime::new(2024, 2, 29, 0, 0).is_some());
    assert!(LocalTime::new(2023, 2, 29, 0, 0).is_none());
    assert!(LocalTime::new(2026, 12, 31, 23, 59).is_some());
    assert!(LocalTime::new(2026, 13, 1, 0, 0).is_none());
    assert!(LocalTime::new(2026, 0, 1, 0, 0).is_none());
    assert!(LocalTime::new(2026, 4, 31, 0, 0).is_none());
    assert!(LocalTime::new(2026, 4, 0, 0, 0).is_none());
    assert!(LocalTime::new(2026, 4, 1, 24, 0).is_none());
    assert!(LocalTime::new(2026, 4, 1, 0, 60).is_none());
    assert!(LocalTime::new(10001, 1, 1, 0, 0).is_none());
    assert!(LocalTime::new(-4001, 1, 1, 0, 0).is_none());
    let t = LocalTime::new(2026, 7, 4, 9, 30).unwrap();
    assert_eq!(
        (t.year, t.month, t.day, t.hour, t.minute),
        (2026, 7, 4, 9, 30)
    );
}

#[test]
fn is_valid_matches_new() {
    let bad = LocalTime {
        year: 2026,
        month: 2,
        day: 30,
        hour: 0,
        minute: 0,
    };
    assert!(!bad.is_valid());
    assert!(LocalTime::default().is_valid());
    assert!(time(2026, 2, 28, 12, 0).is_valid());
}

#[test]
fn day_of_year_counts_from_january_first() {
    assert_eq!(time(2026, 1, 1, 0, 0).day_of_year(), 1);
    assert_eq!(time(1978, 11, 14, 0, 0).day_of_year(), 318);
    assert_eq!(time(1988, 4, 22, 0, 0).day_of_year(), 113);
    assert_eq!(time(2026, 12, 31, 0, 0).day_of_year(), 365);
    assert_eq!(time(2024, 12, 31, 0, 0).day_of_year(), 366);
    assert_eq!(time(2024, 3, 1, 0, 0).day_of_year(), 61);
}

#[test]
fn from_day_of_year_finds_the_date() {
    assert_eq!(
        LocalTime::from_day_of_year(2026, 1, 8, 15),
        Some(time(2026, 1, 1, 8, 15))
    );
    assert_eq!(
        LocalTime::from_day_of_year(2024, 60, 0, 0),
        Some(time(2024, 2, 29, 0, 0))
    );
    assert_eq!(
        LocalTime::from_day_of_year(2026, 60, 0, 0),
        Some(time(2026, 3, 1, 0, 0))
    );
    assert_eq!(
        LocalTime::from_day_of_year(2024, 366, 23, 59),
        Some(time(2024, 12, 31, 23, 59))
    );
    assert_eq!(LocalTime::from_day_of_year(2026, 366, 0, 0), None);
    assert_eq!(LocalTime::from_day_of_year(2026, 0, 0, 0), None);
    assert_eq!(LocalTime::from_day_of_year(2026, 10, 24, 0), None);
}

fn any_time() -> impl Strategy<Value = LocalTime> {
    (1900i32..2100, 1u32..=366, 0u32..24, 0u32..60)
        .prop_filter_map("day in year", |(y, d, h, m)| {
            LocalTime::from_day_of_year(y, d, h, m)
        })
}

proptest! {
    #[test]
    fn day_of_year_round_trips(t in any_time()) {
        prop_assert_eq!(LocalTime::from_day_of_year(t.year, t.day_of_year(), t.hour, t.minute), Some(t));
        prop_assert!(t.is_valid());
    }

}
