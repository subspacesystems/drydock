// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-units: the model's length format, how lengths and areas are written in it, and the
//! `units` record of the native file (`docs/specs/m6-units.md`).

use drydock_kernel::format::*;
use drydock_kernel::model::*;
use drydock_kernel::sun::Geolocation;
use drydock_kernel::units::*;
use proptest::prelude::*;

const IN: f64 = 0.0254;
const FT: f64 = 0.3048;

fn dec(unit: Unit, decimals: u8) -> LengthFormat {
    LengthFormat::Decimal { unit, decimals }
}

fn arch(denominator: u8) -> LengthFormat {
    LengthFormat::Architectural { denominator }
}

// ---- the format itself ----

#[test]
fn the_default_is_inches_to_two_places() {
    assert_eq!(LengthFormat::default(), dec(Unit::Inch, 2));
}

#[test]
fn validity() {
    for d in 0..=LengthFormat::MAX_DECIMALS {
        for unit in [
            Unit::Millimeter,
            Unit::Centimeter,
            Unit::Meter,
            Unit::Inch,
            Unit::Foot,
        ] {
            assert!(dec(unit, d).is_valid());
        }
    }
    assert!(!dec(Unit::Meter, LengthFormat::MAX_DECIMALS + 1).is_valid());
    for d in LengthFormat::DENOMINATORS {
        assert!(arch(d).is_valid());
    }
    for d in [0, 3, 10, 128] {
        assert!(!arch(d).is_valid(), "1/{d}");
    }
}

#[test]
fn bare_numbers_are_read_in_the_decimal_unit_or_inches() {
    assert_eq!(dec(Unit::Millimeter, 0).input_unit(), Unit::Millimeter);
    assert_eq!(dec(Unit::Foot, 2).input_unit(), Unit::Foot);
    assert_eq!(arch(16).input_unit(), Unit::Inch);
}

// ---- decimal lengths ----

#[test]
fn decimal_lengths_in_each_unit() {
    assert_eq!(format_with(2.5, dec(Unit::Meter, 2)), "2.5m");
    assert_eq!(format_with(2.5, dec(Unit::Millimeter, 0)), "2500mm");
    assert_eq!(format_with(0.0125, dec(Unit::Centimeter, 2)), "1.25cm");
    assert_eq!(format_with(4.5 * IN, dec(Unit::Inch, 2)), "4.5\"");
    assert_eq!(format_with(5.25 * FT, dec(Unit::Foot, 2)), "5.25'");
    assert_eq!(format_with(5.0 * FT, dec(Unit::Foot, 2)), "5'");
}

#[test]
fn decimal_lengths_round_and_drop_trailing_zeros() {
    assert_eq!(format_with(1.23456, dec(Unit::Meter, 3)), "1.235m");
    assert_eq!(format_with(1.2, dec(Unit::Meter, 3)), "1.2m");
    assert_eq!(format_with(1234.5, dec(Unit::Meter, 0)), "1235m");
    assert_eq!(format_with(1.0, dec(Unit::Foot, 4)), "3.2808'");
    assert_eq!(format_with(1.0 / 3.0, dec(Unit::Foot, 0)), "1'");
    assert_eq!(format_with(0.0, dec(Unit::Foot, 2)), "0'");
}

#[test]
fn decimal_negatives_and_rounded_zero() {
    assert_eq!(format_with(-1.25, dec(Unit::Meter, 2)), "-1.25m");
    assert_eq!(format_with(-0.001, dec(Unit::Meter, 2)), "0m");
    assert_eq!(format_with(-0.0, dec(Unit::Millimeter, 1)), "0mm");
    assert_eq!(format_with(-2.5 * FT, dec(Unit::Foot, 1)), "-2.5'");
}

