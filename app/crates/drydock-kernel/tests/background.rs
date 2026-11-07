// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-background-kernel: the model's background, sky and ground settings, set as one
//! undoable record and saved as the `background` record of native format version 8
//! (`docs/specs/m10-background-kernel.md`).

use drydock_kernel::background::*;
use drydock_kernel::format::*;
use drydock_kernel::model::*;
use drydock_kernel::units::LengthFormat;
use proptest::prelude::*;

fn rgb8(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

/// Every field away from its default, with values that print short.
fn custom() -> Background {
    Background {
        color: [0.5, 0.5, 0.5],
        sky: false,
        sky_color: [0.0, 0.25, 1.0],
        ground: true,
        ground_color: [1.0, 0.75, 0.0],
    }
}
const CUSTOM_LINE: &str = "background 0.5 0.5 0.5 off 0 0.25 1 on 1 0.75 0";

fn with_background(background: Background) -> Model {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    m.set_background(background, &mut log).unwrap();
    m
}

fn read_err(text: &str) -> ReadError {
    match from_text(text) {
        Ok(_) => panic!("expected an error for {text:?}"),
        Err(e) => e,
    }
}

fn err_at(text: &str, line: usize, kind: ReadErrorKind) {
    assert_eq!(read_err(text), ReadError { line, kind }, "{text:?}");
}

// ---- the value ----

#[test]
fn the_default_colors() {
    assert_eq!(DEFAULT_BACKGROUND, rgb8(0xF4, 0xF8, 0xFC));
    assert_eq!(DEFAULT_SKY, rgb8(0xA8, 0xC8, 0xE8));
    assert_eq!(DEFAULT_GROUND, rgb8(0x9A, 0x9A, 0x96));
}

#[test]
fn the_default_has_sky_and_ground_on() {
    assert_eq!(
        Background::default(),
        Background {
            color: DEFAULT_BACKGROUND,
            sky: true,
            sky_color: DEFAULT_SKY,
            ground: true,
            ground_color: DEFAULT_GROUND,
        }
    );
    assert!(Background::default().is_valid());
}

#[test]
fn channels_from_zero_to_one_are_valid() {
    let mut b = custom();
    assert!(b.is_valid());
    b.color = [0.0, 0.0, 0.0];
    b.sky_color = [1.0, 1.0, 1.0];
    b.ground_color = [-0.0, 0.0, 1.0];
    assert!(b.is_valid());
    b.sky = true;
    b.ground = false;
    assert!(b.is_valid());
}

/// Each of the nine channels, set to `value`.
fn with_channel(index: usize, value: f32) -> Background {
    let mut b = Background::default();
    match index / 3 {
        0 => b.color[index % 3] = value,
        1 => b.sky_color[index % 3] = value,
        _ => b.ground_color[index % 3] = value,
    }
    b
}

#[test]
fn a_channel_out_of_range_or_not_finite_is_invalid() {
    for index in 0..9 {
        for value in [
            -0.01,
            1.01,
            -1.0,
            2.0,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ] {
            assert!(
                !with_channel(index, value).is_valid(),
                "channel {index} = {value}"
            );
        }
    }
}

#[test]
fn a_color_that_is_not_drawn_must_still_be_valid() {
    let b = Background {
        sky: false,
        sky_color: [1.5, 0.0, 0.0],
        ..Background::default()
    };
    assert!(!b.is_valid());
    let b = Background {
        ground: false,
        ground_color: [0.0, f32::NAN, 0.0],
        ..Background::default()
    };
    assert!(!b.is_valid());
}

// ---- the model setting ----

#[test]
fn a_new_model_has_the_default_background() {
    assert_eq!(Model::new().background(), Background::default());
}

#[test]
fn setting_the_background_is_one_record_that_undoes_and_redoes() {
    let mut m = Model::new();
    let revision = m.revision();
    let mut log = ModelEdit::new();
    m.set_background(custom(), &mut log).unwrap();
    assert_eq!(m.background(), custom());
    assert_eq!(
        log.records,
        vec![ModelRecord::SetBackground {
            from: Background::default(),
            to: custom(),
        }]
    );
    assert_eq!(m.revision(), revision, "geometry caches stay valid");
    m.apply_edit(&log.inverse()).unwrap();
    assert_eq!(m.background(), Background::default());
    assert_eq!(m.revision(), revision);
    m.apply_edit(&log).unwrap();
    assert_eq!(m.background(), custom());
    assert_eq!(m.revision(), revision);
}

#[test]
fn the_inverse_swaps_from_and_to() {
    let record = ModelRecord::SetBackground {
        from: Background::default(),
        to: custom(),
    };
    assert_eq!(
        record.inverse(),
        ModelRecord::SetBackground {
            from: custom(),
            to: Background::default(),
        }
    );
}

#[test]
fn turning_only_the_sky_off_is_a_change() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let b = Background {
        sky: false,
        ..Background::default()
    };
    m.set_background(b, &mut log).unwrap();
    assert_eq!(m.background(), b);
    assert_eq!(log.records.len(), 1);
}

