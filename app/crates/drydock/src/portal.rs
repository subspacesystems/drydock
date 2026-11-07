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
    let _ = scheme;
    unimplemented!("m10-theme-linux-portal")
}

/// The color-scheme number in the text `gdbus call ... Settings.ReadOne` prints, e.g.
/// `(<uint32 1>,)`, or the older `Read` form with a doubled variant, `(<<uint32 2>>,)`.
/// `None` for anything else (an error text, empty output, a non-number).
pub fn parse_read_reply(text: &str) -> Option<u32> {
    let _ = text;
    unimplemented!("m10-theme-linux-portal")
}

/// The color-scheme number in one line of `gdbus monitor` output, when the line is a
/// `SettingChanged` signal for namespace `org.freedesktop.appearance` and key
/// `color-scheme`, e.g.
/// `/org/freedesktop/portal/desktop: org.freedesktop.portal.Settings.SettingChanged ('org.freedesktop.appearance', 'color-scheme', <uint32 1>)`.
/// `None` for any other line (other keys or namespaces, other signals, noise).
pub fn parse_signal_line(line: &str) -> Option<u32> {
    let _ = line;
    unimplemented!("m10-theme-linux-portal")
}

/// The desktop's current color-scheme number: runs `gdbus call --session` for `ReadOne`
/// (then `Read` when that fails) and parses the reply with `parse_read_reply`. Gives up
/// after about 2 seconds. `None` when `gdbus` is missing, there is no session bus or
/// portal, it errors or times out, and always off Linux.
pub fn read_scheme() -> Option<u32> {
    unimplemented!("m10-theme-linux-portal")
}

/// On Linux: start a background thread running `gdbus monitor --session` on
/// `org.freedesktop.portal.Desktop`; for each line `parse_signal_line` accepts, call
/// `look::set_fallback_theme(&ctx, theme_for_scheme(Some(n)))` and `ctx.request_repaint()`.
/// Returns at once; the thread ends when `gdbus` exits or is missing. Elsewhere: does nothing.
pub fn watch(ctx: egui::Context) {
    let _ = ctx;
    unimplemented!("m10-theme-linux-portal")
}
