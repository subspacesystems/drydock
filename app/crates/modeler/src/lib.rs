// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! modeler (codename).
//!
//! Milestone 0 (`docs/specs/m0-app.md`): a window that opens a DAE file and lets you
//! orbit, pan and zoom it. The window code lives in `app`; `input` and `document`
//! hold the logic that tests exercise without a display.

pub mod document;
pub mod input;

/// One-line product banner.
pub fn banner() -> String {
    format!("modeler {}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn banner_names_the_codename() {
        assert!(super::banner().starts_with("modeler "));
    }
}
