// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! `ShellExecuteExW` with the "runas" verb. Behaviour: `docs/specs/m13-update-launch-fix.md`.

#![allow(unsafe_code)] // ShellExecuteExW; docs/adr/009-winhttp-unsafe.md

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::UI::Shell::{
    SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::PCWSTR;

use crate::shell::{join_parameters, launch_failure};

/// Starts `installer` with `args` through the shell, elevated ("runas"): the user sees the
/// UAC prompt. Returns once the process has started (does not wait for it to finish). Errors
/// are `launch_failure(code, system message)`.
pub fn run_elevated(installer: &Path, args: &[&str]) -> Result<(), String> {
    let file = wide(installer.as_os_str().encode_wide());
    let verb = wide("runas".encode_utf16());
    let parameters = wide(join_parameters(args).encode_utf16());
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialised and the strings it points at outlive the call.
    match unsafe { ShellExecuteExW(&mut info) } {
        Ok(()) => {
            // SAFETY: the shell set `hProcess` because of SEE_MASK_NOCLOSEPROCESS; we own it.
            if !info.hProcess.is_invalid() {
                let _ = unsafe { CloseHandle(info.hProcess) };
            }
            Ok(())
        }
        Err(error) => {
            let code = error.code().0 as u32 & 0xFFFF;
            Err(launch_failure(code, &error.message()))
        }
    }
}

fn wide(value: impl Iterator<Item = u16>) -> Vec<u16> {
    value.chain(std::iter::once(0)).collect()
}
