// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use std::path::Path;
use std::sync::atomic::AtomicBool;

/// Why a request failed. The updater treats every variant as "no update now".
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportError {
    /// No network, DNS failure, connection refused or timed out.
    Offline,
    /// The server answered with this non-success status.
    Http(u16),
    /// The body is, or would be, longer than the limit given to the call.
    TooLarge,
    /// The cancel flag was set during the call.
    Cancelled,
    /// Writing the destination file failed.
    Io(std::io::ErrorKind),
    Other(String),
}

/// How the updater reaches the network. Tests use a fake; Windows gets WinHTTP
/// (`m13-update-winhttp`). Every call is blocking and runs on the updater's thread. The
/// implementation sends exactly `headers` and adds nothing of its own that identifies the
/// user or machine (a fixed generic `User-Agent` is the only exception).
pub trait Transport: Send + Sync {
    /// GET `url` and return the body. Fails with `TooLarge` when the body exceeds `max_bytes`.
    fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
        max_bytes: u64,
    ) -> Result<Vec<u8>, TransportError>;

    /// GET `url` into the file `dest` (created or truncated). `progress` is called with the
    /// total bytes written so far, after each chunk. Stops with `Cancelled` soon after `cancel`
    /// becomes true and with `TooLarge` when more than `max_bytes` would be written. On any
    /// error the file may be left half written: the caller deletes it.
    fn download(
        &self,
        url: &str,
        headers: &[(String, String)],
        max_bytes: u64,
        dest: &Path,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), TransportError>;
}
