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
        let bytes = strip_bom(bytes);
        let text = std::str::from_utf8(bytes).map_err(|_| ParseError::NotUtf8)?;
        let mut version = None;
        let mut notes_url = None;
        let mut installer_url = None;
        let mut size = None;
        let mut sha256 = None;
        let mut seen = [false; 5];
        for (index, raw) in text.split('\n').enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(ParseError::Syntax { line: index + 1 });
            };
            let key = key.trim();
            let value = value.trim();
            match key {
                "version" => {
                    if seen[0] {
                        return Err(ParseError::Duplicate("version"));
                    }
                    seen[0] = true;
                    version = Some(Version::parse(value).ok_or(ParseError::Invalid("version"))?);
                }
                "notes_url" => {
                    if seen[1] {
                        return Err(ParseError::Duplicate("notes_url"));
                    }
                    seen[1] = true;
                    if !valid_url(value) {
                        return Err(ParseError::Invalid("notes_url"));
                    }
                    notes_url = Some(value.to_string());
                }
                "installer_url" => {
                    if seen[2] {
                        return Err(ParseError::Duplicate("installer_url"));
                    }
                    seen[2] = true;
                    if !valid_url(value) {
                        return Err(ParseError::Invalid("installer_url"));
                    }
                    installer_url = Some(value.to_string());
                }
                "size" => {
                    if seen[3] {
                        return Err(ParseError::Duplicate("size"));
                    }
                    seen[3] = true;
                    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                        return Err(ParseError::Invalid("size"));
                    }
                    let parsed = value
                        .parse::<u64>()
                        .ok()
                        .filter(|n| *n >= 1)
                        .ok_or(ParseError::Invalid("size"))?;
                    size = Some(parsed);
                }
                "sha256" => {
                    if seen[4] {
                        return Err(ParseError::Duplicate("sha256"));
                    }
                    seen[4] = true;
                    sha256 = Some(hex_decode_32(value).ok_or(ParseError::Invalid("sha256"))?);
                }
                _ => {}
            }
        }
        let version = version.ok_or(ParseError::Missing("version"))?;
        let notes_url = notes_url.ok_or(ParseError::Missing("notes_url"))?;
        let installer_url = installer_url.ok_or(ParseError::Missing("installer_url"))?;
        let size = size.ok_or(ParseError::Missing("size"))?;
        let sha256 = sha256.ok_or(ParseError::Missing("sha256"))?;
        Ok(Manifest {
            version,
            notes_url,
            installer_url,
            size,
            sha256,
        })
    }

    /// Canonical text: the five keys in the order above, one per line, `\n` line ends,
    /// final newline, hash in lower-case hex. `Manifest::parse(m.to_text().as_bytes())` is `Ok(m)`.
    pub fn to_text(&self) -> String {
        format!(
            "version = {}\nnotes_url = {}\ninstaller_url = {}\nsize = {}\nsha256 = {}\n",
            self.version,
            self.notes_url,
            self.installer_url,
            self.size,
            hex_encode(&self.sha256),
        )
    }
}

fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes)
}

fn valid_url(value: &str) -> bool {
    value.starts_with("https://")
        && value.len() > "https://".len()
        && !value.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Lower-case hex of the bytes.
pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 0x0f) as usize] as char);
    }
    text
}

/// Decodes exactly 64 hex digits (either case) into 32 bytes; None for any other input.
pub fn hex_decode_32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (index, pair) in text.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        out[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Some(out)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
