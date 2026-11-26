// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! `ShellExecuteExW` with the "runas" verb. Behaviour: `docs/specs/m13-update-launch-fix.md`.

#![allow(unsafe_code)] // ShellExecuteExW; docs/adr/009-winhttp-unsafe.md

use std::path::Path;

/// Starts `installer` with `args` through the shell, elevated ("runas"): the user sees the
/// UAC prompt. Returns once the process has started (does not wait for it to finish). Errors
/// are `launch_failure(code, system message)`.
pub fn run_elevated(installer: &Path, args: &[&str]) -> Result<(), String> {
    let _ = (installer, args);
    unimplemented!("m13-update-launch-fix")
}
