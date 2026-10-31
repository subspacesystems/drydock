// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Print Setup: the printer bed (presets and a custom size), its outline in the viewport,
//! the fits-the-bed check, the overhang angle field and the settings file they are kept in.
//! No window needed except `print_setup_ui`. Behaviour: `docs/specs/m7-print-setup.md`.

use eframe::egui;
use glam::DVec3;
use modeler_kernel::model::{InstanceId, Model};
use modeler_mesh::print_mesh::{self, PrintOptions, PrintScope};
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
    Bed {
        x_mm: preset.x_mm,
        y_mm: preset.y_mm,
        z_mm: bed.z_mm,
    }
}

/// A bed side from a text field: `text` trimmed, a plain decimal number in millimeters (`.`
/// as the separator) with an optional `mm` suffix, within `MIN_BED_MM..=MAX_BED_MM`; else
/// `None`.
pub fn parse_side_mm(text: &str) -> Option<f64> {
    let text = text.trim();
    let number = text.strip_suffix("mm").map(str::trim_end).unwrap_or(text);
    if number.is_empty()
        || number.starts_with('+')
        || number.starts_with('-')
        || number.contains('e')
        || number.contains('E')
        || number.chars().filter(|&c| c == '.').count() > 1
        || !number.chars().all(|c| c.is_ascii_digit() || c == '.')
        || !number.chars().any(|c| c.is_ascii_digit())
    {
        return None;
    }
    let mm = number.parse::<f64>().ok()?;
    (MIN_BED_MM..=MAX_BED_MM).contains(&mm).then_some(mm)
}

/// The height field: blank (after trimming) is `Some(None)`, no limit; text that
/// `parse_side_mm` accepts is `Some(Some(mm))`; anything else is `None`.
pub fn parse_height_mm(text: &str) -> Option<Option<f64>> {
    if text.trim().is_empty() {
        Some(None)
    } else {
        parse_side_mm(text).map(Some)
    }
}

/// `mm` for a field or message: up to 2 decimals, no trailing zeros or point
/// (`220.0` -> `"220"`, `0.25` -> `"0.25"`, `12.346` -> `"12.35"`).
pub fn format_mm(mm: f64) -> String {
    let text = format!("{mm:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".to_string()
    } else {
        text.to_string()
    }
}

