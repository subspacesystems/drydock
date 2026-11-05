// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-print-setup: the Print Setup window's contents, driven with egui_kittest
//! (`docs/specs/m7-print-setup.md`).

use drydock::look;
use drydock::print_setup::*;
use eframe::egui::accesskit::Role;
use eframe::egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

struct State {
    texts: PrintSetupTexts,
    bed: Bed,
    can_drop: bool,
    report: Option<FitReport>,
    events: Vec<PrintSetupEvent>,
}

fn harness(
    texts: PrintSetupTexts,
    can_drop: bool,
    report: Option<FitReport>,
) -> Harness<'static, State> {
    let state = State {
        texts,
        bed: Bed {
            x_mm: 220.0,
            y_mm: 220.0,
            z_mm: Some(250.0),
        },
        can_drop,
        report,
        events: vec![],
    };
    let mut h = Harness::new_ui_state(
        |ui, s: &mut State| {
            let view = PrintSetupView {
                bed: s.bed,
                show_bed: false,
                overhang_degrees: 45.0,
                can_drop: s.can_drop,
                report: s.report.as_ref(),
            };
            let event = print_setup_ui(ui, &mut s.texts, &view);
            if event != PrintSetupEvent::None {
                s.events.push(event);
            }
        },
        state,
    );
    h.ctx.set_fonts(look::font_definitions());
    h.run();
    h
}

fn blank() -> Harness<'static, State> {
    harness(PrintSetupTexts::default(), true, None)
}

/// The four text boxes, top to bottom: X, Y, Z, overhang angle.
fn boxes<'a>(h: &'a Harness<'static, State>) -> Vec<egui_kittest::Node<'a>> {
    let mut nodes: Vec<_> = h.query_all_by_role(Role::TextInput).collect();
    nodes.sort_by(|a, b| a.rect().top().total_cmp(&b.rect().top()));
    nodes
}

fn type_into(h: &mut Harness<'static, State>, index: usize, text: &str, enter: bool) {
    boxes(h)[index].click();
    h.run();
    boxes(h)[index].type_text(text);
    h.run();
    if enter {
        h.key_press(Key::Enter);
        h.run();
    }
}

#[test]
fn the_window_has_the_labels_the_spec_names() {
    let h = blank();
    h.get_by_label("Printer bed");
    for label in [
        "Bed X",
        "Bed Y",
        "Bed Z",
        "Show bed outline",
        "Check fit",
        "Drop to Bed",
        "Overhang angle",
        "deg",
    ] {
        h.get_by_label(label);
    }
    assert_eq!(h.query_all_by_label("mm").count(), 3);
    assert_eq!(boxes(&h).len(), 4);
    for p in &BED_PRESETS {
        h.get_by_label(p.name);
    }
}

#[test]
fn a_preset_button_sets_the_bed_keeping_the_height() {
    let mut h = blank();
    h.get_by_label("256 x 256 mm").click();
    h.run();
    assert_eq!(
        h.state().events,
        vec![PrintSetupEvent::SetBed(Bed {
            x_mm: 256.0,
            y_mm: 256.0,
            z_mm: Some(250.0)
        })]
    );
}

#[test]
fn typing_alone_commits_nothing() {
    let mut h = blank();
    type_into(&mut h, 0, "300", false);
    assert!(h.state().events.is_empty());
    assert_eq!(h.state().texts.x, "300");
}

#[test]
fn enter_commits_a_good_side() {
    let mut h = blank();
    type_into(&mut h, 1, "180.50", true);
    assert_eq!(
        h.state().events,
        vec![PrintSetupEvent::SetBed(Bed {
            x_mm: 220.0,
            y_mm: 180.5,
            z_mm: Some(250.0)
        })]
    );
    assert_eq!(h.state().texts.y, "180.5", "rewritten");
}

#[test]
fn a_bad_side_goes_back_to_the_current_value() {
    let mut h = blank();
    type_into(&mut h, 0, "abc", true);
    assert!(h.state().events.is_empty());
    assert_eq!(h.state().texts.x, "220");
}

#[test]
fn a_blank_height_clears_the_limit() {
    let texts = PrintSetupTexts::for_view(
        &Bed {
            x_mm: 220.0,
            y_mm: 220.0,
            z_mm: Some(250.0),
        },
        45.0,
    );
    assert_eq!(texts.z, "250");
    let mut h = harness(texts, true, None);
    boxes(&h)[2].click();
    h.run();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.key_press(Key::Backspace);
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().events.contains(&PrintSetupEvent::SetBed(Bed {
        x_mm: 220.0,
        y_mm: 220.0,
        z_mm: None
    })));
}

#[test]
fn the_overhang_angle_commits_in_range_only() {
    let mut h = blank();
    type_into(&mut h, 3, "50", true);
    assert_eq!(h.state().events, vec![PrintSetupEvent::SetOverhang(50.0)]);

    let mut h = blank();
    type_into(&mut h, 3, "95", true);
    assert!(h.state().events.is_empty());
    assert_eq!(h.state().texts.overhang, "45");
}

#[test]
fn the_check_box_and_the_check_button_report() {
    let mut h = blank();
    h.get_by_label("Show bed outline").click();
    h.run();
    h.get_by_label("Check fit").click();
    h.run();
    assert_eq!(
        h.state().events,
        vec![PrintSetupEvent::ShowBed(true), PrintSetupEvent::CheckFit]
    );
}

#[test]
fn drop_to_bed_is_a_button_that_needs_a_selection() {
    let mut h = harness(PrintSetupTexts::default(), false, None);
    h.get_by_label("Drop to Bed").click();
    h.run();
    assert!(h.state().events.is_empty(), "disabled");
    let mut h = blank();
    h.get_by_label("Drop to Bed").click();
    h.run();
    assert_eq!(h.state().events, vec![PrintSetupEvent::DropToBed]);
}

#[test]
fn the_fit_message_is_shown() {
    let report = FitReport {
        fits: false,
        size_mm: Some([300.0, 10.0, 10.0]),
        message: "Too big for the bed: 300 x 10 x 10 mm on 220 x 220 mm (X over by 80 mm)".into(),
        skipped: 0,
    };
    let message = report.message.clone();
    let h = harness(PrintSetupTexts::default(), true, Some(report));
    h.get_by_label(&message);
    let h = blank();
    assert!(h.query_by_label_contains("Too big").is_none());
}

#[test]
fn texts_come_from_the_view() {
    let t = PrintSetupTexts::for_view(
        &Bed {
            x_mm: 256.0,
            y_mm: 250.5,
            z_mm: None,
        },
        52.0,
    );
    assert_eq!(
        (
            t.x.as_str(),
            t.y.as_str(),
            t.z.as_str(),
            t.overhang.as_str()
        ),
        ("256", "250.5", "", "52")
    );
}
