// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m10-start-state.md`: the app starts on a new empty
//! model with Select active, the same state as File > New.

use drydock::document::Document;
use drydock::editor::{Editor, Viewport};
use drydock::input::{Modifiers, Tool};
use drydock::print_setup::{Bed, PrintSettings};
use drydock::start::Fresh;
use drydock::viewstate::ViewState;
use drydock_view::{Bounds, Camera, Projection, StandardView};
use glam::{DVec2, DVec3};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

fn top_view() -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(StandardView::Top);
    camera.projection = Projection::Parallel;
    camera.zoom_extents(
        &Bounds {
            min: DVec3::splat(-5.0),
            max: DVec3::splat(5.0),
        },
        800.0 / 600.0,
    );
    Viewport {
        camera,
        size_px: (800.0, 600.0),
    }
}

fn px(view: &Viewport, x: f64, y: f64) -> DVec2 {
    view.project(DVec3::new(x, y, 0.0)).expect("visible")
}

#[test]
fn a_new_editor_and_view_start_with_select() {
    assert_eq!(Editor::new().tool(), Tool::Select);
    assert_eq!(Editor::default().tool(), Tool::Select);
    assert_eq!(ViewState::new().tool, Tool::Select);
}

#[test]
fn fresh_is_an_empty_untitled_model_with_select() {
    let fresh = Fresh::new(&PrintSettings::default());
    assert!(fresh.document.kernel.is_some(), "a kernel model to draw in");
    assert!(fresh.document.untitled);
    assert!(!fresh.document.modified);
    assert_eq!(fresh.title, Document::new_empty().title());
    assert_eq!(fresh.title, fresh.document.title());
    assert_eq!(fresh.editor.tool(), Tool::Select);
    assert_eq!(fresh.view.tool, Tool::Select);
    assert!(fresh.editor.selection().is_empty());
    assert_eq!(fresh.editor.undo_stack().undo_count(), 0);
}

#[test]
fn fresh_takes_the_bed_and_overhang_from_the_print_settings() {
    let mut print = PrintSettings::default();
    print.bed = Bed {
        x_mm: 123.0,
        y_mm: 77.0,
        ..print.bed
    };
    print.show_bed = true;
    print.overhang_degrees = 33.0;
    let fresh = Fresh::new(&print);
    assert_eq!(fresh.view.bed, print.bed);
    assert!(fresh.view.bed_visible);
    assert_eq!(fresh.view.overhang_degrees, 33.0);

    let plain = Fresh::new(&PrintSettings::default());
    assert!(!plain.view.bed_visible);
}

#[test]
fn a_rectangle_drawn_right_after_start_adds_a_face() {
    let mut fresh = Fresh::new(&PrintSettings::default());
    let mut model = fresh.document.kernel.take().expect("kernel model");
    let view = top_view();
    fresh.editor.set_tool(Tool::Rectangle);
    let a = px(&view, 0.0, 0.0);
    let b = px(&view, 3.0, 2.0);
    fresh.editor.pointer_move(&mut model, &view, a, NONE);
    assert!(!fresh.editor.click(&mut model, &view, a, NONE, 1));
    fresh.editor.pointer_move(&mut model, &view, b, NONE);
    assert!(fresh.editor.click(&mut model, &view, b, NONE, 1));
    assert_eq!(model.root().geometry().face_count(), 1);
    assert_eq!(model.root().geometry().edge_count(), 4);
    assert_eq!(fresh.editor.undo_stack().undo_count(), 1);
}
