// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-entity-info: describing the selection, the Entity Info panel (driven with
//! egui_kittest) and renaming definitions and instances in the editor.
//! See docs/specs/m6-entity-info.md.

use eframe::egui::Key;
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use glam::DVec3;
use modeler::editor::Editor;
use modeler::entity_info::*;
use modeler_kernel::component::make_component;
use modeler_kernel::model::*;
use modeler_kernel::select::Entity;
use modeler_kernel::topology::{EdgeId, FaceId};
use modeler_kernel::units::Unit;
use proptest::prelude::*;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// Draw the closed polygon into `body`.
fn polygon(model: &mut Model, body: BodyId, pts: &[DVec3]) {
    let mut log = ModelEdit::new();
    model
        .edit_geometry(body, &mut log, |ctx, edit| {
            for i in 0..pts.len() {
                ctx.add_edge(pts[i], pts[(i + 1) % pts.len()], edit)
                    .unwrap();
            }
        })
        .unwrap();
}

fn rect(x0: f64, y0: f64, w: f64, h: f64, z: f64) -> Vec<DVec3> {
    vec![
        v(x0, y0, z),
        v(x0 + w, y0, z),
        v(x0 + w, y0 + h, z),
        v(x0, y0 + h, z),
    ]
}

fn edges(model: &Model, body: BodyId) -> Vec<EdgeId> {
    model
        .body(body)
        .unwrap()
        .geometry()
        .edges()
        .map(|(id, _)| id)
        .collect()
}

fn faces(model: &Model, body: BodyId) -> Vec<FaceId> {
    model
        .body(body)
        .unwrap()
        .geometry()
        .faces()
        .map(|(id, _)| id)
        .collect()
}

/// Every edge and face of the body.
fn everything(model: &Model, body: BodyId) -> Vec<Entity> {
    let mut out: Vec<Entity> = edges(model, body).into_iter().map(Entity::Edge).collect();
    out.extend(faces(model, body).into_iter().map(Entity::Face));
    out.sort();
    out
}

/// A model whose root holds one instance of a 2 x 1 x 0 rectangle at x 1..3, y 0..1,
/// made into a component (or group) named `name`.
fn one_instance(name: &str, group: bool) -> (Model, InstanceId, DefinitionId) {
    let mut model = Model::new();
    polygon(&mut model, BodyId::Root, &rect(1.0, 0.0, 2.0, 1.0, 0.0));
    let selection = everything(&model, BodyId::Root);
    let mut undo = UndoStack::new();
    let instance = make_component(
        &mut model,
        BodyId::Root,
        &selection,
        DefinitionInfo {
            name: name.to_string(),
            group,
        },
        &mut undo,
        "Make Component",
    )
    .unwrap();
    let definition = model.root().instance(instance).unwrap().definition;
    (model, instance, definition)
}

fn paint_front(model: &mut Model, face: FaceId, material: Option<MaterialId>) {
    let mut log = ModelEdit::new();
    let mut materials = model.root().face_materials(face);
    materials.front = material;
    model
        .set_face_materials(BodyId::Root, face, materials, &mut log)
        .unwrap();
}