/// The outline of `bed` as GPU lines in world meters: the four sides of the rectangle
/// `(-x/2, -y/2, 0)` to `(x/2, y/2, 0)` in the order bottom (y = -y/2, left to right), right,
/// top (right to left), left, each from corner to corner, solid, `BED_COLOR`, `width_px` =
/// `BED_WIDTH_PT * pixels_per_point` (a non-finite or non-positive `pixels_per_point` counts
/// as 1). The height limit is not drawn.
pub fn bed_outline(bed: &Bed, pixels_per_point: f32) -> Vec<ExtraLine> {
    let ppp = if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
        pixels_per_point
    } else {
        1.0
    };
    let width_px = BED_WIDTH_PT * ppp;
    let x = bed.x_mm / 2000.0;
    let y = bed.y_mm / 2000.0;
    let corners = [
        DVec3::new(-x, -y, 0.0),
        DVec3::new(x, -y, 0.0),
        DVec3::new(x, y, 0.0),
        DVec3::new(-x, y, 0.0),
    ];
    (0..4)
        .map(|i| ExtraLine {
            a: corners[i],
            b: corners[(i + 1) % 4],
            color: BED_COLOR,
            width_px,
            dash_px: None,
        })
        .collect()
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
    let scope = instances.map_or(PrintScope::Model, PrintScope::Instances);
    let mesh = print_mesh::print_mesh(model, scope, &PrintOptions::default());
    let skipped = mesh.skipped.len();
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    for object in &mesh.objects {
        for triangle in &object.triangles {
            for &corner in triangle {
                lo = lo.min(corner);
                hi = hi.max(corner);
            }
        }
    }
    let suffix = if skipped > 0 {
        format!(" ({skipped} not solid, left out)")
    } else {
        String::new()
    };
    if !lo.is_finite() || !hi.is_finite() {
        return FitReport {
            fits: false,
            size_mm: None,
            message: format!("Nothing to check: no solid groups or components{suffix}"),
            skipped,
        };
    }
    let size = hi - lo;
    let size_mm = [size.x, size.y, size.z];
    let mut bed_text = format!("{} x {}", format_mm(bed.x_mm), format_mm(bed.y_mm));
    if let Some(z) = bed.z_mm {
        bed_text.push_str(&format!(" x {}", format_mm(z)));
    }
    let size_text = size_mm.map(format_mm).join(" x ");
    let mut over = Vec::new();
    let limits = [Some(bed.x_mm), Some(bed.y_mm), bed.z_mm];
    let names = ["X", "Y", "Z"];
    for (axis, limit) in limits.into_iter().enumerate() {
        if let Some(limit) = limit {
            let amount = size_mm[axis] - limit;
            if amount > 1e-6 {
                over.push(format!("{} over by {} mm", names[axis], format_mm(amount)));
            }
        }
    }
    let fits = over.is_empty();
    let message = if fits {
        format!("Fits the bed: {size_text} mm on {bed_text} mm{suffix}")
    } else {
        format!(
            "Too big for the bed: {size_text} mm on {bed_text} mm ({}){suffix}",
            over.join(", ")
        )
    };
    FitReport {
        fits,
        size_mm: Some(size_mm),
        message,
        skipped,
    }
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
        let mut settings = PrintSettings::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            match parts.as_slice() {
                ["bed", x, y] => {
                    if let (Some(x), Some(y)) = (parse_side_mm(x), parse_side_mm(y)) {
                        settings.bed = Bed {
                            x_mm: x,
                            y_mm: y,
                            z_mm: None,
                        };
                    }
                }
                ["bed", x, y, z] => {
                    if let (Some(x), Some(y), Some(z)) =
                        (parse_side_mm(x), parse_side_mm(y), parse_side_mm(z))
                    {
                        settings.bed = Bed {
                            x_mm: x,
                            y_mm: y,
                            z_mm: Some(z),
                        };
                    }
                }
                ["show-bed", value] => match *value {
                    "yes" => settings.show_bed = true,
                    "no" => settings.show_bed = false,
                    _ => {}
                },
                ["overhang", value] => {
                    if let Ok(degrees) = value.parse::<f64>()
                        && (MIN_OVERHANG_DEGREES..=MAX_OVERHANG_DEGREES).contains(&degrees)
                    {
                        settings.overhang_degrees = degrees;
                    }
                }
                _ => {}
            }
        }
        settings
    }

    /// The file's text: `bed` (with the height only when set), `show-bed` and `overhang`
    /// lines in that order, numbers through `format_mm`, each line ending in `\n`.
    /// `parse(to_text())` gives the settings back (up to 2 decimals).
    pub fn to_text(&self) -> String {
        let mut text = format!(
            "bed {} {}",
            format_mm(self.bed.x_mm),
            format_mm(self.bed.y_mm)
        );
        if let Some(z) = self.bed.z_mm {
            text.push_str(&format!(" {}", format_mm(z)));
        }
        text.push('\n');
        text.push_str(if self.show_bed {
            "show-bed yes\n"
        } else {
            "show-bed no\n"
        });
        text.push_str(&format!("overhang {}\n", format_mm(self.overhang_degrees)));
        text
    }

    /// Read `file`; a missing or unreadable file gives the defaults.
    pub fn load(file: &Path) -> PrintSettings {
        std::fs::read_to_string(file)
            .map(|text| PrintSettings::parse(&text))
            .unwrap_or_default()
    }

    /// Write `file` (creating its folder), through a temporary file and a rename as
    /// `RecentFiles::save` does.
    pub fn save(&self, file: &Path) -> std::io::Result<()> {
        let folder = file.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(folder)?;
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "print-setup".to_owned());
        let temp = folder.join(format!(".{name}.tmp"));
        std::fs::write(&temp, self.to_text())?;
        std::fs::rename(&temp, file)
    }
}

/// The file the settings are kept in: `print-setup` in `recent::config_dir`.
pub fn settings_file(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    crate::recent::config_dir(var).map(|dir| dir.join("print-setup"))
}

