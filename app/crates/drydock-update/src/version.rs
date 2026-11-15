// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use std::fmt;

/// An `X.Y.Z` version. Ordering is numeric per part (`0.10.0` is newer than `0.9.0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl Version {
    /// Parses exactly three dot-separated parts of ASCII digits (no sign, no spaces, no empty
    /// part, no pre-release suffix; leading zeros are accepted: `01.2.3` is 1.2.3).
    /// A part too big for u64 is an error. Returns None for anything else.
    pub fn parse(text: &str) -> Option<Version> {
        let _ = text;
        unimplemented!()
    }

    /// True when `self` is strictly newer than `running` (the caller passes
    /// `Version::parse(env!("CARGO_PKG_VERSION"))`).
    pub fn is_newer_than(&self, running: &Version) -> bool {
        let _ = running;
        unimplemented!()
    }
}

impl fmt::Display for Version {
    /// `major.minor.patch`, no padding.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = f;
        unimplemented!()
    }
}