fn add_material(model: &mut Model, name: &str) -> MaterialId {
    let mut log = ModelEdit::new();
    model.add_material(Material::new(name, [0.5, 0.5, 0.5, 1.0]), &mut log)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

// ---------------------------------------------------------------------------------------
// describe

#[test]
fn nothing_selected() {
    let mut model = Model::new();
    polygon(&mut model, BodyId::Root, &rect(0.0, 0.0, 1.0, 1.0, 0.0));
    let info = describe(&model, BodyId::Root, &[]);
    assert_eq!(
        info,
        EntityInfo {
            kind: InfoKind::Nothing,
            edges: 0,
            faces: 0,
            instances: 0,
            length: None,
            area: None,
            front: None,
            back: None,
            size: None,
            instance: None,
        }
    );
    assert_eq!(title(&info), "No Selection");
}

#[test]
fn edges_sum_their_lengths() {
    let mut model = Model::new();
    polygon(&mut model, BodyId::Root, &rect(0.0, 0.0, 3.0, 4.0, 0.0));
    let all = edges(&model, BodyId::Root);
    let one = describe(&model, BodyId::Root, &[Entity::Edge(all[0])]);
    assert_eq!(one.kind, InfoKind::Edges);
    assert_eq!((one.edges, one.faces, one.instances), (1, 0, 0));
    let len = one.length.unwrap();
    assert!(close(len, 3.0) || close(len, 4.0), "{len}");
    assert_eq!(one.area, None);
    assert_eq!(one.front, None);
    assert_eq!(one.instance, None);
    assert_eq!(title(&one), "Edge");

    let sel: Vec<Entity> = all.iter().map(|e| Entity::Edge(*e)).collect();
    let info = describe(&model, BodyId::Root, &sel);
    assert_eq!(info.kind, InfoKind::Edges);
    assert!(close(info.length.unwrap(), 14.0));
    assert_eq!(title(&info), "4 Edges");
    let size = info.size.unwrap();
    assert!(
        close(size.x, 3.0) && close(size.y, 4.0) && close(size.z, 0.0),
        "{size}"
    );
}

#[test]
fn faces_sum_their_areas_without_holes() {
    let mut model = Model::new();
    polygon(&mut model, BodyId::Root, &rect(0.0, 0.0, 2.0, 3.0, 0.0));
    polygon(&mut model, BodyId::Root, &rect(0.5, 0.5, 1.0, 1.0, 0.0));
    let fs = faces(&model, BodyId::Root);
    assert_eq!(fs.len(), 2);
    let sel: Vec<Entity> = fs.iter().map(|f| Entity::Face(*f)).collect();
    let info = describe(&model, BodyId::Root, &sel);
    assert_eq!(info.kind, InfoKind::Faces);
    assert_eq!((info.edges, info.faces, info.instances), (0, 2, 0));
    assert!(close(info.area.unwrap(), 6.0));
    assert_eq!(info.length, None);
    assert_eq!(title(&info), "2 Faces");
    let areas: Vec<f64> = fs
        .iter()
        .map(|f| {
            describe(&model, BodyId::Root, &[Entity::Face(*f)])
                .area
                .unwrap()
        })
        .collect();
    assert!(
        (close(areas[0], 5.0) && close(areas[1], 1.0))
            || (close(areas[0], 1.0) && close(areas[1], 5.0)),
        "{areas:?}"
    );
    assert_eq!(title(&describe(&model, BodyId::Root, &sel[..1])), "Face");
}

#[test]
fn face_materials_are_shared_or_vary() {
    let mut model = Model::new();
    polygon(&mut model, BodyId::Root, &rect(0.0, 0.0, 1.0, 1.0, 0.0));
    polygon(&mut model, BodyId::Root, &rect(1.0, 0.0, 1.0, 1.0, 0.0));
    let fs = faces(&model, BodyId::Root);
    let sel: Vec<Entity> = fs.iter().map(|f| Entity::Face(*f)).collect();
    let info = describe(&model, BodyId::Root, &sel);
    assert_eq!(info.front, Some(Shared::Same(None)));
    assert_eq!(info.back, Some(Shared::Same(None)));

    let brick = add_material(&mut model, "Brick");
    paint_front(&mut model, fs[0], Some(brick));
    let info = describe(&model, BodyId::Root, &sel);
    assert_eq!(info.front, Some(Shared::Varies));
    assert_eq!(info.back, Some(Shared::Same(None)));
    let info = describe(&model, BodyId::Root, &sel[..1]);
    assert_eq!(info.front, Some(Shared::Same(Some(brick))));

    paint_front(&mut model, fs[1], Some(brick));
    let info = describe(&model, BodyId::Root, &sel);
    assert_eq!(info.front, Some(Shared::Same(Some(brick))));
}

#[test]
fn one_component_shows_its_names_and_copies() {
    let (mut model, instance, definition) = one_instance("Table", false);
    let info = describe(&model, BodyId::Root, &[Entity::Instance(instance)]);
    assert_eq!(info.kind, InfoKind::Components);
    assert_eq!((info.edges, info.faces, info.instances), (0, 0, 1));
    assert_eq!(info.length, None);
    assert_eq!(info.area, None);
    assert_eq!(info.front, None);
    let size = info.size.unwrap();
    assert!(
        close(size.x, 2.0) && close(size.y, 1.0) && close(size.z, 0.0),
        "{size}"
    );
    assert_eq!(
        info.instance,
        Some(InstanceInfo {
            id: instance,
            definition,
            group: false,
            definition_name: "Table".to_string(),
            instance_name: String::new(),
            material: None,
            copies: 1,
        })
    );
    assert_eq!(title(&info), "Component");

    // A second copy and an instance name and material on the first.
    let mut log = ModelEdit::new();
    let mut copy = model.root().instance(instance).unwrap().clone();
    copy.transform = glam::DMat4::from_translation(v(0.0, 5.0, 0.0)) * copy.transform;
    let second = model.add_instance(BodyId::Root, copy, &mut log).unwrap();
    let oak = add_material(&mut model, "Oak");
    let mut first = model.root().instance(instance).unwrap().clone();
    first.name = "Kitchen".to_string();
    first.material = Some(oak);
    model
        .set_instance(BodyId::Root, instance, first, &mut log)
        .unwrap();
    let info = describe(&model, BodyId::Root, &[Entity::Instance(instance)]);
    let one = info.instance.unwrap();
    assert_eq!(one.instance_name, "Kitchen");
    assert_eq!(one.material, Some(oak));
    assert_eq!(one.copies, 2);

    let both = describe(
        &model,
        BodyId::Root,
        &[Entity::Instance(instance), Entity::Instance(second)],
    );
    assert_eq!(both.kind, InfoKind::Components);
    assert_eq!(both.instance, None);
    assert_eq!(title(&both), "2 Components");
    let size = both.size.unwrap();
    assert!(close(size.x, 2.0) && close(size.y, 6.0), "{size}");
}

#[test]
fn one_group_is_a_group() {
    let (model, instance, _) = one_instance("Shed", true);
    let info = describe(&model, BodyId::Root, &[Entity::Instance(instance)]);
    assert_eq!(info.kind, InfoKind::Groups);
    assert!(info.instance.as_ref().unwrap().group);
    assert_eq!(info.instance.unwrap().definition_name, "Shed");
    assert_eq!(
        title(&describe(
            &model,
            BodyId::Root,
            &[Entity::Instance(instance)]
        )),
        "Group"
    );
}

#[test]
fn mixed_selection_counts_entities() {
    let (mut model, instance, _) = one_instance("Table", false);
    polygon(&mut model, BodyId::Root, &rect(5.0, 0.0, 1.0, 1.0, 0.0));
    let mut sel = everything(&model, BodyId::Root);
    sel.push(Entity::Instance(instance));
    sel.sort();
    let info = describe(&model, BodyId::Root, &sel);
    assert_eq!(info.kind, InfoKind::Mixed);
    assert_eq!((info.edges, info.faces, info.instances), (4, 1, 1));
    assert_eq!(info.length, None);
    assert_eq!(info.area, None);
    assert_eq!(info.front, None);
    assert_eq!(info.instance, None);
    assert_eq!(title(&info), "6 Entities");
    let size = info.size.unwrap();
    assert!(close(size.x, 5.0) && close(size.y, 1.0), "{size}");

    // A group and a component together are mixed too.
    let (mut model, table, _) = one_instance("Table", false);
    polygon(&mut model, BodyId::Root, &rect(5.0, 0.0, 1.0, 1.0, 0.0));
    let sel = everything(&model, BodyId::Root);
    let mut undo = UndoStack::new();
    let shed = make_component(
        &mut model,
        BodyId::Root,
        &sel,
        DefinitionInfo {
            name: "Shed".to_string(),
            group: true,
        },
        &mut undo,
        "Make Group",
    )
    .unwrap();
    let info = describe(
        &model,
        BodyId::Root,
        &[Entity::Instance(table), Entity::Instance(shed)],
    );
    assert_eq!(info.kind, InfoKind::Mixed);
    assert_eq!(title(&info), "2 Entities");
}

#[test]
fn entities_the_body_lacks_are_left_out() {
    let (model, instance, definition) = one_instance("Table", false);
    let info = describe(
        &model,
        BodyId::Root,
        &[
            Entity::Edge(EdgeId(999)),
            Entity::Face(FaceId(999)),
            Entity::Instance(instance),
        ],
    );
    assert_eq!(info.kind, InfoKind::Components);
    assert_eq!((info.edges, info.faces, info.instances), (0, 0, 1));
    assert!(info.instance.is_some());
    let gone = describe(&model, BodyId::Root, &[Entity::Edge(EdgeId(999))]);
    assert_eq!(gone.kind, InfoKind::Nothing);
    assert_eq!(gone.size, None);
    // A missing body describes nothing.
    let none = describe(
        &model,
        BodyId::Definition(DefinitionId(definition.0 + 100)),
        &[Entity::Instance(instance)],
    );
    assert_eq!(none.kind, InfoKind::Nothing);
}

#[test]
fn inside_a_definition_uses_its_body() {
    let (model, _, definition) = one_instance("Table", false);
    let body = BodyId::Definition(definition);
    let sel: Vec<Entity> = edges(&model, body).into_iter().map(Entity::Edge).collect();
    assert_eq!(sel.len(), 4);
    let info = describe(&model, body, &sel);
    assert_eq!(info.kind, InfoKind::Edges);
    assert!(close(info.length.unwrap(), 6.0));
}

// ---------------------------------------------------------------------------------------
// text

#[test]
fn titles() {
    let mut info = describe(&Model::new(), BodyId::Root, &[]);
    for (kind, edges, faces, instances, want) in [
        (InfoKind::Edges, 1, 0, 0, "Edge"),
        (InfoKind::Edges, 12, 0, 0, "12 Edges"),
        (InfoKind::Faces, 0, 1, 0, "Face"),
        (InfoKind::Faces, 0, 3, 0, "3 Faces"),
        (InfoKind::Groups, 0, 0, 1, "Group"),
        (InfoKind::Groups, 0, 0, 2, "2 Groups"),
        (InfoKind::Components, 0, 0, 1, "Component"),
        (InfoKind::Components, 0, 0, 7, "7 Components"),
        (InfoKind::Mixed, 2, 1, 1, "4 Entities"),
        (InfoKind::Nothing, 0, 0, 0, "No Selection"),
    ] {
        info.kind = kind;
        info.edges = edges;
        info.faces = faces;
        info.instances = instances;
        assert_eq!(title(&info), want);
    }
}

#[test]
fn areas_in_each_unit() {
    assert_eq!(format_area(1.5, Unit::Meter), "1.5m²");
    assert_eq!(format_area(2.0, Unit::Meter), "2m²");
    assert_eq!(format_area(0.123456, Unit::Meter), "0.12m²");
    assert_eq!(format_area(0.0, Unit::Meter), "0m²");
    assert_eq!(format_area(1.0, Unit::Centimeter), "10000cm²");
    assert_eq!(format_area(0.0001, Unit::Millimeter), "100mm²");
    assert_eq!(format_area(0.0254 * 0.0254 * 144.0, Unit::Inch), "144in²");
    assert_eq!(format_area(0.3048 * 0.3048 * 2.5, Unit::Foot), "2.5ft²");
    assert_eq!(format_area(f64::NAN, Unit::Meter), "0m²");
    assert_eq!(format_area(f64::INFINITY, Unit::Foot), "0ft²");
}

#[test]
fn material_labels() {
    let mut model = Model::new();
    let brick = add_material(&mut model, "Brick");
    assert_eq!(material_label(&model, Shared::Same(None)), "Default");
    assert_eq!(material_label(&model, Shared::Varies), "Varies");
    assert_eq!(material_label(&model, Shared::Same(Some(brick))), "Brick");
    assert_eq!(
        material_label(&model, Shared::Same(Some(MaterialId(brick.0 + 50)))),
        "?"
    );
}

// ---------------------------------------------------------------------------------------
// Editor renames

#[test]
fn rename_definition_is_one_undo_step() {
    let (mut model, instance, definition) = one_instance("Table", false);
    let mut editor = Editor::new();
    let before = model.clone();
    assert!(editor.rename_definition(&mut model, definition, "  Desk "));
    assert_eq!(
        model.definition(definition).unwrap(),
        &DefinitionInfo {
            name: "Desk".to_string(),
            group: false,
        }
    );
    assert_eq!(editor.undo_stack().undo_count(), 1);
    assert_eq!(editor.undo_stack().undo_label(), Some("Rename"));
    assert!(model.root().instance(instance).is_some());
    let mut stack = editor.undo_stack().clone();
    stack.undo(&mut model).unwrap();
    assert_eq!(model.definition(definition), before.definition(definition));
}

#[test]
fn rename_definition_refuses_blank_same_and_unknown() {
    let (mut model, _, definition) = one_instance("Table", true);
    let mut editor = Editor::new();
    assert!(!editor.rename_definition(&mut model, definition, "   "));
    assert!(!editor.rename_definition(&mut model, definition, "Table"));
    assert!(!editor.rename_definition(&mut model, definition, " Table  "));
    assert!(!editor.rename_definition(&mut model, DefinitionId(definition.0 + 77), "X"));
    assert_eq!(editor.undo_stack().undo_count(), 0);
    assert_eq!(model.definition(definition).unwrap().name, "Table");
    // The group flag stays.
    assert!(editor.rename_definition(&mut model, definition, "Shed"));
    assert!(model.definition(definition).unwrap().group);
}

#[test]
fn rename_definition_to_a_taken_name_numbers_it() {
    let (mut model, _, table) = one_instance("Table", false);
    polygon(&mut model, BodyId::Root, &rect(5.0, 0.0, 1.0, 1.0, 0.0));
    let sel = everything(&model, BodyId::Root);
    let mut undo = UndoStack::new();
    let chair_instance = make_component(
        &mut model,
        BodyId::Root,
        &sel,
        DefinitionInfo {
            name: "Chair".to_string(),
            group: false,
        },
        &mut undo,
        "Make Component",
    )
    .unwrap();
    let chair = model.root().instance(chair_instance).unwrap().definition;
    let mut editor = Editor::new();
    assert!(editor.rename_definition(&mut model, chair, "Table"));
    assert_eq!(model.definition(chair).unwrap().name, "Table 2");
    // Its own name doesn't count as taken: numbering lands on its current name, so
    // nothing changes.
    assert!(!editor.rename_definition(&mut model, chair, "Table"));
    assert_eq!(editor.undo_stack().undo_count(), 1);
    // The number goes after the whole name asked for.
    assert!(editor.rename_definition(&mut model, table, "Table 2 "));
    assert_eq!(model.definition(table).unwrap().name, "Table 2 2");
    // Free names are used as they are.
    assert!(editor.rename_definition(&mut model, chair, "Table"));
    assert_eq!(model.definition(chair).unwrap().name, "Table");
    assert!(editor.rename_definition(&mut model, chair, "Table 2"));
    assert_eq!(model.definition(chair).unwrap().name, "Table 2");
    assert_eq!(editor.undo_stack().undo_count(), 4);
}

#[test]
fn rename_instance_in_the_open_body() {
    let (mut model, instance, _) = one_instance("Table", false);
    let mut editor = Editor::new();
    assert!(editor.rename_instance(&mut model, instance, " Kitchen table "));
    assert_eq!(
        model.root().instance(instance).unwrap().name,
        "Kitchen table"
    );
    assert_eq!(editor.undo_stack().undo_count(), 1);
    assert_eq!(editor.undo_stack().undo_label(), Some("Rename"));
    assert!(!editor.rename_instance(&mut model, instance, "Kitchen table"));
    assert_eq!(editor.undo_stack().undo_count(), 1);
    // Blank clears the name.
    assert!(editor.rename_instance(&mut model, instance, "  "));
    assert_eq!(model.root().instance(instance).unwrap().name, "");
    assert!(!editor.rename_instance(&mut model, InstanceId(instance.0 + 40), "X"));
    assert_eq!(editor.undo_stack().undo_count(), 2);
    let mut stack = editor.undo_stack().clone();
    stack.undo(&mut model).unwrap();
    assert_eq!(
        model.root().instance(instance).unwrap().name,
        "Kitchen table"
    );
}

// ---------------------------------------------------------------------------------------
// The panel

struct State {
    panel: EntityInfoPanel,
    model: Model,
    selection: Vec<Entity>,
    actions: Vec<InfoAction>,
}

/// The panel for `selection`, the model's lengths in `unit` to 2 places.
fn harness(mut model: Model, selection: Vec<Entity>, unit: Unit) -> Harness<'static, State> {
    model
        .set_length_format(
            modeler_kernel::units::LengthFormat::Decimal { unit, decimals: 2 },
            &mut modeler_kernel::model::ModelEdit::new(),
        )
        .unwrap();
    let mut h = Harness::new_ui_state(
        |ui, state: &mut State| {
            if let Some(action) = entity_info_panel(
                ui,
                &mut state.panel,
                &state.model,
                BodyId::Root,
                &state.selection,
            ) {
                state.actions.push(action);
            }
        },
        State {
            panel: EntityInfoPanel::default(),
            model,
            selection,
            actions: Vec::new(),
        },
    );
    h.run();
    h
}

