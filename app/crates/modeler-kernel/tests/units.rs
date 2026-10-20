// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Typed lengths (`docs/specs/m2-units.md`).

use modeler_kernel::units::{
    ParseError, Unit, format_length, meters_per, parse_length, parse_lengths,
};

const IN: f64 = 0.0254;
const FT: f64 = 0.3048;

fn ok(input: &str, default: Unit) -> f64 {
    parse_length(input, default).unwrap_or_else(|e| panic!("{input:?} refused: {e:?}"))
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

fn assert_len(input: &str, default: Unit, want: f64) {
    let got = ok(input, default);
    assert!(near(got, want), "{input:?}: got {got}, want {want}");
}

#[test]
fn unit_sizes() {
    assert_eq!(meters_per(Unit::Meter), 1.0);
    assert!(near(meters_per(Unit::Millimeter), 0.001));
    assert!(near(meters_per(Unit::Centimeter), 0.01));
    assert!(near(meters_per(Unit::Inch), IN));
    assert!(near(meters_per(Unit::Foot), FT));
}

#[test]
fn bare_numbers_use_the_default_unit() {
    assert_len("5", Unit::Meter, 5.0);
    assert_len("5", Unit::Millimeter, 0.005);
    assert_len("5", Unit::Centimeter, 0.05);
    assert_len("5", Unit::Inch, 5.0 * IN);
    assert_len("5", Unit::Foot, 5.0 * FT);
    assert_len("2.5", Unit::Meter, 2.5);
    assert_len(".5", Unit::Meter, 0.5);
    assert_len("007", Unit::Meter, 7.0);
    assert_len("0", Unit::Meter, 0.0);
}

#[test]
fn metric_suffixes() {
    assert_len("100mm", Unit::Inch, 0.1);
    assert_len("100 mm", Unit::Inch, 0.1);
    assert_len("2.5m", Unit::Inch, 2.5);
    assert_len("2.5 m", Unit::Millimeter, 2.5);
    assert_len("30cm", Unit::Foot, 0.3);
    assert_len("1.5CM", Unit::Foot, 0.015);
    assert_len("100MM", Unit::Foot, 0.1);
    assert_len("  3m  ", Unit::Foot, 3.0);
}

#[test]
fn inches() {
    assert_len("4\"", Unit::Meter, 4.0 * IN);
    assert_len("4 \"", Unit::Meter, 4.0 * IN);
    assert_len("4in", Unit::Meter, 4.0 * IN);
    assert_len("4 IN", Unit::Meter, 4.0 * IN);
    assert_len("4.25\"", Unit::Meter, 4.25 * IN);
    assert_len("1/2\"", Unit::Meter, 0.5 * IN);
    assert_len("4 1/2\"", Unit::Meter, 4.5 * IN);
    assert_len("4 1/2", Unit::Inch, 4.5 * IN);
    assert_len("4  3/4 in", Unit::Meter, 4.75 * IN);
    assert_len("3/8", Unit::Inch, 0.375 * IN);
    assert_len("7/4\"", Unit::Meter, 1.75 * IN);
}

#[test]
fn feet_and_inches() {
    assert_len("5'", Unit::Meter, 5.0 * FT);
    assert_len("5 '", Unit::Meter, 5.0 * FT);
    assert_len("5ft", Unit::Meter, 5.0 * FT);
    assert_len("5 FT", Unit::Meter, 5.0 * FT);
    assert_len("5.5'", Unit::Meter, 5.5 * FT);
    assert_len("5'4", Unit::Meter, 5.0 * FT + 4.0 * IN);
    assert_len("5' 4", Unit::Meter, 5.0 * FT + 4.0 * IN);
    assert_len("5'4\"", Unit::Meter, 5.0 * FT + 4.0 * IN);
    assert_len("5' 4\"", Unit::Millimeter, 5.0 * FT + 4.0 * IN);
    assert_len("5' 4in", Unit::Meter, 5.0 * FT + 4.0 * IN);
    assert_len("5ft 4in", Unit::Meter, 5.0 * FT + 4.0 * IN);
    assert_len("5ft4in", Unit::Meter, 5.0 * FT + 4.0 * IN);
    assert_len("5'4 1/2\"", Unit::Meter, 5.0 * FT + 4.5 * IN);
    assert_len("5' 4 1/2\"", Unit::Meter, 5.0 * FT + 4.5 * IN);
    assert_len("5' 1/2\"", Unit::Meter, 5.0 * FT + 0.5 * IN);
    assert_len("5' 14\"", Unit::Meter, 5.0 * FT + 14.0 * IN);
    assert_len("0' 6\"", Unit::Meter, 6.0 * IN);
}

#[test]
fn signs() {
    assert_len("-5", Unit::Meter, -5.0);
    assert_len("+5", Unit::Meter, 5.0);
    assert_len("- 5mm", Unit::Meter, -0.005);
    assert_len("-5'4\"", Unit::Meter, -(5.0 * FT + 4.0 * IN));
    assert_len("-4 1/2\"", Unit::Meter, -4.5 * IN);
}

#[test]
fn lists() {
    let v = parse_lengths("5,3", Unit::Meter).unwrap();
    assert_eq!(v.len(), 2);
    assert!(near(v[0], 5.0) && near(v[1], 3.0));
    let v = parse_lengths(" 2m , 30 cm,4\" ", Unit::Millimeter).unwrap();
    assert_eq!(v.len(), 3);
    assert!(near(v[0], 2.0) && near(v[1], 0.3) && near(v[2], 4.0 * IN));
    let v = parse_lengths("7", Unit::Foot).unwrap();
    assert_eq!(v.len(), 1);
    assert!(near(v[0], 7.0 * FT));
    assert_eq!(parse_lengths("5,", Unit::Meter), Err(ParseError::Empty));
    assert_eq!(parse_lengths("5,x", Unit::Meter), Err(ParseError::Invalid));
    assert_eq!(parse_lengths("", Unit::Meter), Err(ParseError::Empty));
    assert_eq!(parse_lengths(",", Unit::Meter), Err(ParseError::Empty));
}

#[test]
fn empty_input() {
    for s in ["", " ", "\t\n  "] {
        assert_eq!(
            parse_length(s, Unit::Meter),
            Err(ParseError::Empty),
            "{s:?}"
        );
    }
}

#[test]
fn refused_input() {
    for s in [
        "m",
        "mm",
        "'",
        "\"",
        "-",
        "+",
        "--5",
        "5 5",
        "5 m m",
        "5mm 3",
        "5m4",
        "5cm 2\"",
        "5\"4",
        "5'4'",
        "5' 4m",
        "5' 4 mm",
        "5''",
        "5\"\"",
        "5 6/",
        "5 /6",
        "1/0",
        "1/0\"",
        "1.5/2",
        "1 1.5/2",
        "1/2/3",
        "5.",
        "5..5",
        "1e3",
        "1,5",
        "abc",
        "5 feet",
        "5x",
        "5 4/",
        "\u{661}",
        "５",
        "5\u{a0}m",
        "5 m\u{301}",
        "0x10",
        "inf",
        "NaN",
        "5'.",
    ] {
        assert_eq!(
            parse_length(s, Unit::Meter),
            Err(ParseError::Invalid),
            "{s:?}"
        );
    }
}

#[test]
fn huge_numbers_are_not_lengths() {
    let big = "9".repeat(400);
    assert_eq!(parse_length(&big, Unit::Meter), Err(ParseError::Invalid));
    assert_eq!(
        parse_length(&format!("{big}'"), Unit::Meter),
        Err(ParseError::Invalid)
    );
    assert!(ok("123456789012", Unit::Meter) > 1e11);
}

#[test]
fn metric_formatting() {
    assert_eq!(format_length(2.5, Unit::Meter, 3), "2.5m");
    assert_eq!(format_length(2.0, Unit::Meter, 3), "2m");
    assert_eq!(format_length(0.1234567, Unit::Meter, 3), "0.123m");
    assert_eq!(format_length(0.1236, Unit::Meter, 3), "0.124m");
    assert_eq!(format_length(0.1, Unit::Millimeter, 0), "100mm");
    assert_eq!(format_length(0.1005, Unit::Millimeter, 1), "100.5mm");
    assert_eq!(format_length(0.3, Unit::Centimeter, 2), "30cm");
    assert_eq!(format_length(0.0, Unit::Meter, 3), "0m");
    assert_eq!(format_length(-1.25, Unit::Meter, 3), "-1.25m");
    assert_eq!(format_length(-0.0001, Unit::Meter, 2), "0m");
    assert_eq!(format_length(-0.0, Unit::Millimeter, 2), "0mm");
    assert_eq!(format_length(1234.5, Unit::Meter, 0), "1235m");
}

#[test]
fn decimals_above_six_count_as_six() {
    assert_eq!(
        format_length(1.0 / 3.0, Unit::Meter, 9),
        format_length(1.0 / 3.0, Unit::Meter, 6)
    );
    assert_eq!(format_length(1.0 / 3.0, Unit::Meter, 6), "0.333333m");
}

#[test]
fn inch_and_foot_formatting() {
    assert_eq!(format_length(4.0 * IN, Unit::Inch, 3), "4\"");
    assert_eq!(format_length(4.5 * IN, Unit::Inch, 3), "4.5\"");
    assert_eq!(format_length(40.0 * IN, Unit::Inch, 3), "40\"");
    assert_eq!(format_length(0.0, Unit::Inch, 3), "0\"");
    assert_eq!(format_length(-4.5 * IN, Unit::Inch, 3), "-4.5\"");
    assert_eq!(format_length(5.0 * FT, Unit::Foot, 3), "5'");
    assert_eq!(format_length(5.0 * FT + 4.0 * IN, Unit::Foot, 3), "5' 4\"");
    assert_eq!(
        format_length(5.0 * FT + 4.5 * IN, Unit::Foot, 3),
        "5' 4.5\""
    );
    assert_eq!(format_length(4.5 * IN, Unit::Foot, 3), "4.5\"");
    assert_eq!(format_length(0.0, Unit::Foot, 3), "0\"");
    assert_eq!(
        format_length(-(2.0 * FT + 1.0 * IN), Unit::Foot, 3),
        "-2' 1\""
    );
    // Inches that round up to a whole foot carry into the feet.
    assert_eq!(format_length(2.0 * FT - 0.0001 * IN, Unit::Foot, 2), "2'");
    assert_eq!(format_length(2.0 * FT - 0.004 * IN, Unit::Foot, 2), "2'");
    assert_eq!(
        format_length(2.0 * FT - 0.04 * IN, Unit::Foot, 2),
        "1' 11.96\""
    );
    // Too small to show: no sign.
    assert_eq!(format_length(-0.0001 * IN, Unit::Foot, 2), "0\"");
    assert_eq!(format_length(-0.0001 * IN, Unit::Inch, 2), "0\"");
}

#[test]
fn odd_values_do_not_panic() {
    for v in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1e300,
        -1e300,
        f64::MAX,
        f64::MIN,
    ] {
        for u in [
            Unit::Millimeter,
            Unit::Centimeter,
            Unit::Meter,
            Unit::Inch,
            Unit::Foot,
        ] {
            let _ = format_length(v, u, 3);
        }
    }
}

#[test]
fn never_panics_on_junk() {
    for s in [
        "\0",
        "\u{1f600}",
        "5\u{1f600}",
        "'''",
        "\"\"\"",
        "///",
        "1/",
        "/1",
        "  -  ",
        "5'  ",
        "5' \"",
        "5' 4 1/",
        "99999999999999999999/1",
        "1/99999999999999999999999",
    ] {
        let _ = parse_length(s, Unit::Meter);
        let _ = parse_lengths(s, Unit::Meter);
    }
}
