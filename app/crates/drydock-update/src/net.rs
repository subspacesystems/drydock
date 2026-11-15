// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Platform-neutral rules of the Windows transport (`winhttp.rs`), kept here so Linux tests
//! cover them. Behaviour: `docs/specs/m13-update-winhttp.md`.

use std::time::Duration;

use crate::TransportError;

/// The only identification sent besides the channel's headers.
pub const USER_AGENT: &str = "Drydock-Updater";
pub const RESOLVE_TIMEOUT_MS: i32 = 10_000;
pub const CONNECT_TIMEOUT_MS: i32 = 10_000;
pub const SEND_TIMEOUT_MS: i32 = 30_000;
/// Longest wait for the next bytes; a stalled download ends after this.
pub const RECEIVE_TIMEOUT_MS: i32 = 30_000;
/// Longest a whole `get` (the manifest) may take.
pub const GET_TOTAL_TIMEOUT: Duration = Duration::from_secs(60);
/// Read size of the body loops.
pub const CHUNK_BYTES: usize = 64 * 1024;

/// A parsed `https://` URL, ready for `WinHttpConnect` / `WinHttpOpenRequest`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpsUrl {
    /// Host name or address; an IPv6 literal without its brackets.
    pub host: String,
    pub port: u16,
    /// Path and query, always starting with `/`; the fragment is dropped.
    pub path: String,
}

/// Split an `https://` URL. See the spec for the rules; every refusal is
/// `TransportError::Other(..)`.
pub fn parse_https_url(url: &str) -> Result<HttpsUrl, TransportError> {
    let _ = url;
    unimplemented!("m13-update-winhttp")
}

/// The extra request headers as one block: `Name: value\r\n` per header, in order; empty for no
/// headers. A bad name or value is `TransportError::Other(..)`.
pub fn header_block(headers: &[(String, String)]) -> Result<String, TransportError> {
    let _ = headers;
    unimplemented!("m13-update-winhttp")
}

/// Ok for 200..=299, otherwise `TransportError::Http(status)`.
pub fn check_status(status: u16) -> Result<(), TransportError> {
    let _ = status;
    unimplemented!("m13-update-winhttp")
}

/// Map a WinHTTP error code (`GetLastError` / the `windows` error's code) to a
/// `TransportError`: timeouts, name resolution and connection failures are `Offline`.
pub fn error_from_win32(code: u32) -> TransportError {
    let _ = code;
    unimplemented!("m13-update-winhttp")
}

/// Counts body bytes against the limit.
#[derive(Clone, Debug)]
pub struct BodyBudget {
    max: u64,
    total: u64,
}

impl BodyBudget {
    pub fn new(max: u64) -> BodyBudget {
        BodyBudget { max, total: 0 }
    }

    /// Check a `Content-Length` before reading: `TooLarge` when it is above the limit;
    /// `None` is fine.
    pub fn check_declared(&self, content_length: Option<u64>) -> Result<(), TransportError> {
        let _ = (self.max, content_length);
        unimplemented!("m13-update-winhttp")
    }

    /// Count `n` more bytes; `TooLarge` once the total is above the limit (exactly the limit
    /// is allowed). Does not overflow.
    pub fn add(&mut self, n: u64) -> Result<(), TransportError> {
        let _ = n;
        unimplemented!("m13-update-winhttp")
    }

    /// Bytes counted so far.
    pub fn total(&self) -> u64 {
        self.total
    }
}
