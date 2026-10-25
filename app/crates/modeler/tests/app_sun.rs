// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The model's sun in the viewport: `shading`, the editor's geolocation and shadow
//! setters, and `--screenshot` with the model's sun (`docs/specs/m5-app-sun.md`).

use std::path::{Path, PathBuf};
use std::process::Command;

use modeler::editor::Editor;
use modeler::input::EditKey;
use modeler::shading::{sun_light, sun_light_for, update_style};
use modeler_kernel::format;
use modeler_kernel::model::{Model, ModelEdit};
use modeler_kernel::sun::{Geolocation, LocalTime, ShadowSettings, sun_direction};
use modeler_render::{Style, SunLight};

fn boulder() -> Geolocation {
    Geolocation {
        latitude: 40.0,
        longitude: -105.0,
        north_angle: 0.0,
        utc_offset: -7.0,
    }
}

fn summer_noon() -> LocalTime {
    LocalTime::new(2026, 6, 21, 12, 30).unwrap()
}

fn settings(shadows: bool, sun_shading: bool, on_ground: bool) -> ShadowSettings {
    ShadowSettings {
        time: summer_noon(),
        shadows,
        sun_shading,
        light: 0.75,
        dark: 0.4,
        on_ground,
    }
}

fn undo_label(editor: &Editor) -> Option<String> {
    editor.undo_stack().undo_label().map(str::to_owned)
}

// ---- shading ----

#[test]
fn no_sun_when_shadows_and_sun_shading_are_off() {
    assert_eq!(
        sun_light_for(&boulder(), &settings(false, false, true)),
        None
    );
    assert_eq!(
        sun_light_for(&boulder(), &settings(false, false, false)),
        None
    );
    assert_eq!(
        sun_light_for(&Geolocation::default(), &ShadowSettings::default()),
        None
    );
}

#[test]
fn sun_shading_alone_gives_light_without_shadows() {
    let s = settings(false, true, true);
    let sun = sun_light_for(&boulder(), &s).expect("sun shading is on");
    assert_eq!(
        sun,
        SunLight {
            direction: sun_direction(&boulder(), &summer_noon()),
            light: 0.75,
            dark: 0.4,
            shadows: false,
            on_ground: false,
        }
    );
}

#[test]
fn shadows_with_and_without_ground() {
    for sun_shading in [false, true] {
        let on = sun_light_for(&boulder(), &settings(true, sun_shading, true)).unwrap();
        assert!(on.shadows && on.on_ground, "{on:?}");
        let off = sun_light_for(&boulder(), &settings(true, sun_shading, false)).unwrap();
        assert!(off.shadows && !off.on_ground, "{off:?}");
        assert_eq!(on.direction, sun_direction(&boulder(), &summer_noon()));
        assert_eq!((on.light, on.dark), (0.75, 0.4));
    }
}

#[test]
fn direction_follows_place_time_and_north() {
    let mut g = boulder();
    g.north_angle = 30.0;
    let mut s = settings(true, false, true);
    s.time = LocalTime::new(2026, 3, 2, 8, 15).unwrap();
    let sun = sun_light_for(&g, &s).unwrap();
    assert_eq!(sun.direction, sun_direction(&g, &s.time));
    let noon = sun_light_for(&boulder(), &settings(true, false, true)).unwrap();
    assert!((sun.direction - noon.direction).length() > 0.1);
}

#[test]
fn light_and_dark_convert_to_f32() {
    let mut s = settings(false, true, false);
    s.light = 0.123456789;
    s.dark = 1.0;
    let sun = sun_light_for(&boulder(), &s).unwrap();
    assert_eq!(sun.light, 0.123456789f64 as f32);
    assert_eq!(sun.dark, 1.0);
}

#[test]
fn sun_light_reads_the_model() {
    let mut model = Model::new();
    assert_eq!(sun_light(&model), None);
    let mut log = ModelEdit::default();
    model.set_geolocation(boulder(), &mut log).unwrap();
    assert_eq!(sun_light(&model), None);
    model
        .set_shadow_settings(settings(true, false, true), &mut log)
        .unwrap();
    assert_eq!(
        sun_light(&model),
        sun_light_for(&boulder(), &settings(true, false, true))
    );
    assert!(sun_light(&model).is_some());
}