#[test]
fn panel_with_nothing_selected() {
    let h = harness(Model::new(), Vec::new(), Unit::Meter);
    h.get_by_label("No Selection");
    assert!(h.query_by_label("Definition").is_none());
    assert!(h.query_by_role(Role::TextInput).is_none());
}

#[test]
fn panel_shows_edge_length_and_size() {
    let mut model = Model::new();
    polygon(&mut model, BodyId::Root, &rect(0.0, 0.0, 3.0, 4.0, 0.0));
    let sel: Vec<Entity> = edges(&model, BodyId::Root)
        .into_iter()
        .map(Entity::Edge)
        .collect();
    let h = harness(model, sel, Unit::Meter);
    h.get_by_label("4 Edges");
    h.get_by_label("Length");
    h.get_by_label("14m");
    h.get_by_label("Width");
    h.get_by_label("3m");
    h.get_by_label("Depth");
    h.get_by_label("4m");
    h.get_by_label("Height");
    h.get_by_label("0m");
    assert!(h.query_by_label("Area").is_none());
}

#[test]
fn panel_shows_face_area_and_materials() {
    let mut model = Model::new();
    polygon(&mut model, BodyId::Root, &rect(0.0, 0.0, 2.0, 3.0, 0.0));
    let brick = add_material(&mut model, "Brick");
    let f = faces(&model, BodyId::Root)[0];
    paint_front(&mut model, f, Some(brick));
    let h = harness(model, vec![Entity::Face(f)], Unit::Centimeter);
    h.get_by_label("Face");
    h.get_by_label("Area");
    h.get_by_label("60000cm²");
    h.get_by_label("Front");
    h.get_by_label("Brick");
    h.get_by_label("Back");
    h.get_by_label("Default");
    assert!(h.query_by_label("Length").is_none());
}

