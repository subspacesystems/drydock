// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Print Setup: the printer bed (presets and a custom size), its outline in the viewport,
//! the fits-the-bed check, the overhang angle field and the settings file they are kept in.
//! No window needed except `print_setup_ui`. Behaviour: `docs/specs/m7-print-setup.md`.

use eframe::egui;
use modeler_kernel::model::{InstanceId, Model};
use modeler_render::ExtraLine;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Smallest bed side the fields accept, in millimeters.
pub const MIN_BED_MM: f64 = 10.0;
/// Largest bed side the fields accept, in millimeters.
pub const MAX_BED_MM: f64 = 2000.0;
/// Smallest overhang angle the field accepts, in degrees from vertical.
pub const MIN_OVERHANG_DEGREES: f64 = 1.0;
/// Largest overhang angle the field accepts, in degrees from vertical.
pub const MAX_OVERHANG_DEGREES: f64 = 89.0;
/// Color of the bed outline.
pub const BED_COLOR: [f32; 4] = [0.9, 0.55, 0.1, 1.0];
/// Width of the bed outline, in points.
pub const BED_WIDTH_PT: f32 = 2.0;

/// The print bed: a rectangle centred on the origin at z = 0, `x_mm` along the red axis and
/// `y_mm` along the green one, and an optional build height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bed {
    pub x_mm: f64,
    pub y_mm: f64,
    /// Build height; `None` means no limit.
    pub z_mm: Option<f64>,
}

impl Default for Bed {
    /// 220 x 220 mm, no height limit.
    fn default() -> Bed {
        Bed {
            x_mm: 220.0,
            y_mm: 220.0,
            z_mm: None,
        }
    }
}

/// A bed size offered as a button.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BedPreset {
    /// The button's label, for example `"220 x 220 mm"`.
    pub name: &'static str,
    pub x_mm: f64,
    pub y_mm: f64,
}

/// The presets, in the order the panel lists them.
pub const BED_PRESETS: [BedPreset; 5] = [
    BedPreset {
        name: "180 x 180 mm",
        x_mm: 180.0,
        y_mm: 180.0,
    },
    BedPreset {
        name: "220 x 220 mm",
        x_mm: 220.0,
        y_mm: 220.0,
    },
    BedPreset {
        name: "256 x 256 mm",
        x_mm: 256.0,
        y_mm: 256.0,
    },
    BedPreset {
        name: "300 x 300 mm",
        x_mm: 300.0,
        y_mm: 300.0,
    },
    BedPreset {
        name: "350 x 350 mm",
        x_mm: 350.0,
        y_mm: 350.0,
    },
];

/// `bed` with the sides of `preset`; the height limit is kept.
pub fn with_preset(bed: Bed, preset: &BedPreset) -> Bed {
    let _ = (bed, preset);
    unimplemented!("m7-print-setup")
}

/// A bed side from a text field: `text` trimmed, a plain decimal number in millimeters (`.`
/// as the separator) with an optional `mm` suffix, within `MIN_BED_MM..=MAX_BED_MM`; else
/// `None`.
pub fn parse_side_mm(text: &str) -> Option<f64> {
    let _ = text;
    unimplemented!("m7-print-setup")
}

/// The height field: blank (after trimming) is `Some(None)`, no limit; text that
/// `parse_side_mm` accepts is `Some(Some(mm))`; anything else is `None`.
pub fn parse_height_mm(text: &str) -> Option<Option<f64>> {
    let _ = text;
    unimplemented!("m7-print-setup")
}

/// `mm` for a field or message: up to 2 decimals, no trailing zeros or point
/// (`220.0` -> `"220"`, `0.25` -> `"0.25"`, `12.346` -> `"12.35"`).
pub fn format_mm(mm: f64) -> String {
    let _ = mm;
    unimplemented!("m7-print-setup")
}

/// The outline of `bed` as GPU lines in world meters: the four sides of the rectangle
/// `(-x/2, -y/2, 0)` to `(x/2, y/2, 0)` in the order bottom (y = -y/2, left to right), right,
/// top (right to left), left, each from corner to corner, solid, `BED_COLOR`, `width_px` =
/// `BED_WIDTH_PT * pixels_per_point` (a non-finite or non-positive `pixels_per_point` counts
/// as 1). The height limit is not drawn.
pub fn bed_outline(bed: &Bed, pixels_per_point: f32) -> Vec<ExtraLine> {
    let _ = (bed, pixels_per_point);
    unimplemented!("m7-print-setup")
}

/// The result of the fits-the-bed check.
#[derive(Clone, Debug, PartialEq)]
pub struct FitReport {
    /// Every checked solid fits. False when there was nothing to check.
    pub fits: bool,
    /// Width, depth and height in millimeters of the box around everything checked (axis
    /// aligned, world axes); `None` when there was nothing to check.
    pub size_mm: Option<[f64; 3]>,
    /// The line for the panel and the status bar; see `fit_report`.
    pub message: String,
    /// Groups and components left out because they are not solids.
    pub skipped: usize,
}

/// Does the model fit the bed? The solids are `modeler_mesh::print_mesh::print_mesh` with
/// `PrintScope::Model` when `instances` is `None`, else `PrintScope::Instances(instances)`,
/// default options. The box is taken over every triangle corner of every object. It fits
/// when each extent is at most the bed's side plus `1e-6` mm (x against `x_mm`, y against
/// `y_mm`, z against `z_mm` only when set); the box's position is not looked at, and parts
/// are not turned to fit. Messages, with `W x D x H` the extents and `BED` the bed, both
/// through `format_mm`, joined with " x " and ending " mm" (`BED` is two sides, or three
/// with a height limit):
/// - nothing solid: `"Nothing to check: no solid groups or components"`, `fits` false;
/// - fits: `"Fits the bed: W x D x H mm on BED mm"`;
/// - too big: `"Too big for the bed: W x D x H mm on BED mm (X over by 80 mm, Z over by 5 mm)"`
///   listing only the axes that are over, in X, Y, Z order, the overshoot through
///   `format_mm`.
///
/// When `skipped > 0` the message ends with `" ({n} not solid, left out)"`.
pub fn fit_report(model: &Model, instances: Option<&[InstanceId]>, bed: &Bed) -> FitReport {
    let _ = (model, instances, bed);
    unimplemented!("m7-print-setup")
}

