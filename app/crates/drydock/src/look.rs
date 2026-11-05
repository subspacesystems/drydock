// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The desktop look (`docs/specs/m6-native-look.md`): the OS's UI font loaded at run time,
//! light or dark following the OS, flat square-ish controls and standard spacing. Pure
//! functions over egui types so tests need no window.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, CornerRadius, FontData, FontDefinitions, Margin, Style};

/// Roboto, bundled: the fallback when the OS font can't be read, and for glyphs it lacks.
pub const ROBOTO: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");

/// The colour scheme to draw with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

/// The operating systems whose UI fonts we know how to find.
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

/// Where the OS keeps its UI font, best first. Absolute paths to `.ttf`, `.otf` or `.ttc`
/// files; Windows lists `segoeui.ttf`.
pub fn font_candidates(os: Os) -> Vec<PathBuf> {
    let paths: &[&str] = match os {
        Os::Windows => &[
            r"C:\Windows\Fonts\segoeui.ttf",
            r"C:\Windows\Fonts\segoeuib.ttf",
            r"C:\Windows\Fonts\segoeuil.ttf",
            r"C:\Windows\Fonts\arial.ttf",
        ],
        Os::Mac => &[
            "/System/Library/Fonts/SFNS.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/Library/Fonts/Arial.ttf",
        ],
        Os::Linux => &[
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/opentype/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/truetype/cantarell/Cantarell-Regular.ttf",
            "/usr/share/fonts/truetype/ubuntu/Ubuntu-R.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ],
    };
    paths.iter().map(PathBuf::from).collect()
}

/// The bytes of the first of `paths` that exists and starts like a font file (TrueType,
/// OpenType, or a collection); `None` if there is none.
pub fn first_readable_font(paths: &[PathBuf]) -> Option<Vec<u8>> {
    paths.iter().find_map(|path| {
        let bytes = std::fs::read(path).ok()?;
        let signature = bytes.get(..4)?;
        (signature == [0, 1, 0, 0]
            || signature == b"OTTO"
            || signature == b"true"
            || signature == b"ttcf")
            .then_some(bytes)
    })
}

/// The fonts to install: with `system` bytes, a font named "System" first in the
/// Proportional family followed by Roboto; without, Roboto alone. Monospace is always
/// Roboto. No egui default fonts.
pub fn font_definitions(system: Option<Vec<u8>>) -> FontDefinitions {
    let mut fonts = FontDefinitions::empty();
    fonts
        .font_data
        .insert("Roboto".to_owned(), Arc::new(FontData::from_static(ROBOTO)));
    let proportional = if let Some(bytes) = system {
        fonts
            .font_data
            .insert("System".to_owned(), Arc::new(FontData::from_owned(bytes)));
        vec!["System".to_owned(), "Roboto".to_owned()]
    } else {
        vec!["Roboto".to_owned()]
    };
    fonts
        .families
        .insert(egui::FontFamily::Proportional, proportional);
    fonts
        .families
        .insert(egui::FontFamily::Monospace, vec!["Roboto".to_owned()]);
    fonts
}

/// Installs the fonts (the OS's UI font if readable) and the style for the OS's theme, both
/// the light and the dark one so a theme change in the OS applies at once.
pub fn apply(ctx: &egui::Context) {
    let system = first_readable_font(&font_candidates(Os::current()));
    ctx.set_fonts(font_definitions(system));
    ctx.set_style_of(egui::Theme::Light, style(Theme::Light));
    ctx.set_style_of(egui::Theme::Dark, style(Theme::Dark));
    ctx.set_theme(egui::ThemePreference::System);
}
