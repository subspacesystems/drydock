// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-fix-materials-width: long material names don't widen the Materials panel.
//! See docs/specs/m4-fix-materials-width.md.

use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use modeler::materials_panel::{MaterialAction, MaterialsPanel, materials_panel};
use modeler_kernel::model::{Material, MaterialId, Model, ModelEdit};

const WIDTH: f32 = 160.0;

/// The app's font (the workspace builds egui without its bundled fonts), so text has its
/// real width.
const ROBOTO: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");

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
    /// Width the panel's contents took in the last frame.
    used: f32,
}

fn long_name() -> String {
    "Wood_Floor_Light_Oak_Planks_With_A_Very_Long_Exported_Name_From_Some_Library_0123456789"
        .to_string()
}

fn model_with(names: &[&str]) -> (Model, Vec<MaterialId>) {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let ids = names
        .iter()
        .map(|name| model.add_material(Material::new(name, [0.5, 0.5, 0.5, 1.0]), &mut log))
        .collect();
    (model, ids)
}

/// The panel drawn in a column `WIDTH` points wide, as the app's right panel gives it.
fn harness(model: Model, current: Option<MaterialId>) -> Harness<'static, State> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(600.0, 800.0))
        .build_ui_state(
            |ui, state: &mut State| {
                let response = ui.allocate_ui(egui::vec2(WIDTH, 760.0), |ui| {
                    ui.set_max_width(WIDTH);
                    if let Some(action) =
                        materials_panel(ui, &mut state.panel, &state.model, state.current)
                    {
                        state.actions.push(action);
                    }
                    ui.min_rect().width()
                });
                state.used = response.inner;
            },
            State {
                panel: MaterialsPanel::default(),
                model,
                current,
                actions: Vec::new(),
                used: 0.0,
            },
        );
    install_fonts(&h.ctx);
    h.run();
    h
}

#[test]
fn short_names_fit() {
    let (model, _) = model_with(&["Brick", "Oak"]);
    let h = harness(model, None);
    assert!(h.state().used <= WIDTH + 0.5, "used {}", h.state().used);
}

#[test]
fn a_long_name_does_not_widen_the_panel() {
    let name = long_name();
    let (model, _) = model_with(&["Brick", &name]);
    let h = harness(model, None);
    assert!(h.state().used <= WIDTH + 0.5, "used {}", h.state().used);
}

#[test]
fn a_long_current_name_does_not_widen_the_panel() {
    let name = long_name();
    let (model, ids) = model_with(&[&name]);
    let h = harness(model, Some(ids[0]));
    assert!(h.state().used <= WIDTH + 0.5, "used {}", h.state().used);
}

#[test]
fn editing_a_long_named_material_does_not_widen_the_panel() {
    let name = long_name();
    let (model, ids) = model_with(&[&name]);
    let mut h = harness(model, Some(ids[0]));
    h.get_by_label("Edit...").click();
    h.run();
    assert!(h.state().panel.form.is_some(), "the form opened");
    assert!(h.state().used <= WIDTH + 0.5, "used {}", h.state().used);
}

#[test]
fn a_long_texture_path_in_the_form_does_not_widen_the_panel() {
    let (model, ids) = model_with(&["Brick"]);
    let mut h = harness(model, Some(ids[0]));
    h.get_by_label("Edit...").click();
    h.run();
    h.state_mut().panel.form.as_mut().unwrap().texture = format!(
        "/home/someone/projects/a/very/deep/folder/structure/textures/{}.png",
        long_name()
    );
    h.run();
    assert!(h.state().used <= WIDTH + 0.5, "used {}", h.state().used);
}

#[test]
fn a_long_named_row_still_chooses_its_material() {
    let name = long_name();
    let (model, ids) = model_with(&["Brick", &name]);
    let mut h = harness(model, None);
    // The row is found by the start of its name, whether or not the rest is cut short.
    h.get_by_label_contains("Wood_Floor_Light").click();
    h.run();
    assert_eq!(
        h.state().actions.last(),
        Some(&MaterialAction::Choose(Some(ids[1])))
    );
}

#[test]
fn the_texture_position_panel_fits_the_same_column() {
    use modeler::texture_panel::{TextureForm, texture_panel};
    for available in [false, true] {
        let mut h = Harness::builder()
            .with_size(egui::vec2(600.0, 800.0))
            .build_ui_state(
                move |ui, state: &mut (TextureForm, f32)| {
                    let response = ui.allocate_ui(egui::vec2(WIDTH, 760.0), |ui| {
                        ui.set_max_width(WIDTH);
                        let _ = texture_panel(ui, &mut state.0, available);
                        ui.min_rect().width()
                    });
                    state.1 = response.inner;
                },
                (TextureForm::default(), 0.0),
            );
        install_fonts(&h.ctx);
        h.run();
        assert!(
            h.state().1 <= WIDTH + 0.5,
            "available {available}: used {}",
            h.state().1
        );
    }
}
