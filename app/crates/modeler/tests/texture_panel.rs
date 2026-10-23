// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-app-texture-position-ui: the Texture Position panel, driven with egui_kittest.
//! See docs/specs/m4-app-texture-position-ui.md.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glam::DVec2;
use modeler::editor::TextureOp;
use modeler::texture_panel::{TextureForm, texture_panel};

struct State {
    form: TextureForm,
    available: bool,
    ops: Vec<TextureOp>,
}

fn harness(available: bool, form: TextureForm) -> Harness<'static, State> {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut State| {
            if let Some(op) = texture_panel(ui, &mut state.form, state.available) {
                state.ops.push(op);
            }
        },
        State {
            form,
            available,
            ops: Vec::new(),
        },
    );
    h.run();
    h
}

fn click(h: &mut Harness<'_, State>, label: &str) {
    h.get_by_label(label).click();
    h.run();
}

fn disabled(h: &Harness<'_, State>, label: &str) -> bool {
    use egui_kittest::kittest::NodeT;
    h.get_by_label(label).accesskit_node().is_disabled()
}

#[test]
fn default_form_is_neutral() {
    assert_eq!(
        TextureForm::default(),
        TextureForm {
            shift: [0.0, 0.0],
            turn: 0.0,
            scale: [1.0, 1.0],
        }
    );
}

#[test]
fn panel_shows_its_widgets() {
    let h = harness(true, TextureForm::default());
    for label in [
        "Texture Position",
        "Move X (m)",
        "Move Y (m)",
        "Move",
        "Turn (deg)",
        "Turn",
        "Scale X",
        "Scale Y",
        "Scale",
        "Reset Texture",
    ] {
        h.get_by_label(label);
    }
    assert!(h.query_by_label("Select a face with a texture").is_none());
    assert!(h.state().ops.is_empty());
}

#[test]
fn unavailable_panel_says_so_and_does_nothing() {
    let form = TextureForm {
        shift: [1.0, 2.0],
        ..TextureForm::default()
    };
    let mut h = harness(false, form.clone());
    h.get_by_label("Select a face with a texture");
    for label in ["Move", "Turn", "Scale", "Reset Texture"] {
        assert!(disabled(&h, label), "{label}");
        click(&mut h, label);
    }
    assert!(h.state().ops.is_empty());
    assert_eq!(h.state().form, form);
}

#[test]
fn move_sends_the_shift_and_clears_it() {
    let form = TextureForm {
        shift: [0.25, -0.5],
        turn: 30.0,
        ..TextureForm::default()
    };
    let mut h = harness(true, form);
    click(&mut h, "Move");
    assert_eq!(h.state().ops, vec![TextureOp::Move(DVec2::new(0.25, -0.5))]);
    assert_eq!(h.state().form.shift, [0.0, 0.0]);
    assert_eq!(h.state().form.turn, 30.0);
}

#[test]
fn turn_sends_radians_about_the_origin_and_clears_it() {
    let form = TextureForm {
        turn: 90.0,
        scale: [2.0, 2.0],
        ..TextureForm::default()
    };
    let mut h = harness(true, form);
    click(&mut h, "Turn");
    assert_eq!(
        h.state().ops,
        vec![TextureOp::Rotate {
            angle: 90f64.to_radians(),
            about: DVec2::ZERO
        }]
    );
    assert_eq!(h.state().form.turn, 0.0);
    assert_eq!(h.state().form.scale, [2.0, 2.0]);
}

#[test]
fn scale_sends_the_factors_and_clears_them() {
    let form = TextureForm {
        scale: [2.0, 0.5],
        ..TextureForm::default()
    };
    let mut h = harness(true, form);
    click(&mut h, "Scale");
    assert_eq!(
        h.state().ops,
        vec![TextureOp::Scale {
            factor: DVec2::new(2.0, 0.5),
            about: DVec2::ZERO
        }]
    );
    assert_eq!(h.state().form.scale, [1.0, 1.0]);
}

#[test]
fn reset_sends_reset_and_keeps_the_form() {
    let form = TextureForm {
        shift: [1.0, 1.0],
        ..TextureForm::default()
    };
    let mut h = harness(true, form.clone());
    click(&mut h, "Reset Texture");
    assert_eq!(h.state().ops, vec![TextureOp::Reset]);
    assert_eq!(h.state().form, form);
}

#[test]
fn each_click_sends_one_op() {
    let form = TextureForm {
        shift: [1.0, 0.0],
        ..TextureForm::default()
    };
    let mut h = harness(true, form);
    click(&mut h, "Move");
    click(&mut h, "Move");
    click(&mut h, "Reset Texture");
    assert_eq!(
        h.state().ops,
        vec![
            TextureOp::Move(DVec2::new(1.0, 0.0)),
            TextureOp::Move(DVec2::ZERO),
            TextureOp::Reset
        ]
    );
}

#[test]
fn invalid_values_disable_their_button_only() {
    for (name, form, bad, good) in [
        (
            "shift",
            TextureForm {
                shift: [f64::NAN, 0.0],
                ..TextureForm::default()
            },
            "Move",
            "Turn",
        ),
        (
            "turn",
            TextureForm {
                turn: f64::INFINITY,
                ..TextureForm::default()
            },
            "Turn",
            "Scale",
        ),
        (
            "zero scale",
            TextureForm {
                scale: [0.0, 1.0],
                ..TextureForm::default()
            },
            "Scale",
            "Move",
        ),
        (
            "negative scale",
            TextureForm {
                scale: [1.0, -2.0],
                ..TextureForm::default()
            },
            "Scale",
            "Move",
        ),
    ] {
        let mut h = harness(true, form.clone());
        assert!(disabled(&h, bad), "{name}");
        assert!(!disabled(&h, good), "{name}");
        click(&mut h, bad);
        assert!(h.state().ops.is_empty(), "{name}");
        assert_eq!(h.state().form, form, "{name}");
    }
}
