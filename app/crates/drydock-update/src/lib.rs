// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Update checks that need no network and no platform: the signed manifest, the installer's
//! hash and the channel definition. Behaviour: `docs/specs/m13-update-verify.md`.
//!
//! # Formats
//!
//! **Text files** (manifest and channel) are UTF-8, one `key = value` per line. The key is
//! everything before the first `=`, the value everything after it; both are trimmed. Blank
//! lines and lines starting with `#` are skipped. Unknown keys are ignored.
//!
//! **Signature file**: a detached Ed25519 signature over the manifest file's exact bytes,
//! stored as the **64 raw bytes** (no base64, no text around it). The public key is 32 raw
//! bytes (written as 64 hex digits in channel files). SHA-256 values are 64 hex digits
//! (upper or lower case accepted, printed lower case).

mod channel;
mod manifest;
mod net;
mod shell;
mod transport;
mod updater;
mod verify;
mod version;
#[cfg(windows)]
mod winhttp;
#[cfg(windows)]
mod winshell;

pub use channel::{Channel, ChannelError};
pub use manifest::{Manifest, ParseError, hex_decode_32, hex_encode};
pub use net::{
    BodyBudget, CHUNK_BYTES, CONNECT_TIMEOUT_MS, GET_TOTAL_TIMEOUT, HttpsUrl, RECEIVE_TIMEOUT_MS,
    RESOLVE_TIMEOUT_MS, SEND_TIMEOUT_MS, USER_AGENT, check_status, error_from_win32, header_block,
    parse_https_url,
};
pub use shell::{ERROR_CANCELLED, LAUNCH_CANCELLED, join_parameters, launch_failure, quote_arg};
pub use transport::{Transport, TransportError};
pub use updater::{Background, Config, Mode, State, Updater};
pub use verify::{FileCheckError, VerifyError, verify, verify_file_sha256};
pub use version::Version;
#[cfg(windows)]
pub use winhttp::WinHttpTransport;
#[cfg(windows)]
pub use winshell::run_elevated;
