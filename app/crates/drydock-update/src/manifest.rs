// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use crate::Version;

/// What a release publishes: the newest version and where to get it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub version: Version,
    pub notes_url: String,
    pub installer_url: String,
    /// Installer size in bytes (greater than zero).
    pub size: u64,
    pub sha256: [u8; 32],
}

/// Why manifest text was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// The bytes are not UTF-8.
    NotUtf8,
    /// A line that is not blank, not a comment and has no `=` (1-based line number).
    Syntax { line: usize },
    /// A required key is absent.
    Missing(&'static str),
    /// A required key appears twice.
    Duplicate(&'static str),
    /// A required key has a value that does not validate.
    Invalid(&'static str),
}

impl Manifest {
    /// Required keys: `version`, `notes_url`, `installer_url`, `size`, `sha256`.
    /// Validation: `version` per [`Version::parse`]; both URLs start with `https://`, have
    /// something after it, and contain no whitespace or control characters; `size` is
    /// decimal digits, at least 1, fits u64; `sha256` is 64 hex digits. Other keys are ignored
    /// (a duplicated unknown key is fine). A UTF-8 byte-order mark and CRLF line ends are
    /// accepted. Errors are reported for the first problem in file order, but a duplicate or
    /// missing key is reported as such, not as Invalid.
    pub fn parse(bytes: &[u8]) -> Result<Manifest, ParseError> {
        let _ = bytes;
        unimplemented!()
    }

    /// Canonical text: the five keys in the order above, one per line, `\n` line ends,
    /// final newline, hash in lower-case hex. `Manifest::parse(m.to_text().as_bytes())` is `Ok(m)`.
    pub fn to_text(&self) -> String {
        unimplemented!()
    }
}

/// Lower-case hex of the bytes.
pub fn hex_encode(bytes: &[u8]) -> String {
    let _ = bytes;
    unimplemented!()
}

/// Decodes exactly 64 hex digits (either case) into 32 bytes; None for any other input.
pub fn hex_decode_32(text: &str) -> Option<[u8; 32]> {
    let _ = text;
    unimplemented!()
}
