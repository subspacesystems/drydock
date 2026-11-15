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
mod transport;
mod updater;
mod verify;
mod version;

pub use channel::{Channel, ChannelError};
pub use manifest::{Manifest, ParseError, hex_decode_32, hex_encode};
pub use transport::{Transport, TransportError};
pub use updater::{Background, Config, Mode, State, Updater};
pub use verify::{FileCheckError, VerifyError, verify, verify_file_sha256};
pub use version::Version;
