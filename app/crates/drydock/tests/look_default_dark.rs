// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m10-theme-default-dark.md`.

use drydock::look::{self, Theme};
use eframe::egui;

fn frame(system: Option<egui::Theme>) -> egui::Context {
    let ctx = egui::Context::default();
    look::apply(&ctx);
    let raw = egui::RawInput {
        system_theme: system,
        ..Default::default()
    };
    let _ = ctx.run_ui(raw, |_| {});
    ctx
}

#[test]
fn no_os_preference_starts_dark() {
    let ctx = frame(None);
    assert_eq!(look::theme_of(&ctx.global_style().visuals), Theme::Dark);
    assert_eq!(
        ctx.options(|o| o.fallback_theme),
        egui::Theme::Dark,
        "the fallback follows look::theme_from(None)"
    );
}

#[test]
fn os_light_still_wins() {
    let ctx = frame(Some(egui::Theme::Light));
    assert_eq!(look::theme_of(&ctx.global_style().visuals), Theme::Light);
}

#[test]
fn os_dark_is_dark() {
    let ctx = frame(Some(egui::Theme::Dark));
    assert_eq!(look::theme_of(&ctx.global_style().visuals), Theme::Dark);
}