#[test]
fn panel_shows_a_component_with_name_fields() {
    let (model, instance, _) = one_instance("Table", false);
    let h = harness(model, vec![Entity::Instance(instance)], Unit::Meter);
    h.get_by_label("Component");
    let definition = h.get_by_label("Definition");
    assert_eq!(definition.accesskit_node().role(), Role::TextInput);
    assert_eq!(
        definition.accesskit_node().value().as_deref(),
        Some("Table")
    );
    let name = h.get_by_label("Instance");
    assert_eq!(name.accesskit_node().role(), Role::TextInput);
    assert_eq!(name.accesskit_node().value().as_deref(), Some(""));
    h.get_by_label("Material");
    h.get_by_label("Default");
    h.get_by_label("In model");
    h.get_by_label("1");
    assert_eq!(h.state().panel.definition_name, "Table");
}

#[test]
fn panel_shows_a_group_with_one_name_field() {
    let (model, instance, _) = one_instance("Shed", true);
    let h = harness(model, vec![Entity::Instance(instance)], Unit::Meter);
    h.get_by_label("Group");
    let name = h.get_by_label("Name");
    assert_eq!(name.accesskit_node().role(), Role::TextInput);
    assert_eq!(name.accesskit_node().value().as_deref(), Some("Shed"));
    assert!(h.query_by_label("Definition").is_none());
    assert!(h.query_by_label("Instance").is_none());
}

