// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-app-materials-ui: the Materials panel and its form, driven with egui_kittest.
//! See docs/specs/m4-app-materials-ui.md (Purge Unused confirms since
//! docs/specs/m4-purge-confirm.md).

use eframe::egui::accesskit::{Role, Toggled};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use glam::DVec2;
use modeler::materials_panel::{MaterialAction, MaterialForm, MaterialsPanel, materials_panel};
use modeler_kernel::model::{Material, MaterialId, Model, ModelEdit, Texture};

fn add(model: &mut Model, material: Material) -> MaterialId {
    let mut log = ModelEdit::new();
    model.add_material(material, &mut log)
}

fn textured(name: &str, color: [f32; 4], path: &str, size: DVec2) -> Material {
    let mut material = Material::new(name, color);
    material.texture = Some(Texture::new(path, size));
    material
}

/// A model with "Brick" (plain) and "Oak" (textured), and their ids.
fn two_materials() -> (Model, MaterialId, MaterialId) {
    let mut model = Model::new();
    let brick = add(&mut model, Material::new("Brick", [0.7, 0.3, 0.2, 1.0]));
    let oak = add(
        &mut model,
        textured(
            "Oak",
            [1.0, 1.0, 1.0, 1.0],
            "/tex/oak.png",
            DVec2::new(0.5, 2.0),
        ),
    );
    (model, brick, oak)
}

// ---------------------------------------------------------------------------------------
// MaterialForm

#[test]
fn new_form_defaults() {
    let form = MaterialForm::new_material();
    assert_eq!(
        form,
        MaterialForm {
            editing: None,
            name: String::new(),
            color: [0.8, 0.8, 0.8],
            opacity: 1.0,
            texture: String::new(),
            size: [1.0, 1.0],
        }
    );
}

#[test]
fn edit_form_is_filled_from_the_material() {
    let (model, brick, oak) = two_materials();
    let form = MaterialForm::edit(oak, model.material(oak).unwrap());
    assert_eq!(
        form,
        MaterialForm {
            editing: Some(oak),
            name: "Oak".to_string(),
            color: [1.0, 1.0, 1.0],
            opacity: 1.0,
            texture: "/tex/oak.png".to_string(),
            size: [0.5, 2.0],
        }
    );
    let glass = Material::new("Glass", [0.2, 0.4, 0.6, 0.25]);
    let form = MaterialForm::edit(brick, &glass);
    assert_eq!(form.editing, Some(brick));
    assert_eq!(form.name, "Glass");
    assert_eq!(form.color, [0.2, 0.4, 0.6]);
    assert_eq!(form.opacity, 0.25);
    assert_eq!(form.texture, "");
    assert_eq!(form.size, [1.0, 1.0]);
}

#[test]
fn edit_form_gives_back_the_material() {
    let id = MaterialId(7);
    for material in [
        Material::new("Glass", [0.2, 0.4, 0.6, 0.25]),
        Material::new("", [0.0, 1.0, 0.0, 0.0]),
        textured("Oak", [0.9, 0.8, 0.7, 1.0], "oak.png", DVec2::new(0.5, 2.0)),
    ] {
        assert_eq!(
            MaterialForm::edit(id, &material).material(),
            Some(material.clone()),
            "{material:?}"
        );
    }
}

#[test]
fn material_trims_name_and_texture() {
    let mut form = MaterialForm::new_material();
    form.name = "  Walnut ".to_string();
    form.color = [0.4, 0.25, 0.1];
    form.opacity = 0.5;
    form.texture = "  walnut.jpg  ".to_string();
    form.size = [0.3, 1.2];
    assert_eq!(
        form.material(),
        Some(textured(
            "Walnut",
            [0.4, 0.25, 0.1, 0.5],
            "walnut.jpg",
            DVec2::new(0.3, 1.2)
        ))
    );
}

#[test]
fn blank_texture_means_none_whatever_the_size() {
    let mut form = MaterialForm::new_material();
    form.texture = "   ".to_string();
    form.size = [0.0, -1.0];
    let material = form.material().expect("blank texture is valid");
    assert_eq!(material.texture, None);
    assert_eq!(material.name, "", "a blank name is left to the kernel");
    assert_eq!(material.color, [0.8, 0.8, 0.8, 1.0]);
}

#[test]
fn out_of_range_fields_give_none() {
    let base = MaterialForm::new_material();
    let mut cases = Vec::new();
    for opacity in [1.5, -0.1, f32::NAN, f32::INFINITY] {
        let mut form = base.clone();
        form.opacity = opacity;
        cases.push(form);
    }
    for (channel, value) in [(0, 2.0), (1, -0.5), (2, f32::NAN)] {
        let mut form = base.clone();
        form.color[channel] = value;
        cases.push(form);
    }
    for size in [
        [0.0, 1.0],
        [1.0, -2.0],
        [f64::INFINITY, 1.0],
        [1.0, f64::NAN],
    ] {
        let mut form = base.clone();
        form.texture = "oak.png".to_string();
        form.size = size;
        cases.push(form);
    }
    for form in cases {
        assert_eq!(form.material(), None, "{form:?}");
    }
    // The ends of the ranges are fine.
    let mut form = base.clone();
    form.color = [0.0, 1.0, 0.0];
    form.opacity = 0.0;
    assert!(form.material().is_some());
}

