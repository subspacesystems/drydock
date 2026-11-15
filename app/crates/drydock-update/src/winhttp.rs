// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Windows transport over WinHTTP. Behaviour: `docs/specs/m13-update-winhttp.md`.

#![allow(unsafe_code)] // WinHTTP calls; docs/adr/009-winhttp-unsafe.md

use std::path::Path;
use std::sync::atomic::AtomicBool;

use crate::{Transport, TransportError};

/// Stateless: every call opens its own WinHTTP session and closes it before returning.
#[derive(Clone, Copy, Debug, Default)]
pub struct WinHttpTransport;

impl WinHttpTransport {
    pub fn new() -> WinHttpTransport {
        WinHttpTransport
    }
}

impl Transport for WinHttpTransport {
    fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
        max_bytes: u64,
    ) -> Result<Vec<u8>, TransportError> {
        let _ = (url, headers, max_bytes);
        unimplemented!("m13-update-winhttp")
    }

    fn download(
        &self,
        url: &str,
        headers: &[(String, String)],
        max_bytes: u64,
        dest: &Path,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), TransportError> {
        let _ = (url, headers, max_bytes, dest, cancel, progress);
        unimplemented!("m13-update-winhttp")
    }
}
