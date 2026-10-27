// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-ui-materials-layout: buttons and search above a scrolling list or tiles.
//! See docs/specs/m4-ui-materials-layout.md.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use modeler::materials_panel::{MaterialAction, MaterialsPanel, matches_search, materials_panel};
use modeler_kernel::model::{Material, MaterialId, Model, ModelEdit};

const ROBOTO: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");
const WIDTH: f32 = 160.0;
const HEIGHT: f32 = 400.0;

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::empty();
    fonts.font_data.insert(
        "Roboto".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(ROBOTO)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.insert(family, vec!["Roboto".to_owned()]);
    }
    ctx.set_fonts(fonts);
}

struct State {
    panel: MaterialsPanel,
    model: Model,
    current: Option<MaterialId>,
    actions: Vec<MaterialAction>,
    /// Size the panel's contents took in the last frame.
    used: egui::Vec2,
}

fn model_with(names: &[String]) -> (Model, Vec<MaterialId>) {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let ids = names
        .iter()
        .map(|name| model.add_material(Material::new(name, [0.5, 0.4, 0.3, 1.0]), &mut log))
        .collect();
    (model, ids)
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// The panel in a column `WIDTH` wide and `HEIGHT` tall, as the app's right panel gives it.
fn harness(model: Model, current: Option<MaterialId>) -> Harness<'static, State> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(600.0, 800.0))
        .build_ui_state(
            |ui, state: &mut State| {
                let response = ui.allocate_ui(egui::vec2(WIDTH, HEIGHT), |ui| {
                    ui.set_max_width(WIDTH);
                    ui.set_max_height(HEIGHT);
                    if let Some(action) =
                        materials_panel(ui, &mut state.panel, &state.model, state.current)
                    {
                        state.actions.push(action);
                    }
                    ui.min_rect().size()
                });
                state.used = response.inner;
            },
            State {
                panel: MaterialsPanel::default(),
                model,
                current,
                actions: Vec::new(),
                used: egui::Vec2::ZERO,
            },
        );
    install_fonts(&h.ctx);
    h.run();
    h
}

fn many(count: usize) -> Vec<String> {
    (0..count).map(|i| format!("Material {i:03}")).collect()
}

// ---------------------------------------------------------------------------------------
// matches_search

#[test]
fn matches_search_rules() {
    assert!(matches_search("Brick", ""));
    assert!(matches_search("Brick", "   "));
    assert!(matches_search("Brick", "bri"));
    assert!(matches_search("Brick", "BRICK"));
    assert!(matches_search("Red Brick Wall", "brick"));
    assert!(matches_search("Red Brick Wall", "  brick "));
    assert!(matches_search("Brick", "ck"));
    assert!(!matches_search("Brick", "oak"));
    assert!(!matches_search("Brick", "bricks"));
    assert!(!matches_search("", "a"));
    assert!(matches_search("", ""));
    assert!(matches_search("Äpfel", "äPF"));
}

// ---------------------------------------------------------------------------------------
// Layout

#[test]
fn buttons_search_and_view_toggle_sit_above_the_list() {
    let (model, _) = model_with(&names(&["Brick", "Oak"]));
    let h = harness(model, None);
    let new = h.get_by_label("New...").rect();
    let edit = h.get_by_label("Edit...").rect();
    let purge = h.get_by_label("Purge Unused").rect();
    let list = h.get_by_label("List").rect();
    let tiles = h.get_by_label("Tiles").rect();
    let default = h.get_by_label("Default").rect();
    let brick = h.get_by_label("Brick").rect();
    assert!(
        h.query_by_label("Materials").is_none(),
        "the dock header carries the title"
    );
    for button in [new, edit, purge] {
        assert!(button.bottom() <= list.top() + 0.5, "{button:?} {list:?}");
    }
    assert!(list.top() <= tiles.bottom());
    assert!((list.center().y - tiles.center().y).abs() < 1.0, "one row");
    assert!(tiles.bottom() <= default.top() + 0.5);
    assert!(default.bottom() <= brick.top() + 0.5);
}

#[test]
fn the_search_box_is_between_the_buttons_and_the_view_toggle() {
    let (model, _) = model_with(&names(&["Brick"]));
    let h = harness(model, None);
    let search = h.get_by_role(egui::accesskit::Role::TextInput).rect();
    assert!(h.get_by_label("New...").rect().bottom() <= search.top() + 0.5);
    assert!(search.bottom() <= h.get_by_label("List").rect().top() + 0.5);
    assert!(search.width() > WIDTH * 0.6, "{search:?}");
}

#[test]
fn many_materials_do_not_push_the_buttons_or_grow_the_panel() {
    let (few, _) = model_with(&names(&["Brick"]));
    let few = harness(few, None);
    let (lots, _) = model_with(&many(300));
    let lots = harness(lots, None);
    for label in ["New...", "Edit...", "Purge Unused", "List", "Tiles"] {
        assert_eq!(
            few.get_by_label(label).rect().top(),
            lots.get_by_label(label).rect().top(),
            "{label}"
        );
    }
    assert!(
        lots.state().used.y <= HEIGHT + 0.5,
        "{:?}",
        lots.state().used
    );
    assert!(
        lots.state().used.x <= WIDTH + 0.5,
        "{:?}",
        lots.state().used
    );
}

