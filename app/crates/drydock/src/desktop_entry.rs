// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Linux only in practice: the desktop entry and hicolor icons that let the dock and task
//! switcher show Drydock's icon (GNOME matches the window's app id to `<APP_ID>.desktop`).
//! Everything except [`install_for_current_user`] is pure or takes the data directory as an
//! argument, so it is tested on a temp dir and on any OS.
//!
//! Behaviour: `docs/specs/m9-linux-desktop-entry.md`.

use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};

use crate::window_icon::{APP_ID, ICON_PNG};

/// Icon sizes written under `icons/hicolor/<n>x<n>/apps/`, in this order. The 256 px icon is
/// `window_icon::ICON_PNG` itself; the others are scaled down from it.
pub const ICON_SIZES: [u32; 3] = [48, 128, 256];

/// One file the app keeps up to date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopFile {
    /// Absolute path: `data_dir` joined with the relative path in the spec.
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

/// What [`install`] did. Files that were already right are in neither list.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub written: Vec<PathBuf>,
    /// Path and the error text.
    pub failed: Vec<(PathBuf, String)>,
}

/// The user data directory: `xdg_data_home` when it is set, not empty and absolute, else
/// `<home>/.local/share` when `home` is set and not empty, else `None`.
pub fn data_dir_from(xdg_data_home: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
    if let Some(xdg) = xdg_data_home.filter(|s| !s.is_empty()) {
        let path = Path::new(xdg);
        if path.is_absolute() {
            return Some(path.to_path_buf());
        }
    }
    home.filter(|s| !s.is_empty())
        .map(|home| Path::new(home).join(".local/share"))
}

/// The `Exec` value for the program at `exe`: the path as one double-quoted argument
/// followed by ` %f`, escaped as the spec describes (quoting rules, then the file's
/// string-escape rule, then `%` doubled).
pub fn exec_line(exe: &Path) -> String {
    let path = exe.to_string_lossy();
    let mut quoted = String::with_capacity(path.len() + 2);
    quoted.push('"');
    for ch in path.chars() {
        match ch {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(ch);
            }
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    let mut escaped = String::with_capacity(quoted.len());
    for ch in quoted.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '%' => escaped.push_str("%%"),
            _ => escaped.push(ch),
        }
    }
    escaped.push_str(" %f");
    escaped
}

/// The text of `<APP_ID>.desktop` for the program at `exe`.
pub fn desktop_entry_text(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Drydock\n\
         Comment=3D modeling for architecture and woodworking\n\
         Exec={}\n\
         Icon={APP_ID}\n\
         Terminal=false\n\
         Categories=Graphics;3DGraphics;\n\
         StartupWMClass={APP_ID}\n",
        exec_line(exe)
    )
}

/// Every file to keep up to date under `data_dir`, with its wanted bytes: the desktop entry
/// first, then the icons in [`ICON_SIZES`] order.
pub fn files_for(data_dir: &Path, exe: &Path) -> Vec<DesktopFile> {
    let mut files = vec![DesktopFile {
        path: data_dir.join(format!("applications/{APP_ID}.desktop")),
        bytes: desktop_entry_text(exe).into_bytes(),
    }];
    let icon = image::load_from_memory_with_format(ICON_PNG, image::ImageFormat::Png)
        .expect("the embedded icon is a valid PNG");
    for n in ICON_SIZES {
        let bytes = if n == 256 {
            ICON_PNG.to_vec()
        } else {
            let resized =
                image::imageops::resize(&icon, n, n, image::imageops::FilterType::Lanczos3);
            let image = image::DynamicImage::ImageRgba8(resized);
            let mut bytes = Vec::new();
            image
                .write_to(
                    &mut std::io::Cursor::new(&mut bytes),
                    image::ImageFormat::Png,
                )
                .expect("the resized icon encodes as PNG");
            bytes
        };
        files.push(DesktopFile {
            path: data_dir.join(format!("icons/hicolor/{n}x{n}/apps/{APP_ID}.png")),
            bytes,
        });
    }
    files
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_default();
    let temp = path.with_file_name(format!(".{name}.tmp-{}", std::process::id()));
    let result = std::fs::write(&temp, bytes).and_then(|()| std::fs::rename(&temp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

/// Write each file of [`files_for`] that is missing or whose bytes differ, creating the
/// folders it needs. Never touches other files, never panics, never stops at a failure: the
/// failed file goes in `failed` and the rest are still tried.
pub fn install(data_dir: &Path, exe: &Path) -> Report {
    let mut report = Report::default();
    for file in files_for(data_dir, exe) {
        if std::fs::read(&file.path).is_ok_and(|existing| existing == file.bytes) {
            continue;
        }
        match write_atomic(&file.path, &file.bytes) {
            Ok(()) => report.written.push(file.path),
            Err(error) => report.failed.push((file.path, error.to_string())),
        }
    }
    report
}

/// What `main` calls at startup on a normal run: on Linux, [`install`] into the directory
/// from [`data_dir_from`] (environment `XDG_DATA_HOME`, `HOME`) for `std::env::current_exe()`,
/// then, only when something was written, best-effort `update-desktop-database` and
/// `gtk-update-icon-cache` (ignored when not installed or failing). Failures are printed to
/// stderr as one line each, never returned. Does nothing on other systems.
#[cfg(target_os = "linux")]
pub fn install_for_current_user() {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => {
            eprintln!("Drydock: could not find its executable: {error}");
            return;
        }
    };
    let xdg_data_home = std::env::var("XDG_DATA_HOME").ok();
    let home = std::env::var("HOME").ok();
    let Some(data_dir) = data_dir_from(xdg_data_home.as_deref(), home.as_deref()) else {
        eprintln!("Drydock: no home directory for the desktop entry");
        return;
    };
    let report = install(&data_dir, &exe);
    for (path, error) in &report.failed {
        eprintln!("Drydock: could not write {}: {error}", path.display());
    }
    if report.written.is_empty() {
        return;
    }
    let _ = Command::new("update-desktop-database")
        .arg(data_dir.join("applications"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = Command::new("gtk-update-icon-cache")
        .args(["-f", "-t"])
        .arg(data_dir.join("icons/hicolor"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(not(target_os = "linux"))]
pub fn install_for_current_user() {}
