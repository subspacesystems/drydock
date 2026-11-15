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
    let bad = || TransportError::Other("invalid https URL".into());
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(bad());
    }
    let (scheme, rest) = url.split_once("://").ok_or_else(bad)?;
    if !scheme.eq_ignore_ascii_case("https") {
        return Err(bad());
    }
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let tail = &rest[authority_end..];
    if authority.contains('@') {
        return Err(bad());
    }

    let (host, port) = if let Some(after_open) = authority.strip_prefix('[') {
        let close = after_open.find(']').ok_or_else(bad)?;
        let host = &after_open[..close];
        if host.is_empty() {
            return Err(bad());
        }
        let port = match &after_open[close + 1..] {
            "" => 443,
            after => {
                let digits = after.strip_prefix(':').ok_or_else(bad)?;
                parse_port(digits)?
            }
        };
        (host.to_string(), port)
    } else {
        if authority.contains(['[', ']']) {
            return Err(bad());
        }
        let (host, port) = match authority.rsplit_once(':') {
            Some((host, digits)) => (host, parse_port(digits)?),
            None => (authority, 443),
        };
        if host.is_empty() {
            return Err(bad());
        }
        (host.to_string(), port)
    };

    let without_fragment = match tail.find('#') {
        Some(i) => &tail[..i],
        None => tail,
    };
    let path = if without_fragment.is_empty() {
        "/".to_string()
    } else if without_fragment.starts_with('?') {
        format!("/{without_fragment}")
    } else {
        without_fragment.to_string()
    };

    Ok(HttpsUrl { host, port, path })
}

fn parse_port(digits: &str) -> Result<u16, TransportError> {
    let bad = || TransportError::Other("invalid https URL".into());
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(bad());
    }
    let port: u32 = digits.parse().map_err(|_| bad())?;
    if port == 0 || port > 65535 {
        return Err(bad());
    }
    Ok(port as u16)
}

/// The extra request headers as one block: `Name: value\r\n` per header, in order; empty for no
/// headers. A bad name or value is `TransportError::Other(..)`.
pub fn header_block(headers: &[(String, String)]) -> Result<String, TransportError> {
    let mut block = String::new();
    for (name, value) in headers {
        let good_name =
            !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
        if !good_name {
            return Err(TransportError::Other("invalid header name".into()));
        }
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(TransportError::Other("invalid header value".into()));
        }
        block.push_str(name);
        block.push_str(": ");
        block.push_str(value);
        block.push_str("\r\n");
    }
    Ok(block)
}

/// Ok for 200..=299, otherwise `TransportError::Http(status)`.
pub fn check_status(status: u16) -> Result<(), TransportError> {
    if (200..=299).contains(&status) {
        Ok(())
    } else {
        Err(TransportError::Http(status))
    }
}

/// Map a WinHTTP error code (`GetLastError` / the `windows` error's code) to a
/// `TransportError`: timeouts, name resolution and connection failures are `Offline`.
pub fn error_from_win32(code: u32) -> TransportError {
    match code {
        12002 | 12007 | 12029 | 12030 => TransportError::Offline,
        _ => TransportError::Other(format!("WinHTTP error {code}")),
    }
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
        match content_length {
            Some(length) if length > self.max => Err(TransportError::TooLarge),
            _ => Ok(()),
        }
    }

    /// Count `n` more bytes; `TooLarge` once the total is above the limit (exactly the limit
    /// is allowed). Does not overflow.
    pub fn add(&mut self, n: u64) -> Result<(), TransportError> {
        self.total = self.total.saturating_add(n);
        if self.total > self.max {
            Err(TransportError::TooLarge)
        } else {
            Ok(())
        }
    }

    /// Bytes counted so far.
    pub fn total(&self) -> u64 {
        self.total
    }
}