// ---------------------------------------------------------------------------------------
// The panel

struct State {
    panel: MaterialsPanel,
    model: Model,
    current: Option<MaterialId>,
    actions: Vec<MaterialAction>,
}

fn harness(model: Model, current: Option<MaterialId>) -> Harness<'static, State> {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut State| {
            if let Some(action) = materials_panel(ui, &mut state.panel, &state.model, state.current)
            {
                state.actions.push(action);
            }
        },
        State {
            panel: MaterialsPanel::default(),
            model,
            current,
            actions: Vec::new(),
        },
    );
    h.run();
    h
}

fn is_selected(h: &Harness<'_, State>, label: &str) -> bool {
    h.get_by_label(label).accesskit_node().toggled() == Some(Toggled::True)
}

#[test]
fn panel_lists_default_then_the_materials_in_order() {
    let (model, _, _) = two_materials();
    let h = harness(model, None);
    assert!(
        h.query_by_label("Materials").is_none(),
        "the dock header carries the title"
    );
    let default = h.get_by_label("Default").rect();
    let brick = h.get_by_label("Brick").rect();
    let oak = h.get_by_label("Oak").rect();
    assert!(
        default.center().y < brick.center().y,
        "{default:?} {brick:?}"
    );
    assert!(brick.center().y < oak.center().y, "{brick:?} {oak:?}");
    h.get_by_label("New...");
    h.get_by_label("Edit...");
    h.get_by_label("Purge Unused");
    assert!(h.state().actions.is_empty());
    assert_eq!(h.state().panel.form, None);
}

#[test]
fn current_row_is_drawn_selected() {
    let (model, brick, _) = two_materials();
    let h = harness(model.clone(), None);
    assert!(is_selected(&h, "Default"));
    assert!(!is_selected(&h, "Brick"));
    assert!(!is_selected(&h, "Oak"));

    let h = harness(model, Some(brick));
    assert!(!is_selected(&h, "Default"));
    assert!(is_selected(&h, "Brick"));
    assert!(!is_selected(&h, "Oak"));
}

#[test]
fn clicking_a_row_chooses_it() {
    let (model, brick, oak) = two_materials();
    let mut h = harness(model, Some(brick));
    h.get_by_label("Oak").click();
    h.run();
    h.get_by_label("Default").click();
    h.run();
    h.get_by_label("Brick").click();
    h.run();
    assert_eq!(
        h.state().actions,
        vec![
            MaterialAction::Choose(Some(oak)),
            MaterialAction::Choose(None),
            MaterialAction::Choose(Some(brick)),
        ]
    );
}

#[test]
fn purge_unused_sends_purge() {
    let (model, _, _) = two_materials();
    let mut h = harness(model, None);
    h.get_by_label("Purge Unused").click();
    h.run();
    assert!(h.state().actions.is_empty(), "it asks first");
    h.get_by_label("Purge").click();
    h.run();
    assert_eq!(h.state().actions, vec![MaterialAction::Purge]);
}

#[test]
fn new_opens_the_form_with_defaults() {
    let (model, _, _) = two_materials();
    let mut h = harness(model, None);
    h.get_by_label("New...").click();
    h.run();
    assert_eq!(h.state().panel.form, Some(MaterialForm::new_material()));
    assert!(h.state().actions.is_empty());
    h.get_by_label("New Material");
    h.get_by_label("Name:");
    h.get_by_label("Color:");
    h.get_by_label("Opacity:");
    h.get_by_label("Texture image:");
    h.get_by_label("Width (in):");
    h.get_by_label("Height (in):");
    h.get_by_label("OK");
    h.get_by_label("Cancel");
}

#[test]
fn edit_needs_a_current_material() {
    let (model, _, _) = two_materials();
    let mut h = harness(model, None);
    let edit = h.get_by_label("Edit...");
    assert!(edit.accesskit_node().is_disabled());
    edit.click();
    h.run();
    assert_eq!(h.state().panel.form, None);
    assert!(h.state().actions.is_empty());
}

#[test]
fn edit_opens_the_form_for_the_current_material() {
    let (model, _, oak) = two_materials();
    let mut h = harness(model.clone(), Some(oak));
    let edit = h.get_by_label("Edit...");
    assert!(!edit.accesskit_node().is_disabled());
    edit.click();
    h.run();
    assert_eq!(
        h.state().panel.form,
        Some(MaterialForm::edit(oak, model.material(oak).unwrap()))
    );
    h.get_by_label("Edit Material");
    assert!(h.state().actions.is_empty());
}

