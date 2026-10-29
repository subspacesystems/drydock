// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! modeler (codename).
//!
//! Milestone 0 (`docs/specs/m0-app.md`): a window that opens a DAE file and lets you
//! orbit, pan and zoom it. The window code lives in `app`; `input` and `document`
//! hold the logic that tests exercise without a display.

pub mod chrome;
pub mod components_panel;
pub mod dialog;
pub mod dock;
pub mod document;
pub mod editor;
pub mod entity_info;
pub mod fit_check;
pub mod geo_panel;
pub mod globe;
pub mod hints;
pub mod icons;
pub mod input;
pub mod look;
pub mod materials_panel;
pub mod mesh_import;
pub mod overlay;
pub mod recent;
pub mod shading;
pub mod shadows_panel;
pub mod sun_windows;
pub mod swatches;
pub mod texture_handles;
pub mod texture_panel;
pub mod toolbars;
pub mod units_ui;
pub mod viewstate;

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