#[test]
fn enter_in_the_definition_field_renames() {
    let (model, instance, definition) = one_instance("Table", false);
    let mut h = harness(model, vec![Entity::Instance(instance)], Unit::Meter);
    h.state_mut().panel.definition_name = "  Desk ".to_string();
    h.run();
    h.get_by_label("Definition").click();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(
        h.state().actions,
        vec![InfoAction::RenameDefinition(definition, "Desk".to_string())]
    );
}

#[test]
fn typing_an_instance_name_and_enter_renames() {
    let (model, instance, _) = one_instance("Table", false);
    let mut h = harness(model, vec![Entity::Instance(instance)], Unit::Meter);
    h.get_by_label("Instance").click();
    h.run();
    h.get_by_label("Instance").type_text("Kitchen");
    h.run();
    assert!(
        h.state().actions.is_empty(),
        "nothing until the field is left"
    );
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(
        h.state().actions,
        vec![InfoAction::RenameInstance(instance, "Kitchen".to_string())]
    );
}

#[test]
fn group_name_field_renames_the_definition() {
    let (model, instance, definition) = one_instance("Shed", true);
    let mut h = harness(model, vec![Entity::Instance(instance)], Unit::Meter);
    h.state_mut().panel.definition_name = "Garage".to_string();
    h.run();
    h.get_by_label("Name").click();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert_eq!(
        h.state().actions,
        vec![InfoAction::RenameDefinition(
            definition,
            "Garage".to_string()
        )]
    );
}