#[test]
fn the_list_scrolls_under_the_fixed_buttons() {
    let (model, _) = model_with(&many(300));
    let mut h = harness(model, None);
    let top_before = h.get_by_label("New...").rect().top();
    h.get_by_label("Material 000").hover();
    h.step();
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, -300.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    h.run_steps(10);
    assert_eq!(h.get_by_label("New...").rect().top(), top_before);
    h.get_by_role(egui::accesskit::Role::TextInput);
    // The first rows scrolled out of the area (clipped away or off screen).
    let first = h.query_by_label("Material 000");
    let moved =
        first.is_none_or(|n| n.rect().bottom() <= h.get_by_label("Tiles").rect().bottom() + 0.5);
    assert!(moved, "the list did not scroll");
}

// ---------------------------------------------------------------------------------------
// Search

#[test]
fn search_lists_only_matching_materials() {
    let (model, _) = model_with(&names(&["Brick", "Oak Floor", "Red Brick", "Glass"]));
    let mut h = harness(model, None);
    h.state_mut().panel.search = "brick".to_string();
    h.run();
    h.get_by_label("Brick");
    h.get_by_label("Red Brick");
    assert!(h.query_by_label("Oak Floor").is_none());
    assert!(h.query_by_label("Glass").is_none());
    assert!(h.query_by_label("Default").is_none());
    h.get_by_label("New...");
}

#[test]
fn clearing_the_search_lists_everything_again() {
    let (model, _) = model_with(&names(&["Brick", "Oak Floor"]));
    let mut h = harness(model, None);
    h.state_mut().panel.search = "oak".to_string();
    h.run();
    assert!(h.query_by_label("Brick").is_none());
    h.state_mut().panel.search = String::new();
    h.run();
    h.get_by_label("Brick");
    h.get_by_label("Oak Floor");
    h.get_by_label("Default");
}

#[test]
fn default_shows_when_the_search_matches_its_name() {
    let (model, _) = model_with(&names(&["Brick", "Oak Floor"]));
    let mut h = harness(model, None);
    h.state_mut().panel.search = "DEF".to_string();
    h.run();
    h.get_by_label("Default");
    assert!(h.query_by_label("Brick").is_none());
}

#[test]
fn search_with_no_match_lists_nothing_and_sends_nothing() {
    let (model, ids) = model_with(&names(&["Brick", "Oak Floor"]));
    let mut h = harness(model, Some(ids[0]));
    h.state_mut().panel.search = "zzz".to_string();
    h.run();
    assert!(h.query_by_label("Brick").is_none());
    assert!(h.query_by_label("Oak Floor").is_none());
    assert!(h.query_by_label("Default").is_none());
    assert!(h.state().actions.is_empty());
    assert_eq!(h.state().current, Some(ids[0]));
}

#[test]
fn typing_in_the_search_box_filters() {
    let (model, _) = model_with(&names(&["Brick", "Oak Floor"]));
    let mut h = harness(model, None);
    h.get_by_role(egui::accesskit::Role::TextInput).click();
    h.run();
    h.get_by_role(egui::accesskit::Role::TextInput)
        .type_text("oak");
    h.run();
    assert_eq!(h.state().panel.search, "oak");
    assert!(h.query_by_label("Brick").is_none());
    h.get_by_label("Oak Floor");
    assert!(h.state().actions.is_empty());
}

#[test]
fn a_filtered_row_still_chooses_its_material() {
    let (model, ids) = model_with(&names(&["Brick", "Oak Floor"]));
    let mut h = harness(model, None);
    h.state_mut().panel.search = "oak".to_string();
    h.run();
    h.get_by_label("Oak Floor").click();
    h.run();
    assert_eq!(
        h.state().actions,
        vec![MaterialAction::Choose(Some(ids[1]))]
    );
}

// ---------------------------------------------------------------------------------------
// List and tiles

#[test]
fn the_list_is_the_default_view() {
    let (model, _) = model_with(&names(&["Brick"]));
    let h = harness(model, None);
    assert!(!h.state().panel.tiles);
}

#[test]
fn the_toggle_switches_views() {
    let (model, _) = model_with(&names(&["Brick"]));
    let mut h = harness(model, None);
    h.get_by_label("Tiles").click();
    h.run();
    assert!(h.state().panel.tiles);
    h.get_by_label("List").click();
    h.run();
    assert!(!h.state().panel.tiles);
    assert!(h.state().actions.is_empty());
}

