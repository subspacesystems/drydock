// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! modeler: the viewer window, or a headless screenshot with `--screenshot`.

mod app;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use modeler::document::Document;
use modeler::viewstate::{ViewState, scene_bounds};
use modeler_render::{Style, render_headless};
use modeler_view::StandardView;

const SCREENSHOT_SIZE: (u32, u32) = (1280, 800);

fn usage() -> String {
    format!(
        "{}\n\n\
         Usage: modeler [FILE]\n\
         \x20      modeler --screenshot OUT.png FILE\n\
         \x20      modeler --convert OUT.modeler FILE\n\n\
         FILE is a .dae or .modeler file.\n\n\
         Options:\n\
         \x20 --screenshot OUT.png  Render FILE once without a window (iso view, zoom\n\
         \x20                       extents, {}x{}), write OUT.png and exit\n\
         \x20 --convert OUT.modeler Convert FILE to a native .modeler file and exit\n\
         \x20 -h, --help            Show this help\n\
         \x20 -V, --version         Show the version",
        modeler::banner(),
        SCREENSHOT_SIZE.0,
        SCREENSHOT_SIZE.1
    )
}

struct Args {
    file: Option<PathBuf>,
    screenshot: Option<PathBuf>,
    convert: Option<PathBuf>,
}

enum Parsed {
    Run(Args),
    Exit(ExitCode),
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Parsed {
    let mut parsed = Args {
        file: None,
        screenshot: None,
        convert: None,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{}", usage());
                return Parsed::Exit(ExitCode::SUCCESS);
            }
            "-V" | "--version" => {
                println!("{}", modeler::banner());
                return Parsed::Exit(ExitCode::SUCCESS);
            }
            "--screenshot" => match args.next() {
                Some(out) => parsed.screenshot = Some(PathBuf::from(out)),
                None => return usage_error("--screenshot needs an output path"),
            },
            "--convert" => match args.next() {
                Some(out) => parsed.convert = Some(PathBuf::from(out)),
                None => return usage_error("--convert needs an output path"),
            },
            _ if arg.starts_with('-') && arg.len() > 1 => {
                return usage_error(&format!("unknown option {arg}"));
            }
            _ if parsed.file.is_some() => return usage_error("only one file can be opened"),
            _ => parsed.file = Some(PathBuf::from(arg)),
        }
    }
    if parsed.convert.is_some() && parsed.screenshot.is_some() {
        return usage_error("--convert and --screenshot cannot be used together");
    }
    if parsed.convert.is_some() && parsed.file.is_none() {
        return usage_error("--convert needs a FILE");
    }
    Parsed::Run(parsed)
}

fn usage_error(message: &str) -> Parsed {
    eprintln!("modeler: {message}\n\n{}", usage());
    Parsed::Exit(ExitCode::from(2))
}

/// Load `file`, render it headless in the iso view framed to the model, write `out`.
fn convert(file: &Path, out: &Path) -> Result<PathBuf, String> {
    let mut document = Document::open(file)?;
    document.save_as(out)
}

fn screenshot(file: &Path, out: &Path) -> Result<(), String> {
    let document = Document::open(file)?;
    let (width, height) = SCREENSHOT_SIZE;
    let mut view = ViewState::new();
    view.camera.set_standard_view(StandardView::Iso);
    view.set_bounds(
        scene_bounds(&document.scene),
        (f64::from(width), f64::from(height)),
    );
    let (image, warnings) = render_headless(
        &document.scene,
        &view.camera,
        width,
        height,
        &Style::default(),
    )
    .map_err(|err| err.to_string())?;
    for warning in warnings {
        eprintln!("modeler: warning: {warning}");
    }
    image
        .save(out)
        .map_err(|err| format!("{}: {err}", out.display()))
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Parsed::Run(args) => args,
        Parsed::Exit(code) => return code,
    };
    if let Some(out) = &args.convert {
        let Some(file) = &args.file else {
            eprintln!("modeler: --convert needs a FILE\n\n{}", usage());
            return ExitCode::from(2);
        };
        return match convert(file, out) {
            Ok(path) => {
                println!("wrote {}", path.display());
                ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("modeler: {message}");
                ExitCode::FAILURE
            }
        };
    }
    if let Some(out) = &args.screenshot {
        let Some(file) = &args.file else {
            eprintln!("modeler: --screenshot needs a FILE\n\n{}", usage());
            return ExitCode::from(2);
        };
        return match screenshot(file, out) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("modeler: {message}");
                ExitCode::FAILURE
            }
        };
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("modeler")
            .with_inner_size([1280.0, 800.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    let file = args.file;
    let result = eframe::run_native(
        "modeler",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, file)))),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("modeler: {err}");
            ExitCode::FAILURE
        }
    }
}