/// Gap in points between the free area's top-left corner and the Print Setup window.
pub const WINDOW_MARGIN: f32 = 16.0;

/// Where the Print Setup window first opens: the top-left corner of `available` (the area
/// left once the menu bar and toolbars are laid out: the viewport's rect) moved
/// right and down by `WINDOW_MARGIN`, so the menus stay clickable.
pub fn window_pos(available: egui::Rect) -> egui::Pos2 {
    let _ = available;
    unimplemented!("m7-fix-dialogs")
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

fn field<T: Copy>(
    ui: &mut egui::Ui,
    label: &str,
    unit: &str,
    text: &mut String,
    current: &str,
    parse: impl Fn(&str) -> Option<T>,
    format: impl Fn(T) -> String,
) -> Option<T> {
    let edited_id = ui.make_persistent_id(("print_setup_field", label));
    let mut edited = ui
        .data(|data| data.get_temp::<bool>(edited_id))
        .unwrap_or(false);
    let mut committed = None;
    ui.horizontal(|ui| {
        ui.label(label);
        let response = ui.add(egui::TextEdit::singleline(text).desired_width(48.0));
        ui.label(unit);
        edited |= response.changed();
        if edited && response.lost_focus() {
            edited = false;
            if let Some(value) = parse(text) {
                *text = format(value);
                committed = Some(value);
            } else {
                *text = current.to_string();
            }
        }
    });
    ui.data_mut(|data| data.insert_temp(edited_id, edited));
    committed
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
    let mut event = PrintSetupEvent::None;
    ui.heading("Printer bed");
    for preset in &BED_PRESETS {
        if ui.button(preset.name).clicked() {
            event = PrintSetupEvent::SetBed(with_preset(view.bed, preset));
        }
    }
    let mut bed = view.bed;
    let mut bed_changed = false;
    let x_current = format_mm(view.bed.x_mm);
    if let Some(x) = field(
        ui,
        "Bed X",
        "mm",
        &mut texts.x,
        &x_current,
        parse_side_mm,
        format_mm,
    ) {
        bed.x_mm = x;
        bed_changed = true;
    }
    let y_current = format_mm(view.bed.y_mm);
    if let Some(y) = field(
        ui,
        "Bed Y",
        "mm",
        &mut texts.y,
        &y_current,
        parse_side_mm,
        format_mm,
    ) {
        bed.y_mm = y;
        bed_changed = true;
    }
    let z_current = view.bed.z_mm.map(format_mm).unwrap_or_default();
    if let Some(z) = field(
        ui,
        "Bed Z",
        "mm",
        &mut texts.z,
        &z_current,
        parse_height_mm,
        |z| z.map(format_mm).unwrap_or_default(),
    ) {
        bed.z_mm = z;
        bed_changed = true;
    }
    if bed_changed {
        event = PrintSetupEvent::SetBed(bed);
    }
    let mut show_bed = view.show_bed;
    if ui.checkbox(&mut show_bed, "Show bed outline").changed() {
        event = PrintSetupEvent::ShowBed(show_bed);
    }
    if ui.button("Check fit").clicked() {
        event = PrintSetupEvent::CheckFit;
    }
    if let Some(report) = view.report {
        ui.label(&report.message);
    }
    if ui
        .add_enabled(view.can_drop, egui::Button::new("Drop to Bed"))
        .clicked()
    {
        event = PrintSetupEvent::DropToBed;
    }
    let overhang_current = format_mm(view.overhang_degrees);
    if let Some(degrees) = field(
        ui,
        "Overhang angle",
        "deg",
        &mut texts.overhang,
        &overhang_current,
        |text| {
            text.trim()
                .parse::<f64>()
                .ok()
                .filter(|d| (MIN_OVERHANG_DEGREES..=MAX_OVERHANG_DEGREES).contains(d))
        },
        format_mm,
    ) {
        event = PrintSetupEvent::SetOverhang(degrees);
    }
    event
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
        PrintSetupTexts {
            x: format_mm(bed.x_mm),
            y: format_mm(bed.y_mm),
            z: bed.z_mm.map(format_mm).unwrap_or_default(),
            overhang: format_mm(overhang_degrees),
        }
    }
}
