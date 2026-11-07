// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-color-picker-srgb: color buttons edit the stored sRGB colors as sRGB.
//! See docs/specs/m10-color-picker-srgb.md.

use drydock::background_ui::background_section;
use drydock::color_ui::{bytes_to_srgb, srgb_color_button, srgb_to_bytes};
use drydock_kernel::background::{Background, DEFAULT_GROUND, DEFAULT_SKY};
use eframe::egui;
use egui_kittest::Harness;

#[test]
fn every_byte_round_trips_exactly() {
    for v in 0..=255u8 {
        assert_eq!(srgb_to_bytes(bytes_to_srgb([v, v, v])), [v, v, v], "{v}");
    }
    assert_eq!(srgb_to_bytes(bytes_to_srgb([1, 128, 254])), [1, 128, 254]);
}

#[test]
fn floats_round_and_clamp() {
    assert_eq!(srgb_to_bytes([0.0, 1.0, 0.5]), [0, 255, 128]);
    assert_eq!(srgb_to_bytes([-0.3, 1.7, 0.001]), [0, 255, 0]);
    assert_eq!(srgb_to_bytes([0.0019, 0.0021, 0.9999]), [0, 1, 255]);
    let c = bytes_to_srgb([0xA8, 0xC8, 0xE8]);
    assert!((c[0] - 168.0 / 255.0).abs() < 1e-6);
}

#[test]
fn the_section_reports_nothing_without_interaction() {
    let mut h = Harness::builder().with_size([420.0, 300.0]).build_ui_state(
        |ui, state: &mut (Background, usize)| {
            if background_section(ui, &state.0).is_some() {
                state.1 += 1;
            }
        },
        (Background::default(), 0usize),
    );
    h.run();
    h.run();
    assert_eq!(h.state().1, 0);
}

#[test]
fn opening_the_picker_does_not_move_an_inexact_color() {
    // Not a multiple of 1/255: must survive frames untouched.
    let start = [0.3337_f32, 0.5001, 0.9];
    let mut h = Harness::builder().with_size([300.0, 200.0]).build_ui_state(
        |ui, state: &mut ([f32; 3], bool)| {
            if srgb_color_button(ui, &mut state.0) {
                state.1 = true;
            }
        },
        (start, false),
    );
    h.run();
    h.run();
    assert_eq!(h.state().0, start);
    assert!(!h.state().1);
}

/// Every filled rect shape painted in the harness output.
fn fills(h: &Harness<'_, Background>) -> Vec<egui::Color32> {
    let mut out = Vec::new();
    for clipped in h.output().shapes.iter() {
        collect(&clipped.shape, &mut out);
    }
    out
}

fn collect(shape: &egui::epaint::Shape, out: &mut Vec<egui::Color32>) {
    match shape {
        egui::epaint::Shape::Rect(r) => out.push(r.fill),
        egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
        _ => {}
    }
}

#[test]
fn the_swatches_paint_the_stored_srgb_colors() {
    let mut h = Harness::builder().with_size([420.0, 300.0]).build_ui_state(
        |ui, bg: &mut Background| {
            let _ = background_section(ui, bg);
        },
        Background::default(),
    );
    h.run();
    let f = fills(&h);
    let b = |c: [f32; 3]| {
        let u = srgb_to_bytes(c);
        egui::Color32::from_rgb(u[0], u[1], u[2])
    };
    assert_eq!(b(DEFAULT_SKY), egui::Color32::from_rgb(0xA8, 0xC8, 0xE8));
    assert!(f.contains(&b(DEFAULT_SKY)), "sky swatch fill");
    assert!(f.contains(&b(DEFAULT_GROUND)), "ground swatch fill");
}