#[test]
fn update_style_reports_changes_only() {
    let mut style = Style::default();
    let before = style;
    assert!(!update_style(&mut style, None));
    assert_eq!(style.sun, None);
    let sun = sun_light_for(&boulder(), &settings(true, true, true));
    assert!(update_style(&mut style, sun));
    assert_eq!(style.sun, sun);
    assert!(!update_style(&mut style, sun));
    let other = sun_light_for(&boulder(), &settings(true, true, false));
    assert!(update_style(&mut style, other));
    assert_eq!(style.sun, other);
    assert!(update_style(&mut style, None));
    assert_eq!(style.sun, None);
    assert_eq!(style.background, before.background);
    assert_eq!(style.default_face, before.default_face);
    assert_eq!(style.default_line, before.default_line);
}

// ---- editor ----

#[test]
fn set_geolocation_is_one_undo_step() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let before = model.geolocation();
    assert!(editor.set_geolocation(&mut model, boulder()));
    assert_eq!(model.geolocation(), boulder());
    assert_eq!(editor.undo_stack().undo_count(), 1);
    assert_eq!(undo_label(&editor).as_deref(), Some("Set Location"));
    assert_eq!(editor.message(), None);

    assert!(editor.key(EditKey::Undo, &mut model));
    assert_eq!(model.geolocation(), before);
    assert!(editor.key(EditKey::Redo, &mut model));
    assert_eq!(model.geolocation(), boulder());
}

#[test]
fn set_geolocation_unchanged_pushes_nothing() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let same = model.geolocation();
    assert!(editor.set_geolocation(&mut model, same));
    assert_eq!(editor.undo_stack().undo_count(), 0);
    assert!(editor.set_geolocation(&mut model, boulder()));
    assert!(editor.set_geolocation(&mut model, boulder()));
    assert_eq!(editor.undo_stack().undo_count(), 1);
}

#[test]
fn bad_geolocation_is_refused() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let before = model.geolocation();
    let mut bad = boulder();
    bad.latitude = 91.0;
    assert!(!editor.set_geolocation(&mut model, bad));
    assert_eq!(model.geolocation(), before);
    assert_eq!(editor.undo_stack().undo_count(), 0);
    assert_eq!(editor.message(), Some("Can't use that location"));
    let mut nan = boulder();
    nan.utc_offset = f64::NAN;
    assert!(!editor.set_geolocation(&mut model, nan));
    assert_eq!(model.geolocation(), before);

    assert!(editor.set_geolocation(&mut model, boulder()));
    assert_eq!(editor.message(), None);
}

#[test]
fn set_shadow_settings_is_one_undo_step() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let before = model.shadow_settings();
    let s = settings(true, false, true);
    assert!(editor.set_shadow_settings(&mut model, s));
    assert_eq!(model.shadow_settings(), s);
    assert_eq!(editor.undo_stack().undo_count(), 1);
    assert_eq!(undo_label(&editor).as_deref(), Some("Shadow Settings"));
    assert!(sun_light(&model).is_some());

    assert!(editor.key(EditKey::Undo, &mut model));
    assert_eq!(model.shadow_settings(), before);
    assert_eq!(sun_light(&model), None);
    assert!(editor.key(EditKey::Redo, &mut model));
    assert_eq!(model.shadow_settings(), s);
}

#[test]
fn set_shadow_settings_unchanged_pushes_nothing() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let same = model.shadow_settings();
    assert!(editor.set_shadow_settings(&mut model, same));
    assert_eq!(editor.undo_stack().undo_count(), 0);
}

#[test]
fn each_setting_change_is_its_own_step() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let mut s = settings(true, false, true);
    assert!(editor.set_shadow_settings(&mut model, s));
    s.time = LocalTime::new(2026, 6, 21, 15, 0).unwrap();
    assert!(editor.set_shadow_settings(&mut model, s));
    assert!(editor.set_geolocation(&mut model, boulder()));
    assert_eq!(editor.undo_stack().undo_count(), 3);
    assert!(editor.key(EditKey::Undo, &mut model));
    assert!(editor.key(EditKey::Undo, &mut model));
    assert_eq!(model.shadow_settings(), settings(true, false, true));
    assert_eq!(model.geolocation(), Geolocation::default());
}

