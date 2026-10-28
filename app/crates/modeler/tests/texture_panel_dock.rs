// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-texture-panel: no doubled heading and a View menu item, driven with egui_kittest.
//! See docs/specs/m6-texture-panel.md.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use modeler::chrome::{MenuState, menu_bar};
use modeler::dock::{DockLayout, PANELS, PanelId, Side, panel_key, title};
use modeler::input::Action;
use modeler::texture_panel::{TextureForm, texture_panel, texture_position_section};
use modeler_kernel::units::Unit;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn the_panel_draws_no_heading_of_its_own() {
    let mut form = TextureForm::default();
    let mut h = Harness::new_ui(move |ui| {
        texture_panel(ui, &mut form, true, Unit::Meter);
    });
    h.run();
    assert!(h.query_by_label("Texture Position").is_none());
    h.get_by_label("Reset Texture");
}

#[test]
fn the_section_draws_no_heading_and_keeps_the_button_above_the_controls() {
    let mut form = TextureForm::default();
    let mut h = Harness::new_ui(move |ui| {
        texture_position_section(ui, &mut form, true, false, Unit::Meter);
    });
    h.run();
    assert!(h.query_by_label("Texture Position").is_none());
    let button = h.get_by_label("Position in Viewport").rect();
    let reset = h.get_by_label("Reset Texture").rect();
    assert!(reset.top() >= button.bottom(), "{button:?} {reset:?}");
}

#[test]
fn the_panel_is_last_closed_and_docked_on_the_right() {
    assert_eq!(PANELS.last(), Some(&PanelId::TexturePosition));
    assert_eq!(title(PanelId::TexturePosition), "Texture Position");
    assert_eq!(panel_key(PanelId::TexturePosition), "texture-position");
    let layout = DockLayout::default();
    assert!(!layout.is_open(PanelId::TexturePosition));
    assert!(
        !layout
            .docked(Side::Right)
            .contains(&PanelId::TexturePosition)
    );
    let mut layout = layout;
    layout.set_open(PanelId::TexturePosition, true);
    assert_eq!(
        layout.docked(Side::Right).last(),
        Some(&PanelId::TexturePosition)
    );
}

type Seen = Rc<RefCell<Vec<Action>>>;

fn view_menu(open: bool) -> (Harness<'static>, Seen) {
    let seen: Seen = Rc::default();
    let sink = seen.clone();
    let state = MenuState {
        texture_position: open,
        ..MenuState::default()
    };
    let mut h = Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            sink.borrow_mut().push(action);
        }
    });
    h.run();
    h.get_by_label("View").click();
    h.run();
    (h, seen)
}

#[test]
fn the_view_menu_offers_the_panel() {
    let (mut h, seen) = view_menu(false);
    h.get_by_label("Texture Position").click();
    h.run();
    assert_eq!(
        *seen.borrow(),
        vec![Action::TogglePanel(PanelId::TexturePosition)]
    );
}

#[test]
fn the_view_menu_item_sits_after_shadows() {
    let (h, _) = view_menu(true);
    let shadows = h.get_by_label("Shadows").rect();
    let texture = h.get_by_label("Texture Position").rect();
    assert!(
        texture.top() >= shadows.bottom() - 1.0,
        "{shadows:?} {texture:?}"
    );
}