#[test]
fn decimals_match_the_old_writer_for_metric_and_inches() {
    for unit in [Unit::Millimeter, Unit::Centimeter, Unit::Meter, Unit::Inch] {
        for d in 0..=LengthFormat::MAX_DECIMALS {
            for m in [0.0, 0.1234567, 1.0, -3.75, 12.0 * IN, 1000.0 / 7.0] {
                assert_eq!(
                    format_with(m, dec(unit, d)),
                    format_length(m, unit, u32::from(d)),
                    "{m} in {unit:?} to {d}"
                );
            }
        }
    }
}

// ---- architectural lengths ----

#[test]
fn architectural_feet_inches_and_fractions() {
    assert_eq!(format_with(5.0 * FT + 4.5 * IN, arch(16)), "5' 4 1/2\"");
    assert_eq!(format_with(5.0 * FT + 4.0 * IN, arch(16)), "5' 4\"");
    assert_eq!(format_with(5.0 * FT, arch(16)), "5' 0\"");
    assert_eq!(format_with(4.5 * IN, arch(16)), "4 1/2\"");
    assert_eq!(format_with(3.0 / 16.0 * IN, arch(16)), "3/16\"");
    assert_eq!(format_with(0.0, arch(16)), "0\"");
    assert_eq!(format_with(2.0 * FT + 11.0 * IN, arch(8)), "2' 11\"");
}

#[test]
fn architectural_fractions_are_in_lowest_terms() {
    assert_eq!(format_with(6.0 / 16.0 * IN, arch(16)), "3/8\"");
    assert_eq!(format_with(8.0 / 64.0 * IN, arch(64)), "1/8\"");
    assert_eq!(format_with(1.0 / 64.0 * IN, arch(64)), "1/64\"");
    assert_eq!(format_with(12.75 * IN, arch(32)), "1' 0 3/4\"");
}

#[test]
fn architectural_rounds_to_the_chosen_fraction_and_carries() {
    // 0.3 in is nearest 1/4 at 1/4, nearest 5/16 at 1/16.
    assert_eq!(format_with(0.3 * IN, arch(4)), "1/4\"");
    assert_eq!(format_with(0.3 * IN, arch(16)), "5/16\"");
    // Whole inches.
    assert_eq!(format_with(4.6 * IN, arch(1)), "5\"");
    assert_eq!(format_with(4.4 * IN, arch(1)), "4\"");
    // 11.99 in rounds up to a whole foot.
    assert_eq!(format_with(11.99 * IN, arch(16)), "1' 0\"");
    assert_eq!(format_with(3.0 * FT + 11.999 * IN, arch(4)), "4' 0\"");
    // 15/16 rounds up to the next inch.
    assert_eq!(format_with(2.97 * IN, arch(16)), "3\"");
}

#[test]
fn architectural_negatives_and_rounded_zero() {
    assert_eq!(format_with(-(2.0 * FT + 1.0 * IN), arch(16)), "-2' 1\"");
    assert_eq!(format_with(-0.5 * IN, arch(16)), "-1/2\"");
    assert_eq!(format_with(-0.001 * IN, arch(16)), "0\"");
}

#[test]
fn non_finite_lengths_read_as_zero() {
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(format_with(v, arch(16)), "0\"");
        assert_eq!(format_with(v, dec(Unit::Meter, 2)), "0m");
        assert_eq!(format_with(v, dec(Unit::Foot, 2)), "0'");
    }
}

// ---- areas ----

#[test]
fn areas_in_the_square_of_the_unit() {
    assert_eq!(format_area_with(1.5, dec(Unit::Meter, 2)), "1.5m²");
    assert_eq!(format_area_with(0.123456, dec(Unit::Meter, 3)), "0.123m²");
    assert_eq!(format_area_with(1.0, dec(Unit::Centimeter, 0)), "10000cm²");
    assert_eq!(format_area_with(1e-6, dec(Unit::Millimeter, 1)), "1mm²");
    assert_eq!(
        format_area_with(IN * IN * 12.5, dec(Unit::Inch, 2)),
        "12.5in²"
    );
    assert_eq!(format_area_with(FT * FT * 3.0, dec(Unit::Foot, 2)), "3ft²");
    assert_eq!(format_area_with(0.0, dec(Unit::Meter, 2)), "0m²");
    assert_eq!(format_area_with(f64::NAN, dec(Unit::Inch, 2)), "0in²");
}

