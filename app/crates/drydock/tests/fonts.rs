// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m10-brand-fonts.md`: Roboto and Roboto Mono for all UI,
//! nothing from the OS or from egui's defaults.

use std::collections::BTreeSet;
use std::path::PathBuf;

use drydock::look::{self, Theme};
use eframe::egui::{self, FontFamily, FontId, TextStyle};

fn dir(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn names(fonts: &egui::FontDefinitions, family: &FontFamily) -> Vec<String> {
    fonts.families[family].clone()
}

#[test]
fn the_four_families_hold_exactly_one_bundled_font_each() {
    let fonts = look::font_definitions();
    assert_eq!(names(&fonts, &FontFamily::Proportional), ["Roboto"]);
    assert_eq!(names(&fonts, &FontFamily::Monospace), ["RobotoMono"]);
    assert_eq!(names(&fonts, &look::medium()), ["Roboto-Medium"]);
    assert_eq!(names(&fonts, &look::mono_medium()), ["RobotoMono-Medium"]);
    assert_eq!(fonts.families.len(), 4, "no other families");
}

#[test]
fn no_egui_default_fonts_and_no_system_font() {
    let fonts = look::font_definitions();
    let keys: BTreeSet<&str> = fonts.font_data.keys().map(String::as_str).collect();
    let want: BTreeSet<&str> = ["Roboto", "Roboto-Medium", "RobotoMono", "RobotoMono-Medium"]
        .into_iter()
        .collect();
    assert_eq!(keys, want);
}

#[test]
fn font_data_is_the_bundled_files() {
    let fonts = look::font_definitions();
    let pairs = [
        ("Roboto", look::ROBOTO),
        ("Roboto-Medium", look::ROBOTO_MEDIUM),
        ("RobotoMono", look::ROBOTO_MONO),
        ("RobotoMono-Medium", look::ROBOTO_MONO_MEDIUM),
    ];
    for (key, bytes) in pairs {
        assert_eq!(&fonts.font_data[key].font[..], bytes, "{key}");
        assert_eq!(&bytes[..4], &[0, 1, 0, 0], "{key} is a TrueType file");
    }
    let distinct: BTreeSet<&[u8]> = pairs.iter().map(|(_, b)| *b).collect();
    assert_eq!(distinct.len(), 4);
}

#[test]
fn buttons_and_headings_use_roboto_medium() {
    for theme in [Theme::Light, Theme::Dark] {
        let styles = look::style(theme).text_styles;
        assert_eq!(
            styles[&TextStyle::Button].family,
            look::medium(),
            "{theme:?}"
        );
        assert_eq!(
            styles[&TextStyle::Heading].family,
            look::medium(),
            "{theme:?}"
        );
        assert_eq!(
            styles[&TextStyle::Body].family,
            FontFamily::Proportional,
            "{theme:?}"
        );
        assert_eq!(
            styles[&TextStyle::Small].family,
            FontFamily::Proportional,
            "{theme:?}"
        );
        assert_eq!(
            styles[&TextStyle::Monospace].family,
            FontFamily::Monospace,
            "{theme:?}"
        );
        // Sizes stay what egui has them.
        assert!(styles[&TextStyle::Heading].size > styles[&TextStyle::Body].size);
    }
}

/// Widths of `text` in each family at 14 points, laid out with the given fonts installed.
fn widths(setup: impl Fn(&egui::Context), text: &str, families: &[FontFamily]) -> Vec<f32> {
    let ctx = egui::Context::default();
    setup(&ctx);
    let mut out = Vec::new();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        for family in families {
            let galley = ui.painter().layout_no_wrap(
                text.to_owned(),
                FontId::new(14.0, family.clone()),
                egui::Color32::WHITE,
            );
            out.push(galley.size().x);
        }
    });
    output.textures_delta.clear();
    out
}

fn install(ctx: &egui::Context) {
    ctx.set_fonts(look::font_definitions());
}

#[test]
fn every_font_parses_and_lays_out_text() {
    let all = [
        FontFamily::Proportional,
        FontFamily::Monospace,
        look::medium(),
        look::mono_medium(),
    ];
    for w in widths(install, "Push/Pull 0123", &all) {
        assert!(w > 10.0, "text measured {w}");
    }
}

#[test]
fn monospace_families_are_monospaced_and_proportional_ones_are_not() {
    let all = [
        FontFamily::Proportional,
        FontFamily::Monospace,
        look::medium(),
        look::mono_medium(),
    ];
    let narrow = widths(install, "iiiiiiii", &all);
    let wide = widths(install, "WWWWWWWW", &all);
    assert!(narrow[0] < wide[0] - 1.0, "Roboto is proportional");
    assert!(narrow[2] < wide[2] - 1.0, "Roboto Medium is proportional");
    assert!(
        (narrow[1] - wide[1]).abs() < 0.01,
        "Roboto Mono is monospaced"
    );
    assert!(
        (narrow[3] - wide[3]).abs() < 0.01,
        "Roboto Mono Medium is monospaced"
    );
}

#[test]
fn medium_is_a_different_cut_from_regular() {
    let all = [
        FontFamily::Proportional,
        look::medium(),
        FontFamily::Monospace,
        look::mono_medium(),
    ];
    let w = widths(install, "Rotate Orient Offset 0123456789", &all);
    assert!((w[0] - w[1]).abs() > 0.01, "Roboto vs Medium: {w:?}");
    // Roboto Mono has one advance for both weights, so the cut shows in the outlines only:
    // the files must at least differ (see font_data_is_the_bundled_files).
    assert!((w[2] - w[3]).abs() < 0.5, "same advance: {w:?}");
}

#[test]
fn apply_installs_the_same_fonts() {
    let all = [look::medium(), look::mono_medium()];
    let direct = widths(install, "Cancel 12.5", &all);
    let applied = widths(look::apply, "Cancel 12.5", &all);
    assert_eq!(direct, applied);
    assert!(applied[0] > 10.0);
}

#[test]
fn the_os_font_lookup_is_gone() {
    let src = std::fs::read_to_string(dir("src/look.rs")).unwrap();
    for gone in [
        "font_candidates",
        "first_readable_font",
        "segoeui",
        "NotoSans",
    ] {
        assert!(!src.contains(gone), "look.rs still mentions {gone}");
    }
}

#[test]
fn licenses_and_sources_are_recorded() {
    let fonts = dir("assets/fonts");
    for file in [
        "Roboto-Regular.ttf",
        "Roboto-Medium.ttf",
        "RobotoMono-Regular.ttf",
        "RobotoMono-Medium.ttf",
        "LICENSE-Roboto",
        "LICENSE-RobotoMono",
    ] {
        assert!(fonts.join(file).is_file(), "{file}");
    }
    let mono = std::fs::read_to_string(fonts.join("LICENSE-RobotoMono")).unwrap();
    assert!(mono.contains("Apache License"));
    let readme = std::fs::read_to_string(fonts.join("README.md")).unwrap();
    let notice = std::fs::read_to_string(dir("../../NOTICE")).unwrap();
    for file in [
        "Roboto-Medium.ttf",
        "RobotoMono-Regular.ttf",
        "RobotoMono-Medium.ttf",
    ] {
        assert!(readme.contains(file), "README.md lacks {file}");
        assert!(notice.contains(file), "NOTICE lacks {file}");
    }
    assert!(readme.contains("sha256"));
}
