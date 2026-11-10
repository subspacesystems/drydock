// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m12-cursor-set.md`: which cursor the viewport asks
//! egui for. Driven with egui_kittest; the request is read back from the context's
//! platform output.

use std::sync::Arc;

use drydock::cursor_set::{apply, cursor_image, effective_tool};
use drydock::cursors;
use drydock::input::Tool;
use eframe::egui;
use egui_kittest::Harness;

const ALL: [Tool; 18] = [
    Tool::Orbit,
    Tool::Pan,
    Tool::Zoom,
    Tool::Select,
    Tool::Line,
    Tool::Rectangle,
    Tool::Circle,
    Tool::Arc,
    Tool::Arc3,
    Tool::PushPull,
    Tool::Rotate,
    Tool::Move,
    Tool::Scale,
    Tool::Tape,
    Tool::Erase,
    Tool::Paint,
    Tool::Offset,
    Tool::FollowMe,
];

/// Run one frame that calls `apply` and return (image, icon) as egui holds them.
fn requested(
    tool: Tool,
    grip: Option<egui::CursorIcon>,
    middle: bool,
    shift: bool,
) -> (Option<egui::CustomCursorImage>, egui::CursorIcon) {
    let mut h = Harness::new_ui(move |ui| apply(ui.ctx(), tool, grip, middle, shift));
    h.run();
    h.ctx.output(|o| (o.cursor_image.clone(), o.cursor_icon))
}

#[test]
fn select_has_no_image() {
    assert!(cursor_image(Tool::Select).is_none());
    let (image, icon) = requested(Tool::Select, None, false, false);
    assert!(image.is_none());
    assert_eq!(icon, egui::CursorIcon::Default);
}

#[test]
fn every_other_tool_gets_its_image_and_hotspot() {
    for tool in ALL.into_iter().filter(|t| *t != Tool::Select) {
        let image = cursor_image(tool).unwrap_or_else(|| panic!("{tool:?}: no image"));
        assert_eq!(image.size, [32, 32], "{tool:?}");
        assert_eq!(image.rgba.len(), 32 * 32 * 4, "{tool:?}");
        let (x, y) = cursors::hotspot(tool).unwrap();
        assert_eq!(image.hotspot, [x as u16, y as u16], "{tool:?}");
        // Same pixels as the PNG, straight alpha.
        let png = image::load_from_memory(cursors::image_png(tool).unwrap())
            .unwrap()
            .to_rgba8();
        assert_eq!(&image.rgba[..], png.as_raw().as_slice(), "{tool:?}");
        // The hotspot pixel is inside the image and not fully transparent.
        let at = ((y * 32 + x) * 4 + 3) as usize;
        assert!(
            image.rgba[at] > 0,
            "{tool:?}: hotspot on a transparent pixel"
        );
    }
}

#[test]
fn images_are_decoded_once() {
    for tool in ALL.into_iter().filter(|t| *t != Tool::Select) {
        let a = cursor_image(tool).unwrap();
        let b = cursor_image(tool).unwrap();
        assert!(Arc::ptr_eq(&a.rgba, &b.rgba), "{tool:?}");
    }
    let line = cursor_image(Tool::Line).unwrap();
    let rect = cursor_image(Tool::Rectangle).unwrap();
    assert!(!Arc::ptr_eq(&line.rgba, &rect.rgba));
}

#[test]
fn apply_requests_the_tools_image() {
    for tool in ALL.into_iter().filter(|t| *t != Tool::Select) {
        // Shift is off, so the Orbit tool stays Orbit.
        let (image, _) = requested(tool, None, false, false);
        let want = cursor_image(tool).unwrap();
        let got = image.unwrap_or_else(|| panic!("{tool:?}: no image requested"));
        assert_eq!(got.size, want.size, "{tool:?}");
        assert_eq!(got.hotspot, want.hotspot, "{tool:?}");
        assert_eq!(&got.rgba[..], &want.rgba[..], "{tool:?}");
    }
}

#[test]
fn tools_have_different_cursors() {
    let line = requested(Tool::Line, None, false, false).0.unwrap();
    let paint = requested(Tool::Paint, None, false, false).0.unwrap();
    assert_ne!(&line.rgba[..], &paint.rgba[..]);
    assert_ne!(line.hotspot, paint.hotspot);
}

#[test]
fn a_grip_wins_over_the_tool_image() {
    for grip in [
        egui::CursorIcon::Move,
        egui::CursorIcon::Grab,
        egui::CursorIcon::ResizeNwSe,
        egui::CursorIcon::ResizeNeSw,
    ] {
        let (image, icon) = requested(Tool::Move, Some(grip), false, false);
        assert!(image.is_none(), "{grip:?}");
        assert_eq!(icon, grip);
    }
}

#[test]
fn the_image_does_not_stick_after_the_tool_changes() {
    // One context over several frames: Line, then Select, then a grip.
    let tool = std::rc::Rc::new(std::cell::Cell::new(Tool::Line));
    let grip = std::rc::Rc::new(std::cell::Cell::new(None));
    let (t, g) = (tool.clone(), grip.clone());
    let mut h = Harness::new_ui(move |ui| apply(ui.ctx(), t.get(), g.get(), false, false));
    h.run();
    assert!(h.ctx.output(|o| o.cursor_image.clone()).is_some());
    tool.set(Tool::Select);
    h.run();
    assert!(h.ctx.output(|o| o.cursor_image.clone()).is_none());
    assert_eq!(h.ctx.output(|o| o.cursor_icon), egui::CursorIcon::Default);
    tool.set(Tool::Rotate);
    h.run();
    assert!(h.ctx.output(|o| o.cursor_image.clone()).is_some());
    grip.set(Some(egui::CursorIcon::Grab));
    h.run();
    assert!(h.ctx.output(|o| o.cursor_image.clone()).is_none());
    assert_eq!(h.ctx.output(|o| o.cursor_icon), egui::CursorIcon::Grab);
}

#[test]
fn middle_drag_and_shift_pick_orbit_or_pan() {
    // (tool, middle, shift) -> shown tool
    for (tool, middle, shift, want) in [
        (Tool::Line, false, false, Tool::Line),
        (Tool::Line, true, false, Tool::Orbit),
        (Tool::Line, true, true, Tool::Pan),
        (Tool::Select, true, false, Tool::Orbit),
        (Tool::Select, true, true, Tool::Pan),
        (Tool::Select, false, true, Tool::Select),
        (Tool::Orbit, false, false, Tool::Orbit),
        (Tool::Orbit, false, true, Tool::Pan),
        (Tool::Pan, false, false, Tool::Pan),
        (Tool::Zoom, false, false, Tool::Zoom),
        (Tool::Zoom, true, false, Tool::Orbit),
        (Tool::Move, false, true, Tool::Move),
    ] {
        assert_eq!(
            effective_tool(tool, middle, shift),
            want,
            "{tool:?} {middle} {shift}"
        );
    }
    // And apply shows that cursor.
    let (image, _) = requested(Tool::Select, None, true, false);
    assert_eq!(
        image.unwrap().hotspot,
        cursor_image(Tool::Orbit).unwrap().hotspot
    );
    let (image, _) = requested(Tool::Line, None, true, true);
    assert_eq!(
        &image.unwrap().rgba[..],
        &cursor_image(Tool::Pan).unwrap().rgba[..]
    );
}
