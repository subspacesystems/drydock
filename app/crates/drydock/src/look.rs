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
    match theme {
        Theme::Dark => Palette {
            bg: c(0x091126),
            bg_sunken: c(0x050A19),
            surface: c(0x0D162D),
            surface_raised: c(0x161F37),
            border: c(0x263047),
            border_control: c(0x5F697E),
            ink: c(0xEEF7FF),
            ink_muted: c(0xAAB1C1),
            accent: c(0x1FD5FF),
            accent_soft: egui::Color32::from_rgba_unmultiplied(31, 213, 255, 41),
            action: c(0x8E83FC),
            on_action: c(0x091126),
            product: c(0xFF8A3D),
            warning: c(0xFFD24D),
            danger: c(0xFF5C7A),
            viewport_bg: c(0x050A19),
            edge: c(0xEEF7FF),
            face_front: c(0x3D475E),
            face_back: c(0xB04A80),
            axis_x: c(0xFF4F64),
            axis_y: c(0x33D17A),
            axis_z: c(0x6FA3FF),
        },
        Theme::Light => Palette {
            bg: c(0xEEF7FF),
            bg_sunken: c(0xE2E8F4),
            surface: c(0xFFFFFF),
            surface_raised: c(0xFFFFFF),
            border: c(0xCBD1DF),
            border_control: c(0x848C9F),
            ink: c(0x091126),
            ink_muted: c(0x3D475E),
            accent: c(0x00739C),
            accent_soft: egui::Color32::from_rgba_unmultiplied(0, 115, 156, 31),
            action: c(0x5247C9),
            on_action: c(0xFFFFFF),
            product: c(0xA84A14),
            warning: c(0x8A6400),
            danger: c(0xB3203F),
            viewport_bg: c(0xF4F8FC),
            edge: c(0x091126),
            face_front: c(0xE2E8F4),
            face_back: c(0xC25A8C),
            axis_x: c(0xD62D45),
            axis_y: c(0x13994F),
            axis_z: c(0x2F6FE0),
        },
    }
}

/// A colour in the `0xRRGGBB` form of the token tables.
fn c(hex: u32) -> egui::Color32 {
    egui::Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The egui style for `theme`: the brand palette of `palette(theme)` mapped onto egui's
/// visuals, flat: no window shadow (menus and popups keep theirs), controls 4 points
/// rounded, menus and windows 8, the hovered fill differs from the resting one, item
/// spacing 4 to 8 points tall, rows 20 to 28 points tall, button padding at least 6
/// points wide.
pub fn style(theme: Theme) -> Style {
    let p = palette(theme);
    let mut style = Style {
        visuals: match theme {
            Theme::Light => egui::Visuals::light(),
            Theme::Dark => egui::Visuals::dark(),
        },
        ..Default::default()
    };
    let v = &mut style.visuals;
    v.panel_fill = p.surface;
    v.window_fill = p.surface_raised;
    v.extreme_bg_color = p.bg_sunken;
    v.faint_bg_color = p.bg;
    v.code_bg_color = p.bg_sunken;
    v.window_stroke = egui::Stroke::new(1.0, p.border);
    v.weak_text_color = Some(p.ink_muted);
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = egui::Stroke::new(1.0, p.accent);
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.warning;
    v.error_fg_color = p.danger;
    for widget in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        widget.corner_radius = CornerRadius::same(4);
        widget.fg_stroke = egui::Stroke::new(1.0, p.ink);
    }
    v.widgets.noninteractive.bg_fill = p.surface;
    v.widgets.noninteractive.weak_bg_fill = p.surface;
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, p.border);
    v.widgets.inactive.bg_fill = p.surface_raised;
    v.widgets.inactive.weak_bg_fill = p.surface;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, p.border_control);
    for widget in [&mut v.widgets.hovered, &mut v.widgets.active] {
        widget.bg_fill = p.accent_soft;
        widget.weak_bg_fill = p.accent_soft;
        widget.bg_stroke = egui::Stroke::new(1.0, p.accent);
    }
    v.widgets.open.bg_fill = p.bg;
    v.widgets.open.weak_bg_fill = p.bg;
    v.widgets.open.bg_stroke = egui::Stroke::new(1.0, p.border_control);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_corner_radius = CornerRadius::same(8);
    v.window_shadow = egui::Shadow::NONE;

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
    let palette = palette(theme);
    let color = |c: egui::Color32| {
        [
            f32::from(c.r()) / 255.0,
            f32::from(c.g()) / 255.0,
            f32::from(c.b()) / 255.0,
            1.0,
        ]
    };
    drydock_render::Style {
        background: color(palette.viewport_bg),
        default_line: color(palette.edge),
        default_face: color(palette.face_front),
        default_back: Some(color(palette.face_back)),
        sun: None,
    }
}

/// Set the colour fields of `style` (not `sun`) to those of `viewport_style(theme)`; true
/// when anything changed, so the viewport must redraw.
pub fn apply_viewport_theme(style: &mut drydock_render::Style, theme: Theme) -> bool {
    let wanted = viewport_style(theme);
    let changed = style.background != wanted.background
        || style.default_line != wanted.default_line
        || style.default_face != wanted.default_face
        || style.default_back != wanted.default_back;
    if changed {
        style.background = wanted.background;
        style.default_line = wanted.default_line;
        style.default_face = wanted.default_face;
        style.default_back = wanted.default_back;
    }
    changed
}

/// The colour of model axis `axis` (0 x, 1 y, 2 z) in `theme`: `palette(theme).axis_x` /
/// `axis_y` / `axis_z` as RGB bytes; any other index gives mid-grey `[160, 160, 160]`.
pub fn axis_color(theme: Theme, axis: usize) -> [u8; 3] {
    let palette = palette(theme);
    let color = match axis {
        0 => palette.axis_x,
        1 => palette.axis_y,
        2 => palette.axis_z,
        _ => egui::Color32::from_rgb(160, 160, 160),
    };
    [color.r(), color.g(), color.b()]
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
