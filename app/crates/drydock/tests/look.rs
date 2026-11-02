// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-native-look.md`.

use std::path::PathBuf;

use drydock::look::{self, Os, Theme};
use eframe::egui;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("modeler-look-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn theme_follows_the_os_and_defaults_to_light() {
    assert_eq!(look::theme_from(Some(egui::Theme::Dark)), Theme::Dark);
    assert_eq!(look::theme_from(Some(egui::Theme::Light)), Theme::Light);
    assert_eq!(look::theme_from(None), Theme::Light);
}

#[test]
fn style_matches_the_scheme() {
    assert!(!look::style(Theme::Light).visuals.dark_mode);
    assert!(look::style(Theme::Dark).visuals.dark_mode);
}

#[test]
fn controls_are_nearly_square() {
    for theme in [Theme::Light, Theme::Dark] {
        let style = look::style(theme);
        let w = &style.visuals.widgets;
        for widget in [
            &w.noninteractive,
            &w.inactive,
            &w.hovered,
            &w.active,
            &w.open,
        ] {
            assert!(widget.corner_radius.nw <= 3, "{theme:?}");
            assert!(widget.corner_radius.se <= 3, "{theme:?}");
        }
        assert!(style.visuals.menu_corner_radius.nw <= 3);
    }
}

#[test]
fn hovering_a_control_changes_its_fill() {
    for theme in [Theme::Light, Theme::Dark] {
        let w = look::style(theme).visuals.widgets;
        assert_ne!(w.hovered.weak_bg_fill, w.inactive.weak_bg_fill, "{theme:?}");
        assert_ne!(w.hovered.bg_fill, w.inactive.bg_fill, "{theme:?}");
    }
}

#[test]
fn spacing_is_standard() {
    for theme in [Theme::Light, Theme::Dark] {
        let s = look::style(theme).spacing;
        assert!((4.0..=8.0).contains(&s.item_spacing.y), "{theme:?}");
        assert!((20.0..=28.0).contains(&s.interact_size.y), "{theme:?}");
        assert!(s.button_padding.x >= 6.0, "{theme:?}");
    }
}

#[test]
fn font_candidates_are_absolute_font_files() {
    for os in [Os::Windows, Os::Mac, Os::Linux] {
        let paths = look::font_candidates(os);
        assert!(!paths.is_empty(), "{os:?}");
        for path in paths {
            let text = path.to_string_lossy().to_lowercase();
            assert!(
                text.ends_with(".ttf") || text.ends_with(".otf") || text.ends_with(".ttc"),
                "{text}"
            );
            let absolute = text.starts_with('/') || text.chars().nth(1) == Some(':');
            assert!(absolute, "{text}");
        }
    }
}

#[test]
fn windows_prefers_segoe_ui() {
    let first = look::font_candidates(Os::Windows)[0]
        .to_string_lossy()
        .to_lowercase();
    assert!(first.ends_with("segoeui.ttf"));
}

#[test]
fn first_readable_font_skips_missing_and_non_font_files() {
    let dir = temp_dir("readable");
    let missing = dir.join("missing.ttf");
    let text = dir.join("notes.ttf");
    std::fs::write(&text, b"this is not a font").unwrap();
    let good = dir.join("good.ttf");
    std::fs::write(&good, [0u8, 1, 0, 0, 9, 9, 9, 9]).unwrap();
    let later = dir.join("later.otf");
    std::fs::write(&later, b"OTTOxxxx").unwrap();

    let bytes = look::first_readable_font(&[missing.clone(), text.clone(), good, later]).unwrap();
    assert_eq!(bytes, vec![0u8, 1, 0, 0, 9, 9, 9, 9]);
    assert_eq!(look::first_readable_font(&[missing, text]), None);
    assert_eq!(look::first_readable_font(&[]), None);
}

#[test]
fn first_readable_font_accepts_opentype_and_collections() {
    let dir = temp_dir("kinds");
    let otf = dir.join("a.otf");
    std::fs::write(&otf, b"OTTOxxxx").unwrap();
    assert!(look::first_readable_font(&[otf]).is_some());
    let ttc = dir.join("a.ttc");
    std::fs::write(&ttc, b"ttcfxxxx").unwrap();
    assert!(look::first_readable_font(&[ttc]).is_some());
}

#[test]
fn without_a_system_font_only_roboto_is_used() {
    let fonts = look::font_definitions(None);
    assert_eq!(
        fonts.families[&egui::FontFamily::Proportional],
        vec!["Roboto".to_owned()]
    );
    assert_eq!(
        fonts.families[&egui::FontFamily::Monospace],
        vec!["Roboto".to_owned()]
    );
    assert!(!fonts.font_data.contains_key("System"));
}

#[test]
fn the_system_font_comes_first_with_roboto_as_fallback() {
    let fonts = look::font_definitions(Some(vec![0, 1, 0, 0]));
    assert_eq!(
        fonts.families[&egui::FontFamily::Proportional],
        vec!["System".to_owned(), "Roboto".to_owned()]
    );
    assert_eq!(
        fonts.families[&egui::FontFamily::Monospace],
        vec!["Roboto".to_owned()]
    );
    assert!(fonts.font_data.contains_key("System"));
    assert!(fonts.font_data.contains_key("Roboto"));
}
