// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! modeler: the viewer window, or a headless screenshot with `--screenshot`.

#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use drydock::document::Document;
use drydock::shading;
use drydock::viewstate::{ViewState, scene_bounds};
use drydock_render::{Style, render_headless};
use drydock_view::StandardView;

const SCREENSHOT_SIZE: (u32, u32) = (1280, 800);

fn usage() -> String {
    format!(
        "{}\n\n\
         Usage: drydock [FILE]\n\
         \x20      drydock --screenshot OUT.png FILE\n\
         \x20      drydock --convert OUT.drydock FILE\n\n\
         FILE is a .dae, .drydock or .modeler file.\n\n\
         Options:\n\
         \x20 --screenshot OUT.png  Render FILE once without a window (iso view, zoom\n\
         \x20                       extents, {}x{}), write OUT.png and exit\n\
         \x20 --convert OUT.drydock Convert FILE to a native .drydock file and exit\n\
         \x20 -h, --help            Show this help\n\
         \x20 -V, --version         Show the version",
        drydock::banner(),
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
                println!("{}", drydock::banner());
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
    eprintln!("Drydock: {message}\n\n{}", usage());
    Parsed::Exit(ExitCode::from(2))
}

/// Open `file` and save it as a native file at `native_path(out)`. Returns the path written.
fn convert(file: &Path, out: &Path) -> Result<PathBuf, String> {
    let mut document = Document::open(file)?;
    document.save_as(out)
}

/// Load `file`, render it headless in the iso view framed to the model, write `out`.
fn screenshot(file: &Path, out: &Path) -> Result<(), String> {
    let document = Document::open(file)?;
    let (width, height) = SCREENSHOT_SIZE;
    let mut view = ViewState::new();
    view.camera.set_standard_view(StandardView::Iso);
    view.set_bounds(
        scene_bounds(&document.scene),
        (f64::from(width), f64::from(height)),
    );
    let style = Style {
        sun: document.kernel.as_ref().and_then(shading::sun_light),
        ..Style::default()
    };
    let (image, warnings) = render_headless(&document.scene, &view.camera, width, height, &style)
        .map_err(|err| err.to_string())?;
    for warning in warnings {
        eprintln!("Drydock: warning: {warning}");
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
            eprintln!("Drydock: --convert needs a FILE\n\n{}", usage());
            return ExitCode::from(2);
        };
        return match convert(file, out) {
            Ok(path) => {
                println!("wrote {}", path.display());
                ExitCode::SUCCESS
            }
            Err(message) => {
                eprintln!("Drydock: {message}");
                ExitCode::FAILURE
            }
        };
    }
    if let Some(out) = &args.screenshot {
        let Some(file) = &args.file else {
            eprintln!("Drydock: --screenshot needs a FILE\n\n{}", usage());
            return ExitCode::from(2);
        };
        return match screenshot(file, out) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("Drydock: {message}");
                ExitCode::FAILURE
            }
        };
    }
    drydock::desktop_entry::install_for_current_user();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Drydock")
            .with_inner_size([1280.0, 800.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    let file = args.file;
    let result = eframe::run_native(
        "drydock",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, file)))),
    );
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Drydock: {err}");
            ExitCode::FAILURE
        }
    }
}
