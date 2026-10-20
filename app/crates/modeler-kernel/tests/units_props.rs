// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Typed length round trips (`docs/specs/m2-units.md`).

use modeler_kernel::units::{Unit, format_length, meters_per, parse_length, parse_lengths};
use proptest::prelude::*;

const UNITS: [Unit; 5] = [
    Unit::Millimeter,
    Unit::Centimeter,
    Unit::Meter,
    Unit::Inch,
    Unit::Foot,
];

/// Half a unit in the last place written, in meters.
fn tolerance(unit: Unit, decimals: u32) -> f64 {
    let last = match unit {
        Unit::Foot | Unit::Inch => meters_per(Unit::Inch),
        other => meters_per(other),
    };
    0.5 * last / 10f64.powi(decimals as i32) * 1.0001
}

proptest! {
    #[test]
    fn written_lengths_read_back(
        meters in -5000.0f64..5000.0,
        unit in 0usize..5,
        decimals in 0u32..5,
        default in 0usize..5,
    ) {
        let unit = UNITS[unit];
        let text = format_length(meters, unit, decimals);
        let back = parse_length(&text, UNITS[default]).unwrap();
        prop_assert!(
            (back - meters).abs() <= tolerance(unit, decimals),
            "{meters} -> {text:?} -> {back}"
        );
    }

    #[test]
    fn writing_a_read_length_is_stable(
        meters in -5000.0f64..5000.0,
        unit in 0usize..5,
        decimals in 0u32..5,
    ) {
        let unit = UNITS[unit];
        let once = format_length(meters, unit, decimals);
        let back = parse_length(&once, Unit::Meter).unwrap();
        prop_assert_eq!(&format_length(back, unit, decimals), &once);
    }

    #[test]
    fn feet_and_inches_add_up(
        feet in 0u32..2000,
        inches in 0u32..12,
        sixteenths in 0u32..16,
        spaced in any::<bool>(),
        quoted in any::<bool>(),
    ) {
        let frac = if sixteenths == 0 { String::new() } else { format!(" {sixteenths}/16") };
        let gap = if spaced { " " } else { "" };
        let quote = if quoted { "\"" } else { "" };
        let text = format!("{feet}'{gap}{inches}{frac}{quote}");
        let want = f64::from(feet) * 0.3048
            + (f64::from(inches) + f64::from(sixteenths) / 16.0) * 0.0254;
        let got = parse_length(&text, Unit::Meter).unwrap();
        prop_assert!((got - want).abs() < 1e-9, "{text:?}: {got} vs {want}");
        let neg = parse_length(&format!("-{text}"), Unit::Meter).unwrap();
        prop_assert!((neg + want).abs() < 1e-9);
    }

    #[test]
    fn lists_read_each_part(a in 0.0f64..1000.0, b in 0.0f64..1000.0, u in 0usize..5) {
        let unit = UNITS[u];
        let text = format!("{}, {}", format_length(a, unit, 3), format_length(b, unit, 3));
        let v = parse_lengths(&text, Unit::Meter).unwrap();
        prop_assert_eq!(v.len(), 2);
        prop_assert!((v[0] - a).abs() <= tolerance(unit, 3));
        prop_assert!((v[1] - b).abs() <= tolerance(unit, 3));
    }

    #[test]
    fn junk_never_panics(s in "\\PC{0,24}") {
        let _ = parse_length(&s, Unit::Meter);
        let _ = parse_lengths(&s, Unit::Foot);
    }

    #[test]
    fn near_misses_never_panic(s in "[0-9 ./'\"a-z,+-]{0,16}") {
        let _ = parse_length(&s, Unit::Inch);
        let _ = parse_lengths(&s, Unit::Millimeter);
    }
}
