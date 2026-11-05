// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The look (`docs/specs/m6-native-look.md`, `m10-brand-*`): the bundled Roboto fonts,
//! light or dark following the OS, the brand palette and standard spacing. Pure
//! functions over egui types so tests need no window.

use eframe::egui::{self, CornerRadius, FontDefinitions, FontFamily, Margin, Style};

/// Roboto Regular 2.138, bundled (`assets/fonts/README.md`).
pub const ROBOTO: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");
/// Roboto Medium 2.138.
pub const ROBOTO_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Roboto-Medium.ttf");
/// Roboto Mono Regular 3.000 (static instance, wght 400).
pub const ROBOTO_MONO: &[u8] = include_bytes!("../assets/fonts/RobotoMono-Regular.ttf");
/// Roboto Mono Medium 3.000 (static instance, wght 500).
pub const ROBOTO_MONO_MEDIUM: &[u8] = include_bytes!("../assets/fonts/RobotoMono-Medium.ttf");

/// The colour scheme to draw with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

/// The operating systems we special-case (browser command, config folders).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    Mac,
    Linux,
}

impl Os {
    /// The OS this build runs on.
    pub fn current() -> Os {
        if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "macos") {
            Os::Mac
        } else {
            Os::Linux
        }
    }
}

/// The theme to use when the OS reports `system` (`None`: it doesn't say): the OS's choice,
/// else Light.
pub fn theme_from(system: Option<egui::Theme>) -> Theme {
    match system {
        Some(egui::Theme::Dark) => Theme::Dark,
        _ => Theme::Light,
    }
}

/// Every colour the app draws with, for one theme: the Subspace brand tokens of
/// `design/brand/tokens.json` (`docs/specs/m10-brand-theme.md`). Other code takes colours
/// from here, never from literals. There is no ground grid, so `viewport-grid` is absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// `bg`: page and app background.
    pub bg: egui::Color32,
    /// `bg-sunken`: wells, the viewport surround.
    pub bg_sunken: egui::Color32,
    /// `surface`: panels.
    pub surface: egui::Color32,
    /// `surface-raised`: menus, popups, dialogs.
    pub surface_raised: egui::Color32,
    /// `border`: hairlines between regions.
    pub border: egui::Color32,
    /// `border-control`: outlines of inputs and buttons.
    pub border_control: egui::Color32,
    /// `ink`: body text.
    pub ink: egui::Color32,
    /// `ink-muted`: secondary text.
    pub ink_muted: egui::Color32,
    /// `accent`: links, focus, selection, the active state.
    pub accent: egui::Color32,
    /// `accent-soft`: hover and selection tint (translucent; build with
    /// `Color32::from_rgba_unmultiplied`).
    pub accent_soft: egui::Color32,
    /// `action`: primary button fill.
    pub action: egui::Color32,
    /// `on-action`: label on `action`.
    pub on_action: egui::Color32,
    /// `product`: drydock-orange ink, the active-tool marker.
    pub product: egui::Color32,
    /// `warning`.
    pub warning: egui::Color32,
    /// `danger`.
    pub danger: egui::Color32,
    /// `viewport-bg`.
    pub viewport_bg: egui::Color32,
    /// `edge`: model edges.
    pub edge: egui::Color32,
    /// `face-front`.
    pub face_front: egui::Color32,
    /// `face-back`: the rose of reversed faces.
    pub face_back: egui::Color32,
    /// `axis-x`.
    pub axis_x: egui::Color32,
    /// `axis-y`.
    pub axis_y: egui::Color32,
    /// `axis-z`.
    pub axis_z: egui::Color32,
}

/// The palette for `theme`: each field is the token's value for that theme, with aliases
/// resolved (`tokens.json`; the table in `design/brand/README.md`).
pub fn palette(theme: Theme) -> Palette {
    let _ = theme;
    unimplemented!("m10-brand-theme")
}

/// The egui style for `theme`: egui's own colours for the scheme with the desktop look on
/// top. Widget corners at most 3 points, the hovered fill differs from the resting one,
/// item spacing 4 to 8 points tall, rows 20 to 28 points tall, button padding at least 6
/// points wide.
pub fn style(theme: Theme) -> Style {
    let mut style = Style {
        visuals: match theme {
            Theme::Light => egui::Visuals::light(),
            Theme::Dark => egui::Visuals::dark(),
        },
        ..Default::default()
    };
    let widgets = &mut style.visuals.widgets;
    for widget in [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        widget.corner_radius = CornerRadius::same(2);
    }
    style.visuals.menu_corner_radius = CornerRadius::same(2);
    style.visuals.window_corner_radius = CornerRadius::same(2);
    let border = style.visuals.widgets.noninteractive.bg_stroke.color;
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, border);

    style.spacing.item_spacing.y = 6.0;
    style.spacing.interact_size.y = 24.0;
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.menu_margin = Margin::symmetric(2, 4);
    style
}

/// The theme `visuals` belong to: Dark when `visuals.dark_mode`, else Light.
pub fn theme_of(visuals: &egui::Visuals) -> Theme {
    let _ = visuals;
    unimplemented!("m10-brand-dialog-buttons")
}

/// The fill of a primary button under the pointer: `palette(theme).action` blended 10%
/// toward white, per channel, rounded to nearest. Its contrast with `on_action` stays at
/// least 4.5:1.
pub fn primary_hover_fill(theme: Theme) -> egui::Color32 {
    let _ = theme;
    unimplemented!("m10-brand-dialog-buttons")
}

/// A dialog's confirming button (`docs/specs/m10-brand-dialog-buttons.md`): filled with the
/// theme's `action` (`primary_hover_fill` while hovered), label `text` in `on_action`
/// (the label font stays the button text style's; no `medium()` here, tests without the
/// brand fonts must still lay it out), at least `ui.spacing().interact_size` big, with the
/// padding of other buttons. Focus ring and border stay accent. Returns the response.
pub fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let _ = (ui, text);
    unimplemented!("m10-brand-dialog-buttons")
}

/// The family for Roboto Medium (weight 500): buttons and panel headers (`style()`).
pub fn medium() -> FontFamily {
    FontFamily::Name("medium".into())
}

/// The family for Roboto Mono Medium (weight 500): emphasised numbers.
pub fn mono_medium() -> FontFamily {
    FontFamily::Name("mono-medium".into())
}

/// The fonts to install, bundled only (`docs/specs/m10-brand-fonts.md`): font data keys
/// "Roboto", "Roboto-Medium", "RobotoMono", "RobotoMono-Medium". Proportional is
/// [Roboto], Monospace is [Roboto Mono], `medium()` is [Roboto Medium], `mono_medium()` is
/// [Roboto Mono Medium]. Each family holds exactly that one font; no egui default fonts.
pub fn font_definitions() -> FontDefinitions {
    unimplemented!("m10-brand-fonts")
}

/// Installs the fonts and the style for the OS's theme, both the light and the dark one so
/// a theme change in the OS applies at once.
pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions());
    ctx.set_style_of(egui::Theme::Light, style(Theme::Light));
    ctx.set_style_of(egui::Theme::Dark, style(Theme::Dark));
    ctx.set_theme(egui::ThemePreference::System);
}