#[test]
fn unchanged_or_blank_definition_name_does_nothing() {
    let (model, instance, _) = one_instance("Table", false);
    let mut h = harness(model, vec![Entity::Instance(instance)], Unit::Meter);
    h.get_by_label("Definition").click();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().actions.is_empty());

    h.state_mut().panel.definition_name = "   ".to_string();
    h.run();
    h.get_by_label("Definition").click();
    h.run();
    h.key_press(Key::Enter);
    h.run();
    assert!(h.state().actions.is_empty());
    assert_eq!(h.state().panel.definition_name, "Table", "blank goes back");
}

#[test]
fn escape_puts_the_name_back() {
    let (model, instance, _) = one_instance("Table", false);
    let mut h = harness(model, vec![Entity::Instance(instance)], Unit::Meter);
    h.get_by_label("Definition").click();
    h.run();
    h.state_mut().panel.definition_name = "Desk".to_string();
    h.run();
    h.key_press(Key::Escape);
    h.run();
    h.run();
    assert!(h.state().actions.is_empty());
    assert_eq!(h.state().panel.definition_name, "Table");
}

#[test]
fn fields_follow_the_selection_and_the_model() {
    let (mut model, table, definition) = one_instance("Table", false);
    polygon(&mut model, BodyId::Root, &rect(5.0, 0.0, 1.0, 1.0, 0.0));
    let sel = everything(&model, BodyId::Root);
    let mut undo = UndoStack::new();
    let chair = make_component(
        &mut model,
        BodyId::Root,
        &sel,
        DefinitionInfo {
            name: "Chair".to_string(),
            group: false,
        },
        &mut undo,
        "Make Component",
    )
    .unwrap();
    let mut h = harness(model, vec![Entity::Instance(table)], Unit::Meter);
    assert_eq!(h.state().panel.definition_name, "Table");
    h.state_mut().selection = vec![Entity::Instance(chair)];
    h.run();
    assert_eq!(h.state().panel.definition_name, "Chair");
    assert_eq!(
        h.get_by_label("Definition")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Chair")
    );
    // Renamed elsewhere (undo, another panel): the field follows.
    h.state_mut().selection = vec![Entity::Instance(table)];
    h.run();
    let mut log = ModelEdit::new();
    h.state_mut()
        .model
        .set_definition(
            definition,
            DefinitionInfo {
                name: "Bench".to_string(),
                group: false,
            },
            &mut log,
        )
        .unwrap();
    h.run();
    assert_eq!(h.state().panel.definition_name, "Bench");
    // Nothing selected: no fields.
    h.state_mut().selection.clear();
    h.run();
    assert!(h.query_by_role(Role::TextInput).is_none());
    assert_eq!(h.state().panel.shown, None);
}