#[test]
fn the_toggle_draws_the_current_view_selected() {
    use egui::accesskit::Toggled;
    let (model, _) = model_with(&names(&["Brick"]));
    let mut h = harness(model, None);
    let selected = |h: &Harness<'_, State>, label: &str| {
        h.get_by_label(label).accesskit_node().toggled() == Some(Toggled::True)
    };
    assert!(selected(&h, "List") && !selected(&h, "Tiles"));
    h.state_mut().panel.tiles = true;
    h.run();
    assert!(!selected(&h, "List") && selected(&h, "Tiles"));
}

#[test]
fn list_view_has_one_row_per_line() {
    let (model, _) = model_with(&names(&["Brick", "Oak", "Glass", "Tile"]));
    let h = harness(model, None);
    let tops: Vec<f32> = ["Brick", "Oak", "Glass", "Tile"]
        .iter()
        .map(|n| h.get_by_label(n).rect().top())
        .collect();
    assert!(tops.windows(2).all(|w| w[0] < w[1]), "{tops:?}");
}

#[test]
fn tiles_flow_left_to_right_and_wrap() {
    let (model, _) = model_with(&names(&["Brick", "Oak", "Glass", "Tile"]));
    let mut h = harness(model, None);
    h.state_mut().panel.tiles = true;
    h.run();
    let default = h.get_by_label("Default").rect();
    let brick = h.get_by_label("Brick").rect();
    let oak = h.get_by_label("Oak").rect();
    let glass = h.get_by_label("Glass").rect();
    // Default is the first tile; the next one is beside it, in the same line.
    assert!(
        (default.top() - brick.top()).abs() < 1.0,
        "{default:?} {brick:?}"
    );
    assert!(brick.left() > default.left());
    // A 160 pt column holds two 64 pt tiles, not three: Oak starts a new line.
    assert!(oak.top() > brick.top() + 20.0, "{brick:?} {oak:?}");
    assert!(oak.left() < brick.left());
    assert!((oak.top() - glass.top()).abs() < 1.0);
    // Every tile stays inside the column.
    for rect in [default, brick, oak, glass] {
        assert!(rect.right() <= WIDTH + 0.5, "{rect:?}");
    }
}

#[test]
fn clicking_a_tile_chooses_its_material() {
    let (model, ids) = model_with(&names(&["Brick", "Oak"]));
    let mut h = harness(model, None);
    h.state_mut().panel.tiles = true;
    h.run();
    h.get_by_label("Oak").click();
    h.run();
    h.get_by_label("Default").click();
    h.run();
    assert_eq!(
        h.state().actions,
        vec![
            MaterialAction::Choose(Some(ids[1])),
            MaterialAction::Choose(None),
        ]
    );
}

#[test]
fn the_current_tile_is_drawn_selected() {
    use egui::accesskit::Toggled;
    let (model, ids) = model_with(&names(&["Brick", "Oak"]));
    let mut h = harness(model, Some(ids[1]));
    h.state_mut().panel.tiles = true;
    h.run();
    let selected =
        |label: &str| h.get_by_label(label).accesskit_node().toggled() == Some(Toggled::True);
    assert!(selected("Oak"));
    assert!(!selected("Brick"));
    assert!(!selected("Default"));
}

#[test]
fn search_applies_to_tiles_too() {
    let (model, _) = model_with(&names(&["Brick", "Oak"]));
    let mut h = harness(model, None);
    h.state_mut().panel.tiles = true;
    h.state_mut().panel.search = "oak".to_string();
    h.run();
    h.get_by_label("Oak");
    assert!(h.query_by_label("Brick").is_none());
    assert!(h.query_by_label("Default").is_none());
}

#[test]
fn long_names_do_not_widen_the_column_in_either_view() {
    let long =
        "Wood_Floor_Light_Oak_Planks_With_A_Very_Long_Exported_Name_From_Some_Library_0123456789";
    let (model, ids) = model_with(&names(&["Brick", long]));
    for tiles in [false, true] {
        let mut h = harness(model.clone(), Some(ids[1]));
        h.state_mut().panel.tiles = tiles;
        h.run();
        assert!(
            h.state().used.x <= WIDTH + 0.5,
            "tiles {tiles}: {:?}",
            h.state().used
        );
        h.get_by_label_contains("Wood_Floor_Light").click();
        h.run();
        assert_eq!(
            h.state().actions.last(),
            Some(&MaterialAction::Choose(Some(ids[1])))
        );
    }
}

#[test]
fn the_form_sits_in_the_scrolling_part_below_the_toggle() {
    let (model, ids) = model_with(&names(&["Brick"]));
    let mut h = harness(model, Some(ids[0]));
    h.get_by_label("Edit...").click();
    h.run();
    assert!(h.state().panel.form.is_some());
    let toggle = h.get_by_label("Tiles").rect();
    let form = h.get_by_label("Edit Material").rect();
    let brick = h.get_by_label("Brick").rect();
    assert!(toggle.bottom() <= form.top() + 0.5);
    assert!(form.bottom() <= brick.top() + 0.5, "{form:?} {brick:?}");
    assert!(h.state().used.y <= HEIGHT + 0.5);
}
