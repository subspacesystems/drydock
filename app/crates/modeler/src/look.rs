// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The desktop look (`docs/specs/m6-native-look.md`): the OS's UI font loaded at run time,
//! light or dark following the OS, flat square-ish controls and standard spacing. Pure
//! functions over egui types so tests need no window.

use std::path::PathBuf;

use eframe::egui::{self, FontDefinitions, Style};

/// Roboto, bundled: the fallback when the OS font can't be read, and for glyphs it lacks.
#[allow(dead_code)]
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
pub fn theme_from(_system: Option<egui::Theme>) -> Theme {
    unimplemented!("m6-native-look")
}

/// The egui style for `theme`: egui's own colours for the scheme with the desktop look on
/// top. Widget corners at most 3 points, the hovered fill differs from the resting one,
/// item spacing 4 to 8 points tall, rows 20 to 28 points tall, button padding at least 6
/// points wide.
pub fn style(_theme: Theme) -> Style {
    unimplemented!("m6-native-look")
}

/// Where the OS keeps its UI font, best first. Absolute paths to `.ttf`, `.otf` or `.ttc`
/// files; Windows lists `segoeui.ttf`.
pub fn font_candidates(_os: Os) -> Vec<PathBuf> {
    unimplemented!("m6-native-look")
}

/// The bytes of the first of `paths` that exists and starts like a font file (TrueType,
/// OpenType, or a collection); `None` if there is none.
pub fn first_readable_font(_paths: &[PathBuf]) -> Option<Vec<u8>> {
    unimplemented!("m6-native-look")
}

/// The fonts to install: with `system` bytes, a font named "System" first in the
/// Proportional family followed by Roboto; without, Roboto alone. Monospace is always
/// Roboto. No egui default fonts.
pub fn font_definitions(_system: Option<Vec<u8>>) -> FontDefinitions {
    unimplemented!("m6-native-look")
}

/// Installs the fonts (the OS's UI font if readable) and the style for the OS's theme, both
/// the light and the dark one so a theme change in the OS applies at once.
pub fn apply(_ctx: &egui::Context) {
    unimplemented!("m6-native-look")
}