#[test]
fn bad_shadow_settings_are_refused() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let before = model.shadow_settings();
    let mut bad = settings(true, true, true);
    bad.dark = 1.5;
    assert!(!editor.set_shadow_settings(&mut model, bad));
    assert_eq!(model.shadow_settings(), before);
    assert_eq!(editor.undo_stack().undo_count(), 0);
    assert_eq!(editor.message(), Some("Can't use those shadow settings"));
    let mut bad_time = settings(true, true, true);
    bad_time.time.month = 13;
    assert!(!editor.set_shadow_settings(&mut model, bad_time));
    assert_eq!(model.shadow_settings(), before);

    assert!(editor.set_shadow_settings(&mut model, settings(true, true, true)));
    assert_eq!(editor.message(), None);
}

// ---- --screenshot ----

/// A 2 m square roof at z = 2 with a post from its corner down to the ground, so the
/// framed view includes the ground under it.
const ROOF: &str = "modeler-model 1
body root
vertex 0 0 0 2
vertex 1 2 0 2
vertex 2 2 2 2
vertex 3 0 2 2
vertex 4 0 0 0
edge 0 0 1
edge 1 1 2
edge 2 2 3
edge 3 3 0
edge 4 4 0
face 0 +0 +1 +2 +3
end
";

fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("modeler-sun-test-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Save the roof with `s`, screenshot it, return the RGBA image.
fn shot(dir: &Path, name: &str, s: ShadowSettings) -> image::RgbaImage {
    let mut model = format::from_text(ROOF).expect("roof parses");
    let mut log = ModelEdit::default();
    model.set_geolocation(boulder(), &mut log).unwrap();
    model.set_shadow_settings(s, &mut log).unwrap();
    let file = dir.join(format!("{name}.modeler"));
    format::save(&model, &file).unwrap();
    let png = dir.join(format!("{name}.png"));
    let out = Command::new(env!("CARGO_BIN_EXE_modeler"))
        .args([
            "--screenshot",
            png.to_str().unwrap(),
            file.to_str().unwrap(),
        ])
        .output()
        .expect("modeler binary runs");
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    image::open(&png).unwrap().to_rgba8()
}

/// Pixels within a few levels of the default background.
fn background_pixels(img: &image::RgbaImage) -> usize {
    let bg = Style::default()
        .background
        .map(|c| (c * 255.0).round() as i32);
    img.pixels()
        .filter(|p| (0..3).all(|i| (i32::from(p[i]) - bg[i]).abs() <= 4))
        .count()
}

#[test]
fn screenshot_uses_the_models_sun() {
    let dir = temp_dir("sun");
    let plain = shot(&dir, "plain", settings(false, false, true));
    let shaded = shot(&dir, "shaded", settings(false, true, true));
    let total = (plain.width() * plain.height()) as usize;

    // Sun shading recolors the roof (lit square to the high sun, not by the viewer's
    // light) but puts nothing on the ground.
    let changed = plain
        .pixels()
        .zip(shaded.pixels())
        .filter(|(a, b)| (0..3).any(|i| a[i].abs_diff(b[i]) > 3))
        .count();
    assert!(
        changed > total / 200,
        "sun shading changed {changed} pixels"
    );
    let (a, b) = (background_pixels(&plain), background_pixels(&shaded));
    assert!(a.abs_diff(b) < total / 1000, "{a} vs {b}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn screenshot_without_ground_shadows_leaves_the_background() {
    let dir = temp_dir("noground");
    let plain = shot(&dir, "plain", settings(false, false, false));
    let no_ground = shot(&dir, "noground", settings(true, false, false));
    let total = (plain.width() * plain.height()) as usize;
    let changed = plain
        .pixels()
        .zip(no_ground.pixels())
        .filter(|(a, b)| (0..3).any(|i| a[i].abs_diff(b[i]) > 3))
        .count();
    assert!(changed > total / 200, "the sun changed {changed} pixels");
    let (a, b) = (background_pixels(&plain), background_pixels(&no_ground));
    assert!(a.abs_diff(b) < total / 1000, "{a} vs {b}");
    let _ = std::fs::remove_dir_all(&dir);
}
