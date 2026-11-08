// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! modeler (codename).
//!
//! Milestone 0 (`docs/specs/m0-app.md`): a window that opens a DAE file and lets you
//! orbit, pan and zoom it. The window code lives in `app`; `input` and `document`
//! hold the logic that tests exercise without a display.

pub mod background_ui;
pub mod chrome;
pub mod clearance_offset;
pub mod color_ui;
pub mod components_panel;
pub mod desktop_entry;
pub mod dialog;
pub mod dock;
pub mod document;
pub mod editor;
pub mod entity_info;
pub mod fit_check;
pub mod follow_tool;
pub mod geo_panel;
pub mod globe;
pub mod guide_help;
pub mod hints;
pub mod icons;
pub mod input;
pub mod look;
pub mod materials_panel;
pub mod mesh_export;
pub mod mesh_import;
pub mod model_source;
pub mod open_buttons;
pub mod overlay;
pub mod portal;
pub mod print_setup;
pub mod prompts;
pub mod recent;
pub mod shading;
pub mod shadows_panel;
pub mod solid_ui;
pub mod start;
pub mod sun_windows;
pub mod swatches;
pub mod tags_panel;
pub mod text_tool;
pub mod texture_handles;
pub mod texture_panel;
pub mod toolbars;
pub mod units_ui;
pub mod viewstate;
pub mod window_icon;
pub mod winmeta;
pub mod winpaths;

/// One-line product banner.
pub fn banner() -> String {
    format!("Drydock {}", crate::chrome::VERSION)
}

#[cfg(test)]
mod tests {
    #[test]
    fn banner_names_the_product() {
        assert!(super::banner().starts_with("Drydock "));
    }
}