#[test]
fn setting_the_same_background_records_nothing() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    m.set_background(Background::default(), &mut log).unwrap();
    assert!(log.records.is_empty());
    m.set_background(custom(), &mut log).unwrap();
    m.set_background(custom(), &mut log).unwrap();
    assert_eq!(log.records.len(), 1);
}

#[test]
fn an_invalid_background_is_refused() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    for index in [0, 4, 8] {
        assert_eq!(
            m.set_background(with_channel(index, 1.5), &mut log),
            Err(ModelError::BadSettings)
        );
    }
    assert_eq!(
        m.set_background(with_channel(2, f32::NAN), &mut log),
        Err(ModelError::BadSettings)
    );
    assert!(log.records.is_empty());
    assert_eq!(m.background(), Background::default());

    let bad = ModelRecord::SetBackground {
        from: Background::default(),
        to: with_channel(3, -1.0),
    };
    assert_eq!(m.apply(&bad), Err(ModelError::BadSettings));
    let stale = ModelRecord::SetBackground {
        from: custom(),
        to: Background::default(),
    };
    assert_eq!(m.apply(&stale), Err(ModelError::Mismatch));
    assert_eq!(m.background(), Background::default());
}

#[test]
fn models_differing_only_in_background_are_not_equal() {
    let a = Model::new();
    let b = with_background(custom());
    assert_ne!(a, b);
    assert_eq!(b.clone(), b);
}

// ---- the native file ----

#[test]
fn the_format_version_is_eight() {
    assert_eq!(FORMAT_VERSION, 8);
}

#[test]
fn the_default_background_keeps_older_versions() {
    assert_eq!(
        to_text(&Model::new()).unwrap(),
        "drydock-model 1\nbody root\nend\n"
    );
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    m.add_tag("Walls", &mut log).unwrap();
    assert!(to_text(&m).unwrap().starts_with("drydock-model 7\n"));
    // set and set back: nothing to write
    m.set_background(custom(), &mut log).unwrap();
    m.set_background(Background::default(), &mut log).unwrap();
    assert!(to_text(&m).unwrap().starts_with("drydock-model 7\n"));
}

#[test]
fn a_changed_background_is_written_in_version_eight() {
    let m = with_background(custom());
    assert_eq!(
        to_text(&m).unwrap(),
        format!("drydock-model 8\n{CUSTOM_LINE}\nbody root\nend\n")
    );
}

#[test]
fn the_background_comes_after_the_other_settings() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    m.set_length_format(LengthFormat::Architectural { denominator: 8 }, &mut log)
        .unwrap();
    m.set_background(custom(), &mut log).unwrap();
    m.add_tag("Walls", &mut log).unwrap();
    assert_eq!(
        to_text(&m).unwrap(),
        format!(
            "drydock-model 8\nunits architectural 8\n{CUSTOM_LINE}\ntag 0 on \"Walls\"\nbody root\nend\n"
        )
    );
}

#[test]
fn round_trip_keeps_every_bit() {
    for b in [
        custom(),
        Background {
            sky: false,
            ..Background::default()
        },
        Background {
            ground: false,
            ..Background::default()
        },
        Background {
            color: rgb8(1, 2, 3),
            sky_color: [0.1, 0.2, 0.3],
            ground_color: [1.0 / 3.0, 2.0 / 3.0, 0.999_999_9],
            ..Background::default()
        },
    ] {
        let m = with_background(b);
        let text = to_text(&m).unwrap();
        assert!(text.starts_with("drydock-model 8\n"), "{text}");
        let back = from_text(&text).unwrap();
        assert_eq!(back.background(), b);
        assert_eq!(back, m);
        assert_eq!(to_text(&back).unwrap(), text);
    }
}