#[test]
fn ok_on_a_new_form_creates_and_closes() {
    let (model, _, _) = two_materials();
    let mut h = harness(model, None);
    let mut form = MaterialForm::new_material();
    form.name = "Pine".to_string();
    form.color = [0.9, 0.8, 0.5];
    form.texture = "pine.png".to_string();
    form.size = [2.0, 1.0];
    h.state_mut().panel.form = Some(form);
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(
        h.state().actions,
        vec![MaterialAction::Create(textured(
            "Pine",
            [0.9, 0.8, 0.5, 1.0],
            "pine.png",
            DVec2::new(2.0, 1.0)
        ))]
    );
    assert_eq!(h.state().panel.form, None);
}

#[test]
fn ok_on_an_edit_form_edits_and_closes() {
    let (model, brick, _) = two_materials();
    let mut h = harness(model.clone(), Some(brick));
    h.get_by_label("Edit...").click();
    h.run();
    if let Some(form) = &mut h.state_mut().panel.form {
        form.color = [0.1, 0.2, 0.3];
        form.opacity = 0.5;
    }
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(
        h.state().actions,
        vec![MaterialAction::Edit(
            brick,
            Material::new("Brick", [0.1, 0.2, 0.3, 0.5])
        )]
    );
    assert_eq!(h.state().panel.form, None);
}

#[test]
fn ok_is_disabled_while_a_field_is_out_of_range() {
    let (model, _, _) = two_materials();
    let mut h = harness(model, None);
    let mut form = MaterialForm::new_material();
    form.texture = "pine.png".to_string();
    form.size = [0.0, 1.0];
    h.state_mut().panel.form = Some(form.clone());
    h.run();
    let ok = h.get_by_label("OK");
    assert!(ok.accesskit_node().is_disabled());
    ok.click();
    h.run();
    assert!(h.state().actions.is_empty());
    assert!(h.state().panel.form.is_some(), "form stays open");
}

#[test]
fn ok_with_a_blank_name_leaves_the_name_to_the_kernel() {
    let (model, _, _) = two_materials();
    let mut h = harness(model, None);
    h.get_by_label("New...").click();
    h.run();
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(
        h.state().actions,
        vec![MaterialAction::Create(Material::new(
            "",
            [0.8, 0.8, 0.8, 1.0]
        ))]
    );
}

#[test]
fn cancel_closes_the_form_without_an_action() {
    let (model, _, _) = two_materials();
    let mut h = harness(model, None);
    h.get_by_label("New...").click();
    h.run();
    h.get_by_label("Cancel").click();
    h.run();
    assert_eq!(h.state().panel.form, None);
    assert!(h.state().actions.is_empty());
}

#[test]
fn name_field_has_focus_when_the_form_opens() {
    let (model, _, _) = two_materials();
    let mut h = harness(model, None);
    h.get_by_label("New...").click();
    h.run();
    h.run();
    // The search box above the list is a text field too; the name field is the one in focus.
    let focused = h
        .get_all_by_role(Role::TextInput)
        .filter(|field| field.is_focused())
        .count();
    assert_eq!(focused, 1, "the name field takes focus");
    // Typing without clicking goes to the name.
    h.event(eframe::egui::Event::Text("Walnut".to_string()));
    h.run();
    assert_eq!(h.state().panel.form.as_ref().unwrap().name, "Walnut");
    h.get_by_label("OK").click();
    h.run();
    assert_eq!(
        h.state().actions,
        vec![MaterialAction::Create(Material::new(
            "Walnut",
            [0.8, 0.8, 0.8, 1.0]
        ))]
    );
}

#[test]
fn form_closes_when_its_material_is_gone() {
    let (model, _, _) = two_materials();
    let mut h = harness(model, None);
    let mut form = MaterialForm::edit(MaterialId(999), &Material::new("Gone", [1.0; 4]));
    form.name = "Gone".to_string();
    h.state_mut().panel.form = Some(form);
    h.run();
    assert_eq!(h.state().panel.form, None);
    assert!(h.state().actions.is_empty());
}

#[test]
fn list_and_buttons_work_while_the_form_is_open() {
    let (model, brick, _) = two_materials();
    let mut h = harness(model, None);
    h.get_by_label("New...").click();
    h.run();
    h.get_by_label("Brick").click();
    h.run();
    h.get_by_label("Purge Unused").click();
    h.run();
    h.get_by_label("Purge").click();
    h.run();
    assert_eq!(
        h.state().actions,
        vec![MaterialAction::Choose(Some(brick)), MaterialAction::Purge]
    );
    assert!(h.state().panel.form.is_some());
}
