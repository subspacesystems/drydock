// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-ui-status: Model Info window, measurement in the status bar, Model Info menu item.
//! See docs/specs/m4-ui-status.md.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use modeler::chrome::{
    MenuState, menu_bar, model_info_lines, model_info_window, status_measurement,
};
use modeler::document::Stats;
use modeler::input::Action;
use modeler::viewstate::{Effect, ViewState};
use std::cell::RefCell;
use std::rc::Rc;

fn stats() -> Stats {
    Stats {
        triangles: 120,
        lines: 7,
        materials: 3,
        size_m: [4.115, 6.706, 2.438],
        faces: 40,
        edges: 95,
    }
}

#[test]
fn lines_without_a_kernel_model_list_the_scene_numbers() {
    assert_eq!(
        model_info_lines(&stats(), false),
        vec![
            "Triangles: 120".to_owned(),
            "Lines: 7".to_owned(),
            "Materials: 3".to_owned(),
            "Size: 4.115 x 6.706 x 2.438 m".to_owned(),
        ]
    );
}

#[test]
fn lines_with_a_kernel_model_add_faces_and_edges() {
    assert_eq!(
        model_info_lines(&stats(), true),
        vec![
            "Triangles: 120".to_owned(),
            "Lines: 7".to_owned(),
            "Materials: 3".to_owned(),
            "Size: 4.115 x 6.706 x 2.438 m".to_owned(),
            "Faces: 40".to_owned(),
            "Edges: 95".to_owned(),
        ]
    );
}

#[test]
fn window_shows_every_line_and_close_clears_open() {
    let open = Rc::new(RefCell::new(true));
    let flag = open.clone();
    let lines = model_info_lines(&stats(), true);
    let shown = lines.clone();
    let mut h = Harness::new_ui(move |ui| {
        let mut value = *flag.borrow();
        model_info_window(ui.ctx(), &mut value, &shown);
        *flag.borrow_mut() = value;
    });
    h.run();
    for line in &lines {
        assert!(h.query_by_label(line).is_some(), "{line}");
    }
    h.get_by_label("Close").click();
    h.run();
    assert!(!*open.borrow());
    assert!(h.query_by_label("Faces: 40").is_none());
}

#[test]
fn window_draws_nothing_while_closed() {
    let lines = model_info_lines(&stats(), true);
    let mut h = Harness::new_ui(move |ui| {
        let mut open = false;
        model_info_window(ui.ctx(), &mut open, &lines);
        assert!(!open);
    });
    h.run();
    assert!(h.query_by_label("Triangles: 120").is_none());
    assert!(h.query_by_label("Close").is_none());
}

fn measurement_harness(
    measurement: Option<(&'static str, &'static str)>,
    typed: &'static str,
) -> Harness<'static> {
    let mut h = Harness::new_ui(move |ui| {
        ui.horizontal(|ui| status_measurement(ui, measurement, typed));
    });
    h.run();
    h
}

#[test]
fn measurement_shows_label_and_value() {
    let h = measurement_harness(Some(("Length", "1.50 m")), "");
    assert!(h.query_by_label("Length").is_some());
    assert!(h.query_by_label("1.50 m").is_some());
}

#[test]
fn measurement_shows_typed_text() {
    let h = measurement_harness(Some(("Distance", "")), "2.5");
    assert!(h.query_by_label("Distance").is_some());
    assert!(h.query_by_label("2.5").is_some());
}

#[test]
fn nothing_to_measure_does_not_panic() {
    let mut h = Harness::new_ui(|ui| {
        status_measurement(ui, None, "");
    });
    h.run();
    assert!(h.query_by_label("Length").is_none());
}

#[test]
fn typed_text_without_a_measurement_still_shows() {
    let h = measurement_harness(None, "5'4");
    assert!(h.query_by_label("5'4").is_some());
}

#[test]
fn model_info_action_gives_its_effect_and_leaves_the_view() {
    let mut view = ViewState::new();
    let camera = view.camera;
    assert_eq!(
        view.apply(Action::ModelInfo, (800.0, 600.0)),
        Effect::ModelInfo
    );
    assert_eq!(view.camera, camera);
}

fn choose_model_info() -> Vec<Action> {
    let seen: Rc<RefCell<Vec<Action>>> = Rc::default();
    let sink = seen.clone();
    let state = MenuState::default();
    let mut h = Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            sink.borrow_mut().push(action);
        }
    });
    h.run();
    h.get_by_label("View").click();
    h.run();
    h.get_by_label("Model Info").click();
    h.run();
    seen.borrow().clone()
}

#[test]
fn the_view_menu_offers_model_info() {
    assert_eq!(choose_model_info(), vec![Action::ModelInfo]);
}
