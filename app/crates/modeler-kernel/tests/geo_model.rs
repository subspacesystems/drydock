// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m5-geo-model.md`: geolocation and shadow settings stored
//! in the model, undoable, and in the native file (`docs/format.md` version 4).

use glam::DVec3;
use modeler_kernel::format::*;
use modeler_kernel::model::*;
use modeler_kernel::sun::{Geolocation, LocalTime, ShadowSettings};

fn denver() -> Geolocation {
    Geolocation {
        latitude: 39.742476,
        longitude: -105.1786,
        north_angle: 12.5,
        utc_offset: -7.0,
    }
}

fn evening() -> ShadowSettings {
    ShadowSettings {
        time: LocalTime::new(2003, 10, 17, 18, 5).unwrap(),
        shadows: true,
        sun_shading: false,
        light: 0.7,
        dark: 0.1,
        on_ground: false,
    }
}

fn read_err(text: &str) -> ReadError {
    match from_text(text) {
        Ok(_) => panic!("expected an error for {text:?}"),
        Err(e) => e,
    }
}

fn kind_at(text: &str, line: usize, kind: ReadErrorKind) {
    assert_eq!(read_err(text), ReadError { line, kind }, "text: {text:?}");
}

/// A model with one edge in the root, so equality and revisions have something to compare.
fn with_edge() -> Model {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    m.edit_geometry(BodyId::Root, &mut log, |ctx, e| {
        let a = ctx.insert_vertex(DVec3::ZERO, e);
        let b = ctx.insert_vertex(DVec3::X, e);
        ctx.insert_edge(a, b, e).unwrap();
    })
    .unwrap();
    m
}

// -- defaults and setting -----------------------------------------------------------------

#[test]
fn a_new_model_has_the_default_settings() {
    let m = Model::new();
    assert_eq!(m.geolocation(), Geolocation::default());
    assert_eq!(m.shadow_settings(), ShadowSettings::default());
}

#[test]
fn set_geolocation_stores_it_as_one_record() {
    let mut m = with_edge();
    let mut log = ModelEdit::new();
    m.set_geolocation(denver(), &mut log).unwrap();
    assert_eq!(m.geolocation(), denver());
    assert_eq!(
        log.records,
        vec![ModelRecord::SetGeolocation {
            from: Geolocation::default(),
            to: denver(),
        }]
    );
    assert_eq!(m.shadow_settings(), ShadowSettings::default());
}

#[test]
fn set_shadow_settings_stores_them_as_one_record() {
    let mut m = with_edge();
    let mut log = ModelEdit::new();
    m.set_shadow_settings(evening(), &mut log).unwrap();
    assert_eq!(m.shadow_settings(), evening());
    assert_eq!(
        log.records,
        vec![ModelRecord::SetShadows {
            from: ShadowSettings::default(),
            to: evening(),
        }]
    );
    assert_eq!(m.geolocation(), Geolocation::default());
}

#[test]
fn setting_the_same_value_records_nothing() {
    let mut m = with_edge();
    let mut log = ModelEdit::new();
    m.set_geolocation(Geolocation::default(), &mut log).unwrap();
    m.set_shadow_settings(ShadowSettings::default(), &mut log)
        .unwrap();
    assert!(log.records.is_empty());
    m.set_geolocation(denver(), &mut log).unwrap();
    m.set_geolocation(denver(), &mut log).unwrap();
    assert_eq!(log.records.len(), 1);
}

#[test]
fn bad_geolocations_are_refused() {
    let base = denver();
    let bad = [
        Geolocation {
            latitude: f64::NAN,
            ..base
        },
        Geolocation {
            latitude: 90.0001,
            ..base
        },
        Geolocation {
            latitude: -91.0,
            ..base
        },
        Geolocation {
            longitude: 180.5,
            ..base
        },
        Geolocation {
            longitude: -181.0,
            ..base
        },
        Geolocation {
            longitude: f64::INFINITY,
            ..base
        },
        Geolocation {
            north_angle: 360.5,
            ..base
        },
        Geolocation {
            north_angle: -361.0,
            ..base
        },
        Geolocation {
            north_angle: f64::NAN,
            ..base
        },
        Geolocation {
            utc_offset: 14.25,
            ..base
        },
        Geolocation {
            utc_offset: -12.5,
            ..base
        },
        Geolocation {
            utc_offset: f64::NEG_INFINITY,
            ..base
        },
    ];
    for g in bad {
        let mut m = with_edge();
        let before = m.clone();
        let mut log = ModelEdit::new();
        assert_eq!(
            m.set_geolocation(g, &mut log),
            Err(ModelError::BadSettings),
            "{g:?}"
        );
        assert!(log.records.is_empty());
        assert_eq!(m.geolocation(), Geolocation::default());
        assert_eq!(m, before);
    }
}

