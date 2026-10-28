// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The desktop look (`docs/specs/m6-native-look.md`): the OS's UI font loaded at run time,
//! light or dark following the OS, flat square-ish controls and standard spacing. Pure
//! functions over egui types so tests need no window.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::{self, CornerRadius, FontData, FontDefinitions, Margin, Style};

/// Roboto, bundled: the fallback when the OS font can't be read, and for glyphs it lacks.
const ROBOTO: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");

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
