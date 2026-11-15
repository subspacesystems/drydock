// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use crate::hex_decode_32;
use crate::manifest::valid_url;

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
        let text = std::str::from_utf8(bytes).map_err(|_| ChannelError::NotUtf8)?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);

        let mut name = None;
        let mut manifest_url = None;
        let mut public_key = None;
        let mut headers = Vec::new();

        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
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
                    if name.is_some() {
                        return Err(ChannelError::Duplicate("name"));
                    }
                    if value.is_empty() || value.chars().any(char::is_control) {
                        return Err(ChannelError::Invalid("name"));
                    }
                    name = Some(value.to_string());
                }
                "manifest_url" => {
                    if manifest_url.is_some() {
                        return Err(ChannelError::Duplicate("manifest_url"));
                    }
                    if !valid_url(value) {
                        return Err(ChannelError::Invalid("manifest_url"));
                    }
                    manifest_url = Some(value.to_string());
                }
                "public_key" => {
                    if public_key.is_some() {
                        return Err(ChannelError::Duplicate("public_key"));
                    }
                    public_key =
                        Some(hex_decode_32(value).ok_or(ChannelError::Invalid("public_key"))?);
                }
                "header" => {
                    headers.push(parse_header(value).ok_or(ChannelError::Invalid("header"))?);
                }
                _ => {}
            }
        }

        Ok(Channel {
            name: name.ok_or(ChannelError::Missing("name"))?,
            manifest_url: manifest_url.ok_or(ChannelError::Missing("manifest_url"))?,
            public_key: public_key.ok_or(ChannelError::Missing("public_key"))?,
            headers,
        })
    }
}

fn parse_header(value: &str) -> Option<(String, String)> {
    let (name, value) = value.split_once(':')?;
    let name = name.trim();
    let value = value.trim();
    let valid_name =
        !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    if !valid_name || value.is_empty() || value.chars().any(char::is_control) {
        return None;
    }
    Some((name.to_string(), value.to_string()))
}
