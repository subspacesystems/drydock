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
fn controls_have_radius_4_and_menus_and_windows_8() {
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
            assert_eq!(
                widget.corner_radius,
                egui::CornerRadius::same(4),
                "{theme:?}"
            );
        }
        assert_eq!(
            style.visuals.menu_corner_radius,
            egui::CornerRadius::same(8)
        );
        assert_eq!(
            style.visuals.window_corner_radius,
            egui::CornerRadius::same(8)
        );
    }
}

#[test]
fn windows_are_flat_and_popups_keep_a_shadow() {
    for theme in [Theme::Light, Theme::Dark] {
        let v = look::style(theme).visuals;
        assert_eq!(v.window_shadow, egui::Shadow::NONE, "{theme:?}");
        assert_ne!(v.popup_shadow, egui::Shadow::NONE, "{theme:?}");
    }
}

fn c(hex: u32) -> egui::Color32 {
    egui::Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

#[test]
fn palette_holds_the_brand_tokens() {
    let d = look::palette(Theme::Dark);
    assert_eq!(d.bg, c(0x091126));
    assert_eq!(d.bg_sunken, c(0x050A19));
    assert_eq!(d.surface, c(0x0D162D));
    assert_eq!(d.surface_raised, c(0x161F37));
    assert_eq!(d.border, c(0x263047));
    assert_eq!(d.border_control, c(0x5F697E));
    assert_eq!(d.ink, c(0xEEF7FF));
    assert_eq!(d.ink_muted, c(0xAAB1C1));
    assert_eq!(d.accent, c(0x1FD5FF));
    assert_eq!(
        d.accent_soft,
        egui::Color32::from_rgba_unmultiplied(31, 213, 255, 41)
    );
    assert_eq!(d.action, c(0x8E83FC));
    assert_eq!(d.on_action, c(0x091126));
    assert_eq!(d.product, c(0xFF8A3D));
    assert_eq!(d.warning, c(0xFFD24D));
    assert_eq!(d.danger, c(0xFF5C7A));
    assert_eq!(d.viewport_bg, c(0x050A19));
    assert_eq!(d.edge, c(0xEEF7FF));
    assert_eq!(d.face_front, c(0x3D475E));
    assert_eq!(d.face_back, c(0xB04A80));
    assert_eq!(
        (d.axis_x, d.axis_y, d.axis_z),
        (c(0xFF4F64), c(0x33D17A), c(0x6FA3FF))
    );

    let l = look::palette(Theme::Light);
    assert_eq!(l.bg, c(0xEEF7FF));
    assert_eq!(l.bg_sunken, c(0xE2E8F4));
    assert_eq!(l.surface, c(0xFFFFFF));
    assert_eq!(l.surface_raised, c(0xFFFFFF));
    assert_eq!(l.border, c(0xCBD1DF));
    assert_eq!(l.border_control, c(0x848C9F));
    assert_eq!(l.ink, c(0x091126));
    assert_eq!(l.ink_muted, c(0x3D475E));
    assert_eq!(l.accent, c(0x00739C));
    assert_eq!(
        l.accent_soft,
        egui::Color32::from_rgba_unmultiplied(0, 115, 156, 31)
    );
    assert_eq!(l.action, c(0x5247C9));
    assert_eq!(l.on_action, c(0xFFFFFF));
    assert_eq!(l.product, c(0xA84A14));
    assert_eq!(l.warning, c(0x8A6400));
    assert_eq!(l.danger, c(0xB3203F));
    assert_eq!(l.viewport_bg, c(0xF4F8FC));
    assert_eq!(l.edge, c(0x091126));
    assert_eq!(l.face_front, c(0xE2E8F4));
    assert_eq!(l.face_back, c(0xC25A8C));
    assert_eq!(
        (l.axis_x, l.axis_y, l.axis_z),
        (c(0xD62D45), c(0x13994F), c(0x2F6FE0))
    );
}

#[test]
fn style_maps_the_palette_onto_visuals() {
    for theme in [Theme::Light, Theme::Dark] {
        let p = look::palette(theme);
        let v = look::style(theme).visuals;
        let w = &v.widgets;
        assert_eq!(v.panel_fill, p.surface, "{theme:?}");
        assert_eq!(v.window_fill, p.surface_raised, "{theme:?}");
        assert_eq!(v.extreme_bg_color, p.bg_sunken, "{theme:?}");
        assert_eq!(v.faint_bg_color, p.bg, "{theme:?}");
        assert_eq!(v.window_stroke.color, p.border, "{theme:?}");
        assert_eq!(w.noninteractive.bg_stroke.color, p.border, "{theme:?}");
        assert_eq!(w.inactive.bg_stroke.color, p.border_control, "{theme:?}");
        for widget in [
            &w.noninteractive,
            &w.inactive,
            &w.hovered,
            &w.active,
            &w.open,
        ] {
            assert_eq!(widget.fg_stroke.color, p.ink, "{theme:?}");
        }
        assert_eq!(v.weak_text_color, Some(p.ink_muted), "{theme:?}");
        assert_eq!(w.hovered.weak_bg_fill, p.accent_soft, "{theme:?}");
        assert_eq!(w.active.weak_bg_fill, p.accent_soft, "{theme:?}");
        assert_eq!(v.selection.bg_fill, p.accent_soft, "{theme:?}");
        assert_eq!(v.selection.stroke.color, p.accent, "{theme:?}");
        assert_eq!(v.hyperlink_color, p.accent, "{theme:?}");
        assert_eq!(v.warn_fg_color, p.warning, "{theme:?}");
        assert_eq!(v.error_fg_color, p.danger, "{theme:?}");
    }
}

/// sRGB channel to linear light (WCAG 2).
fn lin(v: u8) -> f64 {
    let v = f64::from(v) / 255.0;
    if v <= 0.03928 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: [f64; 3]) -> f64 {
    let l = |v: f64| lin(v.round() as u8);
    0.2126 * l(c[0]) + 0.7152 * l(c[1]) + 0.0722 * l(c[2])
}

fn rgb(c: egui::Color32) -> [f64; 3] {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    [f64::from(r), f64::from(g), f64::from(b)]
}

fn contrast(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// `top` (translucent) over the opaque `under`.
fn over(top: egui::Color32, under: egui::Color32) -> [f64; 3] {
    let [r, g, b, a] = top.to_srgba_unmultiplied();
    let a = f64::from(a) / 255.0;
    let u = rgb(under);
    [
        f64::from(r) * a + u[0] * (1.0 - a),
        f64::from(g) * a + u[1] * (1.0 - a),
        f64::from(b) * a + u[2] * (1.0 - a),
    ]
}

#[test]
fn text_has_at_least_4_5_to_1_contrast() {
    for theme in [Theme::Light, Theme::Dark] {
        let p = look::palette(theme);
        let v = look::style(theme).visuals;
        for (name, ink) in [("ink", p.ink), ("ink-muted", p.ink_muted)] {
            for (bg_name, bg) in [
                ("panel_fill", v.panel_fill),
                ("window_fill", v.window_fill),
                ("faint_bg_color", v.faint_bg_color),
                ("extreme_bg_color", v.extreme_bg_color),
            ] {
                let ratio = contrast(rgb(ink), rgb(bg));
                assert!(ratio >= 4.5, "{theme:?} {name} on {bg_name}: {ratio:.2}");
            }
        }
        let ratio = contrast(rgb(p.ink), over(p.accent_soft, p.surface));
        assert!(ratio >= 4.5, "{theme:?} ink on accent-soft: {ratio:.2}");
    }
}

#[test]
fn the_visuals_use_no_stock_egui_colours() {
    for theme in [Theme::Light, Theme::Dark] {
        let p = look::palette(theme);
        let allowed = [
            p.bg,
            p.bg_sunken,
            p.surface,
            p.surface_raised,
            p.border,
            p.border_control,
            p.ink,
            p.ink_muted,
            p.accent,
            p.accent_soft,
            p.action,
            p.on_action,
            p.product,
            p.warning,
            p.danger,
            egui::Color32::TRANSPARENT,
        ];
        let v = look::style(theme).visuals;
        let w = &v.widgets;
        let mut found = vec![
            ("panel_fill", v.panel_fill),
            ("window_fill", v.window_fill),
            ("extreme_bg_color", v.extreme_bg_color),
            ("faint_bg_color", v.faint_bg_color),
            ("window_stroke", v.window_stroke.color),
            ("selection.bg_fill", v.selection.bg_fill),
            ("selection.stroke", v.selection.stroke.color),
            ("hyperlink_color", v.hyperlink_color),
            ("warn_fg_color", v.warn_fg_color),
            ("error_fg_color", v.error_fg_color),
        ];
        for (state, widget) in [
            ("noninteractive", &w.noninteractive),
            ("inactive", &w.inactive),
            ("hovered", &w.hovered),
            ("active", &w.active),
            ("open", &w.open),
        ] {
            found.push((state, widget.bg_fill));
            found.push((state, widget.weak_bg_fill));
            found.push((state, widget.bg_stroke.color));
            found.push((state, widget.fg_stroke.color));
        }
        for (name, color) in found {
            assert!(
                allowed.contains(&color),
                "{theme:?} {name}: {color:?} is not a palette colour"
            );
        }
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