#[test]
fn limit_geolocations_are_accepted() {
    let good = [
        (90.0, 180.0, 360.0, 14.0),
        (-90.0, -180.0, -360.0, -12.0),
        (0.0, 0.0, 0.0, 5.75),
        (-33.9, 18.4, -0.0, 2.0),
    ];
    for (latitude, longitude, north_angle, utc_offset) in good {
        let g = Geolocation {
            latitude,
            longitude,
            north_angle,
            utc_offset,
        };
        let mut m = Model::new();
        let mut log = ModelEdit::new();
        m.set_geolocation(g, &mut log).unwrap();
        assert_eq!(m.geolocation(), g);
    }
}

#[test]
fn bad_shadow_settings_are_refused() {
    let base = evening();
    let bad_time = LocalTime {
        year: 2026,
        month: 2,
        day: 30,
        hour: 12,
        minute: 0,
    };
    let bad = [
        ShadowSettings {
            light: 1.01,
            ..base
        },
        ShadowSettings {
            light: -0.01,
            ..base
        },
        ShadowSettings {
            light: f64::NAN,
            ..base
        },
        ShadowSettings { dark: 2.0, ..base },
        ShadowSettings { dark: -1.0, ..base },
        ShadowSettings {
            dark: f64::INFINITY,
            ..base
        },
        ShadowSettings {
            time: bad_time,
            ..base
        },
        ShadowSettings {
            time: LocalTime {
                hour: 24,
                ..base.time
            },
            ..base
        },
    ];
    for s in bad {
        let mut m = with_edge();
        let mut log = ModelEdit::new();
        assert_eq!(
            m.set_shadow_settings(s, &mut log),
            Err(ModelError::BadSettings),
            "{s:?}"
        );
        assert!(log.records.is_empty());
        assert_eq!(m.shadow_settings(), ShadowSettings::default());
    }
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let edges = ShadowSettings {
        light: 0.0,
        dark: 1.0,
        ..base
    };
    m.set_shadow_settings(edges, &mut log).unwrap();
    assert_eq!(m.shadow_settings(), edges);
}

// -- records, undo, equality, revision ----------------------------------------------------

#[test]
fn records_undo_and_redo() {
    let mut m = with_edge();
    let mut undo = UndoStack::new();
    let mut log = ModelEdit::new();
    m.set_geolocation(denver(), &mut log).unwrap();
    m.set_shadow_settings(evening(), &mut log).unwrap();
    undo.push("Shadow Settings", log);
    assert_eq!(
        undo.undo(&mut m).unwrap().as_deref(),
        Some("Shadow Settings")
    );
    assert_eq!(m.geolocation(), Geolocation::default());
    assert_eq!(m.shadow_settings(), ShadowSettings::default());
    undo.redo(&mut m).unwrap();
    assert_eq!(m.geolocation(), denver());
    assert_eq!(m.shadow_settings(), evening());
}

#[test]
fn a_record_whose_from_does_not_match_is_refused() {
    let mut m = Model::new();
    let wrong = ModelRecord::SetGeolocation {
        from: denver(),
        to: Geolocation::default(),
    };
    assert_eq!(m.apply(&wrong), Err(ModelError::Mismatch));
    let wrong = ModelRecord::SetShadows {
        from: evening(),
        to: ShadowSettings::default(),
    };
    assert_eq!(m.apply(&wrong), Err(ModelError::Mismatch));
    assert_eq!(m.geolocation(), Geolocation::default());
    assert_eq!(m.shadow_settings(), ShadowSettings::default());
}

#[test]
fn a_record_with_bad_values_is_refused() {
    let mut m = Model::new();
    let bad = ModelRecord::SetGeolocation {
        from: Geolocation::default(),
        to: Geolocation {
            latitude: 100.0,
            ..denver()
        },
    };
    assert_eq!(m.apply(&bad), Err(ModelError::BadSettings));
    let bad = ModelRecord::SetShadows {
        from: ShadowSettings::default(),
        to: ShadowSettings {
            light: 3.0,
            ..evening()
        },
    };
    assert_eq!(m.apply(&bad), Err(ModelError::BadSettings));
    assert_eq!(m, Model::new());
}

#[test]
fn inverse_swaps_from_and_to() {
    let r = ModelRecord::SetShadows {
        from: ShadowSettings::default(),
        to: evening(),
    };
    assert_eq!(
        r.inverse(),
        ModelRecord::SetShadows {
            from: evening(),
            to: ShadowSettings::default(),
        }
    );
}

