// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m10-theme-linux-portal.md`.

use drydock::look::{self, Theme};
use drydock::portal::{parse_read_reply, parse_signal_line, theme_for_scheme};
use eframe::egui;

#[test]
fn scheme_mapping() {
    assert_eq!(theme_for_scheme(Some(1)), Theme::Dark);
    assert_eq!(theme_for_scheme(Some(2)), Theme::Light);
    assert_eq!(theme_for_scheme(Some(0)), Theme::Light, "GNOME Default");
    assert_eq!(theme_for_scheme(Some(7)), Theme::Light);
    assert_eq!(
        theme_for_scheme(None),
        Theme::Dark,
        "the desktop doesn't say"
    );
}

#[test]
fn read_reply_forms() {
    assert_eq!(parse_read_reply("(<uint32 1>,)\n"), Some(1));
    assert_eq!(parse_read_reply("(<uint32 0>,)"), Some(0));
    assert_eq!(parse_read_reply("(<<uint32 2>>,)\n"), Some(2));
    assert_eq!(parse_read_reply("  ( <uint32 2> , ) "), Some(2));
}

#[test]
fn read_reply_rejects_noise() {
    assert_eq!(parse_read_reply(""), None);
    assert_eq!(parse_read_reply("\n"), None);
    assert_eq!(
        parse_read_reply(
            "Error: GDBus.Error:org.freedesktop.DBus.Error.ServiceUnknown: The name is not activatable"
        ),
        None
    );
    assert_eq!(parse_read_reply("(<'dark'>,)"), None);
    assert_eq!(parse_read_reply("(<uint32 >,)"), None);
    assert_eq!(parse_read_reply("(<uint32 -1>,)"), None);
}

const PREFIX: &str =
    "/org/freedesktop/portal/desktop: org.freedesktop.portal.Settings.SettingChanged";

#[test]
fn signal_line_color_scheme() {
    let line = format!("{PREFIX} ('org.freedesktop.appearance', 'color-scheme', <uint32 1>)");
    assert_eq!(parse_signal_line(&line), Some(1));
    let line = format!("{PREFIX} ('org.freedesktop.appearance', 'color-scheme', <uint32 0>)\n");
    assert_eq!(parse_signal_line(&line), Some(0));
}

#[test]
fn signal_line_ignores_other_settings() {
    let other_key =
        format!("{PREFIX} ('org.freedesktop.appearance', 'accent-color', <(0.2, 0.4, 0.9)>)");
    assert_eq!(parse_signal_line(&other_key), None);
    let other_ns = format!("{PREFIX} ('org.gnome.desktop.interface', 'color-scheme', <uint32 1>)");
    assert_eq!(parse_signal_line(&other_ns), None);
    let other_signal = "/org/freedesktop/portal/desktop: org.freedesktop.DBus.Properties.PropertiesChanged ('org.freedesktop.appearance', 'color-scheme', <uint32 1>)";
    assert_eq!(parse_signal_line(other_signal), None);
    assert_eq!(parse_signal_line(""), None);
    assert_eq!(
        parse_signal_line(
            "Monitoring signals from all objects owned by org.freedesktop.portal.Desktop"
        ),
        None
    );
    let text = format!("{PREFIX} ('org.freedesktop.appearance', 'color-scheme', <'dark'>)");
    assert_eq!(parse_signal_line(&text), None);
}

fn frame(ctx: &egui::Context) {
    let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
}

#[test]
fn fallback_theme_switches_visuals_live() {
    let ctx = egui::Context::default();
    look::apply(&ctx);
    frame(&ctx);
    // Xvfb has no portal and winit reports no theme: the "doesn't say" answer is Dark.
    assert_eq!(look::theme_of(&ctx.global_style().visuals), Theme::Dark);

    look::set_fallback_theme(&ctx, Theme::Light);
    frame(&ctx);
    assert_eq!(look::theme_of(&ctx.global_style().visuals), Theme::Light);
    assert_eq!(ctx.options(|o| o.fallback_theme), egui::Theme::Light);

    look::set_fallback_theme(&ctx, Theme::Dark);
    frame(&ctx);
    assert_eq!(look::theme_of(&ctx.global_style().visuals), Theme::Dark);
}

#[test]
fn viewport_style_follows_the_theme() {
    let mut style = look::viewport_style(Theme::Dark);
    assert!(look::apply_viewport_theme(&mut style, Theme::Light));
    assert_eq!(
        style.background,
        look::viewport_style(Theme::Light).background
    );
}

#[test]
fn no_portal_here_means_no_scheme() {
    // Xvfb test machines have no session portal; with gdbus missing or no bus this is None.
    // If a developer's desktop does answer, any number is fine, but it must not hang.
    let started = std::time::Instant::now();
    let _ = drydock::portal::read_scheme();
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
}
