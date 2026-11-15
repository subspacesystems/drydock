// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use crate::hex_decode_32;

/// Where updates come from: a name, the manifest's URL, the key its signature must match and
/// extra request headers (a later channel can add an auth header through configuration alone).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    pub name: String,
    pub manifest_url: String,
    pub public_key: [u8; 32],
    /// (name, value) pairs in file order.
    pub headers: Vec<(String, String)>,
}

/// Why a channel file was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChannelError {
    NotUtf8,
    /// A line that is not blank, not a comment and has no `=` (1-based line number).
    Syntax {
        line: usize,
    },
    Missing(&'static str),
    Duplicate(&'static str),
    Invalid(&'static str),
}

impl Channel {
    /// Same line syntax as the manifest ([`crate`] docs). Keys: `name` (non-empty, no control
    /// characters), `manifest_url` (as for manifest URLs: `https://`, no whitespace or
    /// control characters), `public_key` (64 hex digits) are required once each; `header`
    /// is optional and repeatable, value `Name: value` (split at the first colon, both parts
    /// trimmed). A header name is 1 or more of ASCII letters, digits and `-`; the value must be
    /// non-empty and contain no control characters (so no CR or LF).
    /// A bad header line is `Invalid("header")`. Unknown keys are ignored.
    pub fn parse(bytes: &[u8]) -> Result<Channel, ChannelError> {
        let bytes = strip_bom(bytes);
        let text = std::str::from_utf8(bytes).map_err(|_| ChannelError::NotUtf8)?;
        let mut name = None;
        let mut manifest_url = None;
        let mut public_key = None;
        let mut headers = Vec::new();
        let mut seen = [false; 3];
        for (index, raw) in text.split('\n').enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(ChannelError::Syntax { line: index + 1 });
            };
            let key = key.trim();
            let value = value.trim();
            match key {
                "name" => {
                    if seen[0] {
                        return Err(ChannelError::Duplicate("name"));
                    }
                    seen[0] = true;
                    if value.is_empty() || value.chars().any(char::is_control) {
                        return Err(ChannelError::Invalid("name"));
                    }
                    name = Some(value.to_string());
                }
                "manifest_url" => {
                    if seen[1] {
                        return Err(ChannelError::Duplicate("manifest_url"));
                    }
                    seen[1] = true;
                    if !valid_url(value) {
                        return Err(ChannelError::Invalid("manifest_url"));
                    }
                    manifest_url = Some(value.to_string());
                }
                "public_key" => {
                    if seen[2] {
                        return Err(ChannelError::Duplicate("public_key"));
                    }
                    seen[2] = true;
                    public_key =
                        Some(hex_decode_32(value).ok_or(ChannelError::Invalid("public_key"))?);
                }
                "header" => {
                    let Some((header_name, header_value)) = value.split_once(':') else {
                        return Err(ChannelError::Invalid("header"));
                    };
                    let header_name = header_name.trim();
                    let header_value = header_value.trim();
                    if header_name.is_empty()
                        || !header_name
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                        || header_value.is_empty()
                        || header_value.chars().any(char::is_control)
                    {
                        return Err(ChannelError::Invalid("header"));
                    }
                    headers.push((header_name.to_string(), header_value.to_string()));
                }
                _ => {}
            }
        }
        let name = name.ok_or(ChannelError::Missing("name"))?;
        let manifest_url = manifest_url.ok_or(ChannelError::Missing("manifest_url"))?;
        let public_key = public_key.ok_or(ChannelError::Missing("public_key"))?;
        Ok(Channel {
            name,
            manifest_url,
            public_key,
            headers,
        })
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
