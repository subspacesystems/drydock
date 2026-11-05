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
    let c = egui::Color32::from_rgb;
    match theme {
        Theme::Light => Palette {
            bg: c(0xEE, 0xF7, 0xFF),
            bg_sunken: c(0xE2, 0xE8, 0xF4),
            surface: c(0xFF, 0xFF, 0xFF),
            surface_raised: c(0xFF, 0xFF, 0xFF),
            border: c(0xCB, 0xD1, 0xDF),
            border_control: c(0x84, 0x8C, 0x9F),
            ink: c(0x09, 0x11, 0x26),
            ink_muted: c(0x3D, 0x47, 0x5E),
            accent: c(0x00, 0x73, 0x9C),
            accent_soft: egui::Color32::from_rgba_unmultiplied(0, 115, 156, 31),
            action: c(0x52, 0x47, 0xC9),
            on_action: c(0xFF, 0xFF, 0xFF),
            product: c(0xA8, 0x4A, 0x14),
            warning: c(0x8A, 0x64, 0x00),
            danger: c(0xB3, 0x20, 0x3F),
            viewport_bg: c(0xF4, 0xF8, 0xFC),
            edge: c(0x09, 0x11, 0x26),
            face_front: c(0xE2, 0xE8, 0xF4),
            face_back: c(0xC2, 0x5A, 0x8C),
            axis_x: c(0xD6, 0x2D, 0x45),
            axis_y: c(0x13, 0x99, 0x4F),
            axis_z: c(0x2F, 0x6F, 0xE0),
        },
        Theme::Dark => Palette {
            bg: c(0x09, 0x11, 0x26),
            bg_sunken: c(0x05, 0x0A, 0x19),
            surface: c(0x0D, 0x16, 0x2D),
            surface_raised: c(0x16, 0x1F, 0x37),
            border: c(0x26, 0x30, 0x47),
            border_control: c(0x5F, 0x69, 0x7E),
            ink: c(0xEE, 0xF7, 0xFF),
            ink_muted: c(0xAA, 0xB1, 0xC1),
            accent: c(0x1F, 0xD5, 0xFF),
            accent_soft: egui::Color32::from_rgba_unmultiplied(31, 213, 255, 41),
            action: c(0x8E, 0x83, 0xFC),
            on_action: c(0x09, 0x11, 0x26),
            product: c(0xFF, 0x8A, 0x3D),
            warning: c(0xFF, 0xD2, 0x4D),
            danger: c(0xFF, 0x5C, 0x7A),
            viewport_bg: c(0x05, 0x0A, 0x19),
            edge: c(0xEE, 0xF7, 0xFF),
            face_front: c(0x3D, 0x47, 0x5E),
            face_back: c(0xB0, 0x4A, 0x80),
            axis_x: c(0xFF, 0x4F, 0x64),
            axis_y: c(0x33, 0xD1, 0x7A),
            axis_z: c(0x6F, 0xA3, 0xFF),
        },
    }
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

/// The viewport colours for `theme` (`docs/specs/m10-brand-viewport.md`): `background` =
/// `viewport_bg`, `default_line` = `edge`, `default_face` = `face_front`, `default_back` =
/// `Some(face_back)`; all opaque, sRGB 0..1 (byte / 255). `sun` is `None`.
pub fn viewport_style(theme: Theme) -> drydock_render::Style {
    let _ = theme;
    unimplemented!("m10-brand-viewport")
}

/// Set the colour fields of `style` (not `sun`) to those of `viewport_style(theme)`; true
/// when anything changed, so the viewport must redraw.
pub fn apply_viewport_theme(style: &mut drydock_render::Style, theme: Theme) -> bool {
    let _ = (style, theme);
    unimplemented!("m10-brand-viewport")
}

/// The colour of model axis `axis` (0 x, 1 y, 2 z) in `theme`: `palette(theme).axis_x` /
/// `axis_y` / `axis_z` as RGB bytes; any other index gives mid-grey `[160, 160, 160]`.
pub fn axis_color(theme: Theme, axis: usize) -> [u8; 3] {
    let _ = (theme, axis);
    unimplemented!("m10-brand-viewport")
}

/// The theme `visuals` belong to: Dark when `visuals.dark_mode`, else Light.
pub fn theme_of(visuals: &egui::Visuals) -> Theme {
    if visuals.dark_mode {
        Theme::Dark
    } else {
        Theme::Light
    }
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
    let mut fonts = FontDefinitions::empty();
    let data = [
        ("Roboto", ROBOTO),
        ("Roboto-Medium", ROBOTO_MEDIUM),
        ("RobotoMono", ROBOTO_MONO),
        ("RobotoMono-Medium", ROBOTO_MONO_MEDIUM),
    ];
    for (name, bytes) in data {
        fonts.font_data.insert(
            name.to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(bytes)),
        );
    }
    fonts
        .families
        .insert(FontFamily::Proportional, vec!["Roboto".to_owned()]);
    fonts
        .families
        .insert(FontFamily::Monospace, vec!["RobotoMono".to_owned()]);
    fonts
        .families
        .insert(medium(), vec!["Roboto-Medium".to_owned()]);
    fonts
        .families
        .insert(mono_medium(), vec!["RobotoMono-Medium".to_owned()]);
    fonts
}

/// Installs the fonts and the style for the OS's theme, both the light and the dark one so
/// a theme change in the OS applies at once.
pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions());
    ctx.set_style_of(egui::Theme::Light, style(Theme::Light));
    ctx.set_style_of(egui::Theme::Dark, style(Theme::Dark));
    ctx.set_theme(egui::ThemePreference::System);
}