/// What is kept between runs, in the file `print-setup` of the config folder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrintSettings {
    pub bed: Bed,
    pub show_bed: bool,
    pub overhang_degrees: f64,
}

impl Default for PrintSettings {
    /// The default bed, outline hidden, `modeler_dae::DEFAULT_OVERHANG_DEGREES`.
    fn default() -> PrintSettings {
        PrintSettings {
            bed: Bed::default(),
            show_bed: false,
            overhang_degrees: modeler_dae::DEFAULT_OVERHANG_DEGREES,
        }
    }
}

impl PrintSettings {
    /// The settings from the file's text, one record per line (`\r\n` also ends a line):
    /// `bed X Y` or `bed X Y Z` (mm), `show-bed yes` or `show-bed no`, `overhang DEGREES`.
    /// Numbers must pass `parse_side_mm` (bed) or lie within `MIN_OVERHANG_DEGREES..=
    /// MAX_OVERHANG_DEGREES`. A line that is unknown or has a bad value is ignored (that
    /// setting keeps its default); a setting repeated takes its last good line.
    pub fn parse(text: &str) -> PrintSettings {
        let _ = text;
        unimplemented!("m7-print-setup")
    }

    /// The file's text: `bed` (with the height only when set), `show-bed` and `overhang`
    /// lines in that order, numbers through `format_mm`, each line ending in `\n`.
    /// `parse(to_text())` gives the settings back (up to 2 decimals).
    pub fn to_text(&self) -> String {
        unimplemented!("m7-print-setup")
    }

    /// Read `file`; a missing or unreadable file gives the defaults.
    pub fn load(file: &Path) -> PrintSettings {
        let _ = file;
        unimplemented!("m7-print-setup")
    }

    /// Write `file` (creating its folder), through a temporary file and a rename as
    /// `RecentFiles::save` does.
    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        let _ = file;
        unimplemented!("m7-print-setup")
    }
}

/// The file the settings are kept in: `print-setup` in `recent::config_dir`.
pub fn settings_file(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    crate::recent::config_dir(var).map(|dir| dir.join("print-setup"))
}

/// What the Print Setup window shows.
#[derive(Clone, Debug, PartialEq)]
pub struct PrintSetupView<'a> {
    pub bed: Bed,
    pub show_bed: bool,
    pub overhang_degrees: f64,
    /// Groups or components are selected, so Drop to Bed has something to move.
    pub can_drop: bool,
    /// The last fits-the-bed result, if one was run.
    pub report: Option<&'a FitReport>,
}

/// What the user did in the Print Setup window this frame.
#[derive(Clone, Debug, PartialEq)]
pub enum PrintSetupEvent {
    None,
    /// A preset button or a committed custom side: the new bed.
    SetBed(Bed),
    /// The "Show bed outline" box was clicked: its new state.
    ShowBed(bool),
    /// A committed overhang angle, in degrees.
    SetOverhang(f64),
    /// "Check fit" was pressed.
    CheckFit,
    /// "Drop to Bed" was pressed.
    DropToBed,
}

/// Draw the window's contents in `ui` and say what happened. Layout, top to bottom, all
/// labels exact (tests find them):
/// - heading `"Printer bed"`, then one button per `BED_PRESETS` entry labelled by its `name`;
/// - three one-line text boxes labelled `"Bed X"`, `"Bed Y"` and `"Bed Z"`, each followed by
///   a `"mm"` label, holding `format_mm` of the bed's sides (Z blank when unset). Like
///   `chrome::clearance_field`: on losing focus or Enter after an edit, a good value
///   (`parse_side_mm`, `parse_height_mm`) is rewritten with `format_mm` and returned as
///   `SetBed`; a bad one is replaced by the current value and nothing is returned;
/// - a check box `"Show bed outline"` (`ShowBed`);
/// - a button `"Check fit"` (`CheckFit`) and, when `report` is set, its message as a label;
/// - a button `"Drop to Bed"` (`DropToBed`), disabled unless `can_drop`;
/// - a box `"Overhang angle"` followed by `"deg"`, committed the same way as the sides with
///   a value in `MIN_OVERHANG_DEGREES..=MAX_OVERHANG_DEGREES` returned as `SetOverhang`.
///
/// `texts` holds the four boxes' strings between frames; the caller keeps it and rewrites
/// it from the view whenever the view changes from outside (`PrintSetupTexts::for_view`).
pub fn print_setup_ui(
    ui: &mut egui::Ui,
    texts: &mut PrintSetupTexts,
    view: &PrintSetupView,
) -> PrintSetupEvent {
    let _ = (ui, texts, view);
    unimplemented!("m7-print-setup")
}

/// The strings in the Print Setup boxes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PrintSetupTexts {
    pub x: String,
    pub y: String,
    pub z: String,
    pub overhang: String,
}

impl PrintSetupTexts {
    /// The boxes' texts for `bed` and `overhang_degrees` (`format_mm`; Z blank when unset).
    pub fn for_view(bed: &Bed, overhang_degrees: f64) -> PrintSetupTexts {
        let _ = (bed, overhang_degrees);
        unimplemented!("m7-print-setup")
    }
}
