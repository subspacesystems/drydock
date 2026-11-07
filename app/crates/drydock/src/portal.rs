// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Linux desktop's light/dark setting, read from the XDG desktop portal
//! (`org.freedesktop.appearance` / `color-scheme`) by running GLib's `gdbus`
//! (`docs/specs/m10-theme-linux-portal.md`).

use crate::look::Theme;
use eframe::egui;

/// The theme for a portal `color-scheme` value: 1 (prefer dark) Dark; 2 (prefer light) and
/// 0 (no preference) and any other number Light; `None` (the desktop has no setting, or
/// the answer could not be read) Dark.
pub fn theme_for_scheme(scheme: Option<u32>) -> Theme {
    match scheme {
        Some(1) => Theme::Dark,
        Some(_) => Theme::Light,
        None => Theme::Dark,
    }
}

/// The color-scheme number in the text `gdbus call ... Settings.ReadOne` prints, e.g.
/// `(<uint32 1>,)`, or the older `Read` form with a doubled variant, `(<<uint32 2>>,)`.
/// `None` for anything else (an error text, empty output, a non-number).
pub fn parse_read_reply(text: &str) -> Option<u32> {
    parse_uint32(text)
}

/// The color-scheme number in one line of `gdbus monitor` output, when the line is a
/// `SettingChanged` signal for namespace `org.freedesktop.appearance` and key
/// `color-scheme`, e.g.
/// `/org/freedesktop/portal/desktop: org.freedesktop.portal.Settings.SettingChanged ('org.freedesktop.appearance', 'color-scheme', <uint32 1>)`.
/// `None` for any other line (other keys or namespaces, other signals, noise).
pub fn parse_signal_line(line: &str) -> Option<u32> {
    const SIGNAL: &str = "org.freedesktop.portal.Settings.SettingChanged";
    const NAMESPACE: &str = "'org.freedesktop.appearance'";
    const KEY: &str = "'color-scheme'";
    if !line.contains(SIGNAL) || !line.contains(NAMESPACE) || !line.contains(KEY) {
        return None;
    }
    parse_uint32(line)
}

/// The first `<uint32 N>` in `text` as `N`; `None` when there is no such number.
fn parse_uint32(text: &str) -> Option<u32> {
    const MARKER: &str = "<uint32";
    let start = text.find(MARKER)? + MARKER.len();
    let rest = &text[start..];
    let end = rest.find('>')?;
    rest[..end].trim().parse().ok()
}

/// The desktop's current color-scheme number: runs `gdbus call --session` for `ReadOne`
/// (then `Read` when that fails) and parses the reply with `parse_read_reply`. Gives up
/// after about 2 seconds. `None` when `gdbus` is missing, there is no session bus or
/// portal, it errors or times out, and always off Linux.
pub fn read_scheme() -> Option<u32> {
    #[cfg(target_os = "linux")]
    {
        read_scheme_linux()
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// On Linux: start a background thread running `gdbus monitor --session` on
/// `org.freedesktop.portal.Desktop`; for each line `parse_signal_line` accepts, call
/// `look::set_fallback_theme(&ctx, theme_for_scheme(Some(n)))` and `ctx.request_repaint()`.
/// Returns at once; the thread ends when `gdbus` exits or is missing. Elsewhere: does nothing.
pub fn watch(ctx: egui::Context) {
    #[cfg(target_os = "linux")]
    {
        std::thread::spawn(move || watch_linux(ctx));
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = ctx;
    }
}

#[cfg(target_os = "linux")]
fn read_scheme_linux() -> Option<u32> {
    use std::time::{Duration, Instant};

    let started = Instant::now();
    let total = Duration::from_secs(2);
    for method in [
        "org.freedesktop.portal.Settings.ReadOne",
        "org.freedesktop.portal.Settings.Read",
    ] {
        let remaining = total.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            break;
        }
        let args = [
            "call",
            "--session",
            "--dest",
            "org.freedesktop.portal.Desktop",
            "--object-path",
            "/org/freedesktop/portal/desktop",
            "--method",
            method,
            "org.freedesktop.appearance",
            "color-scheme",
        ];
        if let Some(text) = gdbus_output(&args, remaining)
            && let Some(scheme) = parse_read_reply(&text)
        {
            return Some(scheme);
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn gdbus_output(args: &[&str], timeout: std::time::Duration) -> Option<String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::Instant;

    let mut child = Command::new("gdbus")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut text = String::new();
                child.stdout.take()?.read_to_string(&mut text).ok()?;
                return Some(text);
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn watch_linux(ctx: egui::Context) {
    use std::io::BufRead;
    use std::process::{Command, Stdio};

    let Ok(mut child) = Command::new("gdbus")
        .args([
            "monitor",
            "--session",
            "--dest",
            "org.freedesktop.portal.Desktop",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        return;
    };
    for line in std::io::BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        if let Some(scheme) = parse_signal_line(&line) {
            crate::look::set_fallback_theme(&ctx, theme_for_scheme(Some(scheme)));
            ctx.request_repaint();
        }
    }
    let _ = child.wait();
}