// ---------------------------------------------------------------------------------------
// Properties

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn counts_and_kind_agree_with_the_selection(
        picks in prop::collection::vec((0u8..3, 0usize..12), 0..10),
    ) {
        let (mut model, instance, _) = one_instance("Table", false);
        polygon(&mut model, BodyId::Root, &rect(5.0, 0.0, 1.0, 1.0, 0.0));
        polygon(&mut model, BodyId::Root, &rect(6.0, 0.0, 1.0, 2.0, 0.0));
        let es = edges(&model, BodyId::Root);
        let fs = faces(&model, BodyId::Root);
        let mut sel: Vec<Entity> = picks
            .iter()
            .map(|(kind, i)| match kind {
                0 => Entity::Edge(es.get(*i).copied().unwrap_or(EdgeId(900 + *i as u64))),
                1 => Entity::Face(fs.get(*i).copied().unwrap_or(FaceId(900 + *i as u64))),
                _ => Entity::Instance(if *i == 0 { instance } else { InstanceId(900 + *i as u64) }),
            })
            .collect();
        sel.sort();
        sel.dedup();
        let info = describe(&model, BodyId::Root, &sel);
        let have_e = sel.iter().filter(|e| matches!(e, Entity::Edge(id) if es.contains(id))).count();
        let have_f = sel.iter().filter(|e| matches!(e, Entity::Face(id) if fs.contains(id))).count();
        let have_i = sel.iter().filter(|e| **e == Entity::Instance(instance)).count();
        prop_assert_eq!((info.edges, info.faces, info.instances), (have_e, have_f, have_i));
        let kinds = [have_e > 0, have_f > 0, have_i > 0].iter().filter(|b| **b).count();
        let want = match (kinds, have_e > 0, have_f > 0) {
            (0, _, _) => InfoKind::Nothing,
            (1, true, _) => InfoKind::Edges,
            (1, _, true) => InfoKind::Faces,
            (1, _, _) => InfoKind::Components,
            _ => InfoKind::Mixed,
        };
        prop_assert_eq!(info.kind, want);
        prop_assert_eq!(info.length.is_some(), want == InfoKind::Edges);
        prop_assert_eq!(info.area.is_some(), want == InfoKind::Faces);
        prop_assert_eq!(info.front.is_some(), want == InfoKind::Faces);
        prop_assert_eq!(info.size.is_some(), want != InfoKind::Nothing);
        prop_assert_eq!(info.instance.is_some(), have_i == 1 && kinds == 1);
        if let Some(size) = info.size {
            prop_assert!(size.x >= 0.0 && size.y >= 0.0 && size.z >= 0.0);
        }
        if let Some(l) = info.length {
            prop_assert!(l > 0.0);
        }
        if let Some(a) = info.area {
            prop_assert!(a > 0.0);
        }
        prop_assert!(!title(&info).is_empty());
    }
}
