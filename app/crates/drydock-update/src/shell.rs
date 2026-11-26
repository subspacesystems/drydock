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
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_owned();
    }
    let mut quoted = String::with_capacity(arg.len() + 2);
    quoted.push('"');
    let mut backslashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                quoted.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.extend(std::iter::repeat_n('\\', backslashes));
                quoted.push(c);
                backslashes = 0;
            }
        }
    }
    quoted.extend(std::iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
}

/// The arguments quoted with [`quote_arg`] and joined by single spaces (`[]` gives `""`).
pub fn join_parameters(args: &[&str]) -> String {
    args.iter()
        .map(|arg| quote_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

/// What a failed shell launch reports: [`LAUNCH_CANCELLED`] when `code` is [`ERROR_CANCELLED`];
/// otherwise `text` (the system's message) trimmed, or `"error {code}"` when that is empty.
pub fn launch_failure(code: u32, text: &str) -> String {
    if code == ERROR_CANCELLED {
        return LAUNCH_CANCELLED.to_owned();
    }
    let text = text.trim();
    if text.is_empty() {
        format!("error {code}")
    } else {
        text.to_owned()
    }
}