#[test]
fn architectural_areas_are_square_feet_to_two_places() {
    assert_eq!(format_area_with(FT * FT * 20.0, arch(16)), "20ft²");
    assert_eq!(format_area_with(FT * FT * 1.0 / 3.0, arch(64)), "0.33ft²");
}

// ---- typed back ----

fn step(format: LengthFormat) -> f64 {
    match format {
        LengthFormat::Decimal { unit, decimals } => {
            meters_per(unit) / 10f64.powi(i32::from(decimals))
        }
        LengthFormat::Architectural { denominator } => IN / f64::from(denominator),
    }
}

fn any_format() -> impl Strategy<Value = LengthFormat> {
    prop_oneof![
        (0usize..5, 0u8..=LengthFormat::MAX_DECIMALS).prop_map(|(u, d)| {
            let unit = [
                Unit::Millimeter,
                Unit::Centimeter,
                Unit::Meter,
                Unit::Inch,
                Unit::Foot,
            ][u];
            dec(unit, d)
        }),
        (0usize..7).prop_map(|i| arch(LengthFormat::DENOMINATORS[i])),
    ]
}

proptest! {
    /// What is written reads back (as typed into the measurement box, a bare number in the
    /// format's input unit) to within half a rounding step.
    #[test]
    fn written_lengths_read_back(meters in -500.0f64..500.0, format in any_format()) {
        let text = format_with(meters, format);
        let back = parse_length(&text, format.input_unit())
            .unwrap_or_else(|e| panic!("{text:?} does not parse: {e:?}"));
        prop_assert!(
            (back - meters).abs() <= step(format) / 2.0 + 1e-9,
            "{meters} -> {text:?} -> {back}"
        );
    }

    /// Writing what was read gives the same text.
    #[test]
    fn writing_is_stable(meters in -500.0f64..500.0, format in any_format()) {
        let once = format_with(meters, format);
        let back = parse_length(&once, format.input_unit()).unwrap();
        prop_assert_eq!(format_with(back, format), once);
    }
}

// ---- the model setting ----

#[test]
fn a_new_model_has_the_default_format() {
    assert_eq!(Model::new().length_format(), LengthFormat::default());
}

#[test]
fn setting_the_format_is_one_record_that_undoes() {
    let mut m = Model::new();
    let revision = m.revision();
    let mut log = ModelEdit::new();
    m.set_length_format(arch(16), &mut log).unwrap();
    assert_eq!(m.length_format(), arch(16));
    assert_eq!(log.records.len(), 1);
    assert_eq!(
        log.records[0],
        ModelRecord::SetLengthFormat {
            from: LengthFormat::default(),
            to: arch(16)
        }
    );
    assert_eq!(m.revision(), revision, "geometry caches stay valid");
    m.apply_edit(&log.inverse()).unwrap();
    assert_eq!(m.length_format(), LengthFormat::default());
}

#[test]
fn setting_the_same_format_records_nothing() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    m.set_length_format(LengthFormat::default(), &mut log)
        .unwrap();
    assert!(log.records.is_empty());
}

#[test]
fn an_invalid_format_is_refused() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    assert_eq!(
        m.set_length_format(arch(3), &mut log),
        Err(ModelError::BadSettings)
    );
    assert_eq!(
        m.set_length_format(dec(Unit::Meter, 7), &mut log),
        Err(ModelError::BadSettings)
    );
    assert!(log.records.is_empty());
    assert_eq!(m.length_format(), LengthFormat::default());
    let bad = ModelRecord::SetLengthFormat {
        from: LengthFormat::default(),
        to: arch(0),
    };
    assert_eq!(m.apply(&bad), Err(ModelError::BadSettings));
    let stale = ModelRecord::SetLengthFormat {
        from: arch(16),
        to: arch(8),
    };
    assert_eq!(m.apply(&stale), Err(ModelError::Mismatch));
}

