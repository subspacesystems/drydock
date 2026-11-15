// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! `m13-update-ui`: where updating is not supported (any OS but Windows, with nothing forcing it
//! on) the menu item is not there at all. Its own test binary: the forcing flag is process-wide.

use drydock::chrome::{MenuState, menu_bar};
use drydock::update_ui::supported;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

#[cfg(not(windows))]
#[test]
fn unsupported_hides_the_menu_item() {
    assert!(!supported());
    let state = MenuState {
        update_channel: true,
        ..MenuState::default()
    };
    let mut h = Harness::builder()
        .with_size(eframe::egui::vec2(1000.0, 800.0))
        .build_ui(move |ui| {
            menu_bar(ui, &state);
        });
    h.run();
    h.get_by_label("Help").click();
    h.run();
    assert!(h.query_by_label("User Guide").is_some());
    assert!(h.query_by_label_contains("Check for Updates").is_none());
}

#[cfg(windows)]
#[test]
fn supported_on_windows() {
    assert!(supported());
}
