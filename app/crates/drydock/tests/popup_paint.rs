// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m12-popup-paint: the pure parts of painting a window pop-up before it shows. See
//! docs/specs/m12-popup-paint.md.

use drydock::popup::{ShowStep, initial_size, popup_fill_rect, popup_frame, show_step};
use eframe::egui::{self, Rect, pos2, vec2};

#[test]
fn frame_is_square_without_shadow() {
    let style = egui::Style::default();
    let frame = popup_frame(&style);
    let menu = egui::Frame::menu(&style);
    assert_eq!(frame.corner_radius, egui::CornerRadius::ZERO);
    assert_eq!(frame.shadow, egui::Shadow::NONE);
    assert_eq!(frame.fill, menu.fill, "same fill as a menu");
    assert_eq!(frame.stroke.width, 1.0, "1 px border");
    assert_eq!(frame.inner_margin, menu.inner_margin);
}

#[test]
fn frame_follows_the_light_and_dark_styles() {
    let style = |visuals| egui::Style {
        visuals,
        ..egui::Style::default()
    };
    let dark = popup_frame(&style(egui::Visuals::dark()));
    let light = popup_frame(&style(egui::Visuals::light()));
    assert_ne!(dark.fill, light.fill);
}

#[test]
fn fill_covers_the_whole_viewport_rect() {
    let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(133.5, 77.25));
    assert_eq!(popup_fill_rect(r), r);
}

#[test]
fn initial_size_uses_the_cache_or_the_placeholder() {
    assert_eq!(initial_size(Some(vec2(210.0, 305.0))), vec2(210.0, 305.0));
    assert_eq!(initial_size(None), vec2(120.0, 40.0));
}

#[test]
fn hidden_wrong_size_resizes_first() {
    let target = vec2(200.0, 300.0);
    assert_eq!(
        show_step(false, vec2(120.0, 40.0), None, target, 0),
        ShowStep::Resize(target)
    );
    // A frame painted at the old size does not count.
    assert_eq!(
        show_step(false, vec2(120.0, 40.0), Some(vec2(120.0, 40.0)), target, 1),
        ShowStep::Resize(target)
    );
}

#[test]
fn hidden_right_size_paints_once_then_reveals() {
    let target = vec2(200.0, 300.0);
    assert_eq!(show_step(false, target, None, target, 0), ShowStep::Wait);
    assert_eq!(
        show_step(false, target, Some(target), target, 1),
        ShowStep::Reveal
    );
    // Within the resize tolerance counts as the same size.
    assert_eq!(
        show_step(
            false,
            vec2(200.4, 300.0),
            Some(vec2(200.0, 299.6)),
            target,
            1
        ),
        ShowStep::Reveal
    );
}

#[test]
fn painted_at_another_size_waits() {
    let target = vec2(200.0, 300.0);
    assert_eq!(
        show_step(false, target, Some(vec2(150.0, 300.0)), target, 2),
        ShowStep::Wait
    );
}

#[test]
fn never_visible_before_a_paint() {
    // Hidden, correct size, nothing painted yet: never Reveal or Keep.
    let target = vec2(80.0, 60.0);
    for hidden in 0..5 {
        let step = show_step(false, target, None, target, hidden);
        assert_eq!(step, ShowStep::Wait, "frame {hidden}");
    }
}

#[test]
fn a_window_that_never_reports_its_size_still_appears() {
    let target = vec2(80.0, 60.0);
    assert_eq!(
        show_step(false, vec2(120.0, 40.0), None, target, 5),
        ShowStep::Reveal
    );
    assert_eq!(
        show_step(false, vec2(120.0, 40.0), None, target, 50),
        ShowStep::Reveal
    );
}

#[test]
fn visible_keeps_or_follows_content_changes() {
    let target = vec2(200.0, 300.0);
    assert_eq!(show_step(true, target, None, target, 0), ShowStep::Keep);
    assert_eq!(
        show_step(true, target, Some(target), vec2(200.0, 340.0), 3),
        ShowStep::Resize(vec2(200.0, 340.0))
    );
    assert_eq!(
        show_step(true, vec2(200.5, 300.4), None, target, 0),
        ShowStep::Keep
    );
}

#[test]
fn full_sequence_shows_at_the_final_size_after_two_hidden_paints() {
    let target = vec2(180.0, 220.0);
    let mut current = initial_size(None);
    let mut painted = None;
    let mut visible = false;
    let mut hidden = 0;
    let mut sizes_when_shown = vec![];
    for _ in 0..8 {
        match show_step(visible, current, painted, target, hidden) {
            ShowStep::Resize(t) => {
                current = t;
                painted = None;
            }
            ShowStep::Wait => painted = Some(current),
            ShowStep::Reveal => {
                visible = true;
                sizes_when_shown.push(current);
            }
            ShowStep::Keep => {}
        }
        if !visible {
            hidden += 1;
        }
    }
    assert_eq!(
        sizes_when_shown,
        vec![target],
        "shown once, at the final size"
    );
}

#[test]
fn a_cached_size_shows_one_frame_sooner() {
    let target = vec2(180.0, 220.0);
    let start = initial_size(Some(target));
    assert_eq!(show_step(false, start, None, target, 0), ShowStep::Wait);
    assert_eq!(
        show_step(false, start, Some(start), target, 1),
        ShowStep::Reveal
    );
}