#[test]
fn equality_compares_the_settings() {
    let a = with_edge();
    let mut b = a.clone();
    assert_eq!(a, b);
    let mut log = ModelEdit::new();
    b.set_geolocation(denver(), &mut log).unwrap();
    assert_ne!(a, b);
    let mut c = a.clone();
    c.set_shadow_settings(evening(), &mut log).unwrap();
    assert_ne!(a, c);
}

#[test]
fn settings_do_not_change_the_revision() {
    // The revision keys geometry caches (picking, scenes); settings don't touch geometry.
    let mut m = with_edge();
    let r = m.revision();
    let mut undo = UndoStack::new();
    let mut log = ModelEdit::new();
    m.set_geolocation(denver(), &mut log).unwrap();
    m.set_shadow_settings(evening(), &mut log).unwrap();
    assert_eq!(m.revision(), r);
    undo.push("settings", log);
    undo.undo(&mut m).unwrap();
    assert_eq!(m.revision(), r);
    undo.redo(&mut m).unwrap();
    assert_eq!(m.revision(), r);
}

#[test]
fn a_failed_edit_rolls_settings_back() {
    let mut m = with_edge();
    let mut edit = ModelEdit::new();
    edit.records.push(ModelRecord::SetGeolocation {
        from: Geolocation::default(),
        to: denver(),
    });
    edit.records.push(ModelRecord::RemoveMaterial(
        MaterialId(99),
        Material::new("x", [1.0, 1.0, 1.0, 1.0]),
    ));
    assert!(m.apply_edit(&edit).is_err());
    assert_eq!(m.geolocation(), Geolocation::default());
}

// -- native file --------------------------------------------------------------------------

#[test]
fn version_is_four() {
    assert_eq!(FORMAT_VERSION, 4);
}

#[test]
fn default_settings_keep_the_old_versions() {
    let text = to_text(&Model::new()).unwrap();
    assert_eq!(text, "modeler-model 1\nbody root\nend\n");
    let text = to_text(&with_edge()).unwrap();
    assert!(text.starts_with("modeler-model 1\n"));
    assert!(!text.contains("geolocation") && !text.contains("sun "));
}

#[test]
fn writer_puts_settings_after_the_header() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    m.set_geolocation(denver(), &mut log).unwrap();
    m.set_shadow_settings(evening(), &mut log).unwrap();
    m.add_material(Material::new("red", [1.0, 0.0, 0.0, 1.0]), &mut log);
    let text = to_text(&m).unwrap();
    assert_eq!(
        text,
        "modeler-model 4\n\
         geolocation 39.742476 -105.1786 12.5 -7\n\
         sun 2003 10 17 18 5 on off 0.7 0.1 off\n\
         material 0 1 0 0 1 \"red\" -\n\
         body root\n\
         end\n"
    );
}

#[test]
fn only_changed_settings_are_written() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    m.set_shadow_settings(
        ShadowSettings {
            shadows: true,
            ..ShadowSettings::default()
        },
        &mut log,
    )
    .unwrap();
    let text = to_text(&m).unwrap();
    assert_eq!(
        text,
        "modeler-model 4\nsun 2026 9 21 14 0 on off 0.8 0.45 on\nbody root\nend\n"
    );
    let mut m = Model::new();
    m.set_geolocation(
        Geolocation {
            north_angle: 90.0,
            ..Geolocation::default()
        },
        &mut log,
    )
    .unwrap();
    assert_eq!(
        to_text(&m).unwrap(),
        "modeler-model 4\ngeolocation 45 0 90 0\nbody root\nend\n"
    );
}

#[test]
fn settings_round_trip_exactly() {
    let mut m = with_edge();
    let mut log = ModelEdit::new();
    let g = Geolocation {
        latitude: 0.1 + 0.2,
        longitude: -1.0 / 3.0,
        north_angle: -0.0,
        utc_offset: 5.5,
    };
    let s = ShadowSettings {
        time: LocalTime::new(-44, 3, 15, 0, 59).unwrap(),
        shadows: false,
        sun_shading: true,
        light: 1.0 / 7.0,
        dark: 0.0,
        on_ground: true,
    };
    m.set_geolocation(g, &mut log).unwrap();
    m.set_shadow_settings(s, &mut log).unwrap();
    let text = to_text(&m).unwrap();
    let back = from_text(&text).unwrap();
    assert_eq!(back, m);
    let bg = back.geolocation();
    assert_eq!(bg.latitude.to_bits(), g.latitude.to_bits());
    assert_eq!(bg.longitude.to_bits(), g.longitude.to_bits());
    assert_eq!(bg.north_angle.to_bits(), g.north_angle.to_bits());
    assert_eq!(bg.utc_offset.to_bits(), g.utc_offset.to_bits());
    assert_eq!(back.shadow_settings(), s);
    assert_eq!(to_text(&back).unwrap(), text);
}

