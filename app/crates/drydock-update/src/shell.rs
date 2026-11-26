// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Starting the installer through the Windows shell: the pure parts (no platform code, so they
//! run in tests everywhere). Behaviour: `docs/specs/m13-update-launch-fix.md`.

/// The reason string a launcher returns when the user declined the elevation prompt. The
/// update window turns it into the calm status line "Update cancelled".
pub const LAUNCH_CANCELLED: &str = "cancelled";

/// The Windows error code for a declined elevation prompt (`ERROR_CANCELLED`).
pub const ERROR_CANCELLED: u32 = 1223;

/// One argument as the Windows command-line rules (`CommandLineToArgvW`) want it: unchanged
/// when it is non-empty and holds no space, tab or double quote; otherwise in double quotes,
/// with each double quote preceded by a backslash and each run of backslashes that comes
/// before a double quote (or the closing quote) doubled.
pub fn quote_arg(arg: &str) -> String {
    let _ = arg;
    unimplemented!("m13-update-launch-fix")
}

/// The arguments quoted with [`quote_arg`] and joined by single spaces (`[]` gives `""`).
pub fn join_parameters(args: &[&str]) -> String {
    let _ = args;
    unimplemented!("m13-update-launch-fix")
}

/// What a failed shell launch reports: [`LAUNCH_CANCELLED`] when `code` is [`ERROR_CANCELLED`];
/// otherwise `text` (the system's message) trimmed, or `"error {code}"` when that is empty.
pub fn launch_failure(code: u32, text: &str) -> String {
    let _ = (code, text);
    unimplemented!("m13-update-launch-fix")
}