// ---- the native file ----

#[test]
fn the_version_holds_units() {
    // version 5 added units; m7-curves made it 6 (tests/curves_format.rs)
    const { assert!(FORMAT_VERSION >= 5) };
}

#[test]
fn the_default_format_keeps_older_versions() {
    let text = to_text(&Model::new()).unwrap();
    assert_eq!(text, "drydock-model 1\nbody root\nend\n");
}

#[test]
fn a_changed_format_is_written_after_the_other_settings() {
    let cases = [
        (dec(Unit::Millimeter, 1), "units decimal mm 1"),
        (dec(Unit::Centimeter, 0), "units decimal cm 0"),
        (dec(Unit::Meter, 3), "units decimal m 3"),
        (dec(Unit::Inch, 4), "units decimal in 4"),
        (dec(Unit::Foot, 2), "units decimal ft 2"),
        (arch(16), "units architectural 16"),
        (arch(1), "units architectural 1"),
    ];
    for (format, line) in cases {
        let mut m = Model::new();
        let mut log = ModelEdit::new();
        m.set_length_format(format, &mut log).unwrap();
        assert_eq!(
            to_text(&m).unwrap(),
            format!("drydock-model 5\n{line}\nbody root\nend\n")
        );
        let back = from_text(&to_text(&m).unwrap()).unwrap();
        assert_eq!(back.length_format(), format);
        assert_eq!(back, m);
    }
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    m.set_geolocation(
        Geolocation {
            north_angle: 90.0,
            ..Geolocation::default()
        },
        &mut log,
    )
    .unwrap();
    m.set_length_format(arch(8), &mut log).unwrap();
    assert_eq!(
        to_text(&m).unwrap(),
        "drydock-model 5\ngeolocation 45 0 90 0\nunits architectural 8\nbody root\nend\n"
    );
}

#[test]
fn a_file_without_units_reads_as_the_default() {
    let m = from_text("drydock-model 4\ngeolocation 45 0 90 0\nbody root\nend\n").unwrap();
    assert_eq!(m.length_format(), LengthFormat::default());
}

fn read_err(text: &str) -> ReadError {
    match from_text(text) {
        Ok(_) => panic!("expected an error for {text:?}"),
        Err(e) => e,
    }
}

#[test]
fn bad_units_records_are_refused() {
    let unknown = "drydock-model 4\nunits decimal m 2\nbody root\nend\n";
    assert_eq!(
        read_err(unknown),
        ReadError {
            line: 2,
            kind: ReadErrorKind::UnknownRecord
        },
        "a version 4 file has no units record"
    );
    for line in [
        "units decimal m",
        "units decimal yd 2",
        "units decimal m two",
        "units decimal m -1",
        "units architectural",
        "units architectural 16 2",
        "units metric 2",
        "units",
    ] {
        let text = format!("drydock-model 5\n{line}\nbody root\nend\n");
        assert_eq!(
            read_err(&text),
            ReadError {
                line: 2,
                kind: ReadErrorKind::BadField
            },
            "{line}"
        );
    }
    for line in ["units decimal m 7", "units architectural 3"] {
        let text = format!("drydock-model 5\n{line}\nbody root\nend\n");
        assert_eq!(
            read_err(&text),
            ReadError {
                line: 2,
                kind: ReadErrorKind::Rejected(ModelError::BadSettings)
            },
            "{line}"
        );
    }
    let twice = "drydock-model 5\nunits decimal m 2\nunits decimal m 2\nbody root\nend\n";
    assert_eq!(
        read_err(twice),
        ReadError {
            line: 3,
            kind: ReadErrorKind::BadField
        }
    );
}
