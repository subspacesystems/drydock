// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-background-model-info: the Background section of Model Info, the editor's
//! "Set Background" step, and the viewport style built from the model.
//! See docs/specs/m10-background-model-info.md.

use drydock::background_ui::background_section;
use drydock::editor::Editor;
use drydock::input::EditKey;
use drydock::look::{Theme, apply_model_style, model_style, palette, viewport_style};
use drydock_kernel::background::{Background, DEFAULT_BACKGROUND, DEFAULT_GROUND, DEFAULT_SKY};
use drydock_kernel::model::Model;
use drydock_render::SunLight;
use eframe::egui::{
    self,
    accesskit::{Role, Toggled},
};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

fn opaque(c: [f32; 3]) -> [f32; 4] {
    [c[0], c[1], c[2], 1.0]
}

fn bytes(c: egui::Color32) -> [f32; 4] {
    [
        f32::from(c.r()) / 255.0,
        f32::from(c.g()) / 255.0,
        f32::from(c.b()) / 255.0,
        1.0,
    ]
}

fn close(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
}

// ---------------------------------------------------------------------------------------
// The style built from the model

#[test]
fn a_new_model_gives_sky_and_ground_and_the_light_edge_colors() {
    let style = model_style(Some(&Model::new().background()));
    assert_eq!(style.background, opaque(DEFAULT_BACKGROUND));
    assert_eq!(style.sky, Some(opaque(DEFAULT_SKY)));
    assert_eq!(style.ground, Some(opaque(DEFAULT_GROUND)));
    let light = palette(Theme::Light);
    assert!(close(style.default_line, bytes(light.edge)));
    assert!(close(style.default_face, bytes(light.face_front)));
    assert!(close(style.default_back.unwrap(), bytes(light.face_back)));
    assert_eq!(style.sun, None);
}

#[test]
fn the_edge_and_face_colors_do_not_follow_the_dark_theme() {
    let style = model_style(Some(&Background::default()));
    let dark = viewport_style(Theme::Dark);
    assert!(!close(style.default_line, dark.default_line));
    assert!(!close(style.default_face, dark.default_face));
    assert!(!close(style.background, dark.background));
}

#[test]
fn switched_off_sky_and_ground_are_none_and_colors_are_taken_from_the_model() {
    let background = Background {
        color: [0.1, 0.2, 0.3],
        sky: false,
        sky_color: [0.4, 0.5, 0.6],
        ground: true,
        ground_color: [0.7, 0.8, 0.9],
    };
    let style = model_style(Some(&background));
    assert_eq!(style.background, [0.1, 0.2, 0.3, 1.0]);
    assert_eq!(style.sky, None);
    assert_eq!(style.ground, Some([0.7, 0.8, 0.9, 1.0]));

    let both_off = Background {
        ground: false,
        ..background
    };
    assert_eq!(model_style(Some(&both_off)).ground, None);
}

#[test]
fn no_model_gives_the_default_background() {
    let none = model_style(None);
    let default = model_style(Some(&Background::default()));
    assert_eq!(none.background, default.background);
    assert_eq!(none.sky, default.sky);
    assert_eq!(none.ground, default.ground);
}

#[test]
fn apply_reports_changes_and_leaves_the_sun_alone() {
    let mut style = viewport_style(Theme::Dark);
    let sun = Some(SunLight {
        direction: glam::DVec3::new(0.0, 0.0, 1.0),
        light: 0.5,
        dark: 0.5,
        shadows: false,
        on_ground: false,
    });
    style.sun = sun;
    let background = Background::default();
    assert!(apply_model_style(&mut style, Some(&background)));
    assert_eq!(style.sun, sun);
    assert_eq!(style.sky, Some(opaque(DEFAULT_SKY)));
    assert!(!apply_model_style(&mut style, Some(&background)));

    let changed = Background {
        sky: false,
        ..background
    };
    assert!(apply_model_style(&mut style, Some(&changed)));
    assert_eq!(style.sky, None);
    assert!(apply_model_style(&mut style, None));
    assert_eq!(style.sky, Some(opaque(DEFAULT_SKY)));
}

// ---------------------------------------------------------------------------------------
// The editor step

#[test]
fn set_background_is_one_undo_step_and_undo_restores_it() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let before = model.background();
    let wanted = Background {
        sky: false,
        ground_color: [0.2, 0.3, 0.4],
        ..before
    };
    assert!(editor.set_background(&mut model, wanted));
    assert_eq!(model.background(), wanted);
    assert_eq!(editor.undo_stack().undo_count(), 1);
    assert_eq!(editor.undo_stack().undo_label(), Some("Set Background"));

    assert!(editor.key(EditKey::Undo, &mut model));
    assert_eq!(model.background(), before);
    assert!(editor.key(EditKey::Redo, &mut model));
    assert_eq!(model.background(), wanted);
}

#[test]
fn setting_the_same_background_makes_no_step() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let same = model.background();
    assert!(editor.set_background(&mut model, same));
    assert_eq!(editor.undo_stack().undo_count(), 0);
}

#[test]
fn an_invalid_background_is_refused_and_changes_nothing() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let before = model.background();
    let bad = Background {
        sky_color: [1.5, 0.0, 0.0],
        ..before
    };
    assert!(!editor.set_background(&mut model, bad));
    assert_eq!(model.background(), before);
    assert_eq!(editor.undo_stack().undo_count(), 0);
}

// ---------------------------------------------------------------------------------------
// The section

struct Section {
    background: Background,
    picked: Vec<Background>,
}

fn section(background: Background) -> Harness<'static, Section> {
    let mut h = Harness::builder().with_size([420.0, 400.0]).build_ui_state(
        |ui, state: &mut Section| {
            if let Some(new) = background_section(ui, &state.background) {
                state.picked.push(new);
                state.background = new;
            }
        },
        Section {
            background,
            picked: Vec::new(),
        },
    );
    h.run();
    h
}

fn checked(h: &Harness<'_, Section>, label: &str) -> bool {
    let node = h.get_by_label(label);
    assert_eq!(node.accesskit_node().role(), Role::CheckBox, "{label}");
    node.accesskit_node().toggled() == Some(Toggled::True)
}

#[test]
fn section_shows_the_heading_rows_and_the_models_flags() {
    let h = section(Background::default());
    h.get_by_label("Background");
    h.get_by_label("Background:");
    assert!(checked(&h, "Sky"));
    assert!(checked(&h, "Ground"));
    assert!(h.state().picked.is_empty());

    let h = section(Background {
        sky: false,
        ground: false,
        ..Background::default()
    });
    assert!(!checked(&h, "Sky"));
    assert!(!checked(&h, "Ground"));
}

#[test]
fn unchecking_sky_returns_the_settings_with_the_sky_off_and_colors_kept() {
    let mut h = section(Background::default());
    h.get_by_label("Sky").click();
    h.run();
    let picked = h.state().picked.clone();
    assert_eq!(picked.len(), 1);
    assert_eq!(
        picked[0],
        Background {
            sky: false,
            ..Background::default()
        }
    );
    assert!(!checked(&h, "Sky"));
    assert!(checked(&h, "Ground"));
}

#[test]
fn unchecking_ground_and_checking_it_again() {
    let mut h = section(Background::default());
    h.get_by_label("Ground").click();
    h.run();
    assert!(!h.state().background.ground);
    h.get_by_label("Ground").click();
    h.run();
    assert_eq!(h.state().background, Background::default());
    assert_eq!(h.state().picked.len(), 2);
}
