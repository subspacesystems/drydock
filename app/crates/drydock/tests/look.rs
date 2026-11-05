// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-native-look.md`.

use drydock::look::{self, Theme};
use eframe::egui;

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