#[test]
fn reader_accepts_settings_anywhere_before_end() {
    let text = "modeler-model 4\n\
                material 0 1 1 1 1 \"m\" -\n\
                body root\n\
                vertex 0 0 0 0\n\
                sun 2026 1 2 3 4 off on 0.5 0.5 off\n\
                geolocation -10 20 30 1\n\
                end\n";
    let m = from_text(text).unwrap();
    assert_eq!(
        m.geolocation(),
        Geolocation {
            latitude: -10.0,
            longitude: 20.0,
            north_angle: 30.0,
            utc_offset: 1.0,
        }
    );
    let s = m.shadow_settings();
    assert_eq!(s.time, LocalTime::new(2026, 1, 2, 3, 4).unwrap());
    assert!(!s.shadows && s.sun_shading && !s.on_ground);
    assert_eq!((s.light, s.dark), (0.5, 0.5));
}

#[test]
fn reader_refuses_settings_in_older_versions() {
    kind_at(
        "modeler-model 3\ngeolocation 1 2 3 4\nbody root\nend\n",
        2,
        ReadErrorKind::UnknownRecord,
    );
    kind_at(
        "modeler-model 1\nsun 2026 1 2 3 4 off on 0.5 0.5 off\nbody root\nend\n",
        2,
        ReadErrorKind::UnknownRecord,
    );
}

#[test]
fn reader_refuses_malformed_settings() {
    let bad = [
        "geolocation 1 2 3",
        "geolocation 1 2 3 4 5",
        "geolocation 1 2 x 4",
        "geolocation 1 2 inf 4",
        "geolocation 1 2 NaN 4",
        "sun 2026 1 2 3 4 off on 0.5 0.5",
        "sun 2026 1 2 3 4 off on 0.5 0.5 off off",
        "sun 2026 1 2 3 4 yes on 0.5 0.5 off",
        "sun 2026 1 2 3 4 off on 0.5 0.5 1",
        "sun 2026 1 2 3 4.5 off on 0.5 0.5 off",
        "sun 2026 -1 2 3 4 off on 0.5 0.5 off",
        "sun 2026 1 2 3 4 off on nan 0.5 off",
    ];
    for line in bad {
        kind_at(
            &format!("modeler-model 4\n{line}\nbody root\nend\n"),
            2,
            ReadErrorKind::BadField,
        );
    }
}

#[test]
fn reader_refuses_a_second_settings_record() {
    kind_at(
        "modeler-model 4\ngeolocation 1 2 3 4\ngeolocation 1 2 3 4\nbody root\nend\n",
        3,
        ReadErrorKind::BadField,
    );
    kind_at(
        "modeler-model 4\nsun 2026 1 2 3 4 off on 0.5 0.5 off\nbody root\nsun 2026 1 2 3 4 off on 0.5 0.5 off\nend\n",
        4,
        ReadErrorKind::BadField,
    );
}

#[test]
fn reader_rejects_out_of_range_settings() {
    let bad = [
        "geolocation 91 0 0 0",
        "geolocation 0 200 0 0",
        "geolocation 0 0 0 15",
        "sun 2026 2 30 12 0 off off 0.5 0.5 off",
        "sun 2026 1 1 24 0 off off 0.5 0.5 off",
        "sun 2026 1 1 12 0 off off 1.5 0.5 off",
    ];
    for line in bad {
        kind_at(
            &format!("modeler-model 4\n{line}\nbody root\nend\n"),
            2,
            ReadErrorKind::Rejected(ModelError::BadSettings),
        );
    }
}

#[test]
fn save_and_load_keep_the_settings() {
    let dir = std::env::temp_dir().join(format!("modeler-geo-model-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sun.modeler");
    let mut m = with_edge();
    let mut log = ModelEdit::new();
    m.set_geolocation(denver(), &mut log).unwrap();
    m.set_shadow_settings(evening(), &mut log).unwrap();
    save(&m, &path).unwrap();
    let back = load(&path).unwrap();
    assert_eq!(back, m);
    assert_eq!(back.geolocation(), denver());
    assert_eq!(back.shadow_settings(), evening());
    std::fs::remove_dir_all(&dir).unwrap();
}
