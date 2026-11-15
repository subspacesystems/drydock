// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

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
        let _ = bytes;
        unimplemented!()
    }
}