#[test]
fn save_and_load_keep_the_background() {
    let dir = std::env::temp_dir().join(format!(
        "drydock-background-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bg.drydock");
    let m = with_background(custom());
    save(&m, &path).unwrap();
    let back = load(&path).unwrap();
    assert_eq!(back.background(), custom());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn files_without_the_record_read_as_the_default() {
    for text in [
        "drydock-model 1\nbody root\nend\n",
        "drydock-model 7\ntag 0 on \"Walls\"\nbody root\nend\n",
        "drydock-model 8\nbody root\nend\n",
    ] {
        assert_eq!(from_text(text).unwrap().background(), Background::default());
    }
}

#[test]
fn the_record_may_stand_anywhere_before_end() {
    let text = format!("drydock-model 8\nbody root\nvertex 0 1 2 3\n{CUSTOM_LINE}\nend\n");
    assert_eq!(from_text(&text).unwrap().background(), custom());
}

#[test]
fn versions_one_to_seven_have_no_background_record() {
    for v in 1..=7 {
        err_at(
            &format!("drydock-model {v}\n{CUSTOM_LINE}\nbody root\nend\n"),
            2,
            ReadErrorKind::UnknownRecord,
        );
    }
    err_at(
        "drydock-model 9\nbody root\nend\n",
        1,
        ReadErrorKind::UnsupportedVersion(9),
    );
}

#[test]
fn malformed_background_records_are_bad_fields() {
    for line in [
        "background",
        "background 0.5 0.5 0.5 off 0 0.25 1 on 1 0.75",
        "background 0.5 0.5 0.5 off 0 0.25 1 on 1 0.75 0 0",
        "background 0.5 0.5 0.5 yes 0 0.25 1 on 1 0.75 0",
        "background 0.5 0.5 0.5 off 0 0.25 1 1 1 0.75 0",
        "background 0.5 0.5 grey off 0 0.25 1 on 1 0.75 0",
        "background 0.5 0.5 0.5 off 0 NaN 1 on 1 0.75 0",
        "background 0.5 0.5 0.5 off 0 0.25 1 on inf 0.75 0",
        "background 0.5 0.5 0.5 \"off\" 0 0.25 1 on 1 0.75 0",
        "background on 0.5 0.5 0.5 0 0.25 1 on 1 0.75 0",
    ] {
        err_at(
            &format!("drydock-model 8\n{line}\nbody root\nend\n"),
            2,
            ReadErrorKind::BadField,
        );
    }
}

#[test]
fn out_of_range_colors_are_rejected() {
    for line in [
        "background 1.5 0.5 0.5 off 0 0.25 1 on 1 0.75 0",
        "background 0.5 0.5 0.5 off 0 -0.25 1 on 1 0.75 0",
        "background 0.5 0.5 0.5 off 0 0.25 1 on 1 0.75 2",
    ] {
        err_at(
            &format!("drydock-model 8\n{line}\nbody root\nend\n"),
            2,
            ReadErrorKind::Rejected(ModelError::BadSettings),
        );
    }
}

#[test]
fn a_second_background_record_is_a_bad_field() {
    err_at(
        &format!("drydock-model 8\n{CUSTOM_LINE}\n{CUSTOM_LINE}\nbody root\nend\n"),
        3,
        ReadErrorKind::BadField,
    );
    let default_line = "background 0.95686275 0.972549 0.9882353 on 0.65882355 0.78431374 0.9098039 on 0.6039216 0.6039216 0.5882353";
    err_at(
        &format!("drydock-model 8\n{default_line}\n{CUSTOM_LINE}\nbody root\nend\n"),
        3,
        ReadErrorKind::BadField,
    );
}

// ---- properties ----

fn channel() -> impl Strategy<Value = f32> {
    prop_oneof![Just(0.0f32), Just(1.0f32), 0.0f32..=1.0]
}

fn color() -> impl Strategy<Value = [f32; 3]> {
    [channel(), channel(), channel()]
}

fn any_background() -> impl Strategy<Value = Background> {
    (color(), any::<bool>(), color(), any::<bool>(), color()).prop_map(
        |(color, sky, sky_color, ground, ground_color)| Background {
            color,
            sky,
            sky_color,
            ground,
            ground_color,
        },
    )
}

proptest! {
    #[test]
    fn any_valid_background_round_trips(b in any_background()) {
        prop_assert!(b.is_valid());
        let m = with_background(b);
        let text = to_text(&m).unwrap();
        let expected_version = if b == Background::default() { 1 } else { 8 };
        let header = format!("drydock-model {expected_version}\n");
        prop_assert!(text.starts_with(&header), "{}", text);
        let back = from_text(&text).unwrap();
        prop_assert_eq!(back.background(), b);
        prop_assert_eq!(back, m);
    }

    #[test]
    fn set_then_undo_restores_the_model(a in any_background(), b in any_background()) {
        let mut m = with_background(a);
        let before = m.clone();
        let mut log = ModelEdit::new();
        m.set_background(b, &mut log).unwrap();
        prop_assert_eq!(log.records.len(), usize::from(a != b));
        m.apply_edit(&log.inverse()).unwrap();
        prop_assert_eq!(m, before);
    }
}
