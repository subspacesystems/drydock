// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-units-ui: the Model Info Units section (driven with egui_kittest), the editor's
//! "Set Units" step, and lengths and areas in the model's format in the measurement box,
//! Entity Info, Model Info, the Texture Position panel and the material form.
//! See docs/specs/m6-units-ui.md.

use drydock::chrome::model_info_lines;
use drydock::document::Stats;
use drydock::editor::{Editor, TextureOp, Viewport};
use drydock::entity_info::{EntityInfoPanel, entity_info_panel};
use drydock::input::{EditKey, Modifiers, Tool};
use drydock::materials_panel::{MaterialAction, MaterialsPanel, materials_panel};
use drydock::texture_panel::{TextureForm, texture_panel};
use drydock::units_ui::*;
use drydock_kernel::model::{BodyId, Material, MaterialId, Model, ModelEdit};
use drydock_kernel::select::Entity;
use drydock_kernel::units::{LengthFormat, Unit, format_with};
use drydock_view::{Bounds, Camera, Projection, StandardView};
use eframe::egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use glam::{DVec2, DVec3};

fn dec(unit: Unit, decimals: u8) -> LengthFormat {
    LengthFormat::Decimal { unit, decimals }
}

fn arch(denominator: u8) -> LengthFormat {
    LengthFormat::Architectural { denominator }
}

fn model_in(format: LengthFormat) -> Model {
    let mut model = Model::new();
    model
        .set_length_format(format, &mut ModelEdit::new())
        .unwrap();
    model
}

const ALL_UNITS: [Unit; 5] = [
    Unit::Millimeter,
    Unit::Centimeter,
    Unit::Meter,
    Unit::Inch,
    Unit::Foot,
];

// ---------------------------------------------------------------------------------------
// Helpers

#[test]
fn unit_rows_are_in_order() {
    assert_eq!(
        UNIT_NAMES,
        [
            "Millimeters",
            "Centimeters",
            "Meters",
            "Inches",
            "Feet",
            "Architectural"
        ]
    );
    for (row, unit) in ALL_UNITS.into_iter().enumerate() {
        assert_eq!(unit_index(dec(unit, 0)), row);
        assert_eq!(unit_index(dec(unit, 5)), row);
    }
    assert_eq!(unit_index(arch(1)), 5);
    assert_eq!(unit_index(arch(64)), 5);
}

#[test]
fn picking_a_decimal_unit_keeps_the_places() {
    assert_eq!(pick_unit(dec(Unit::Inch, 3), 2), dec(Unit::Meter, 3));
    assert_eq!(pick_unit(dec(Unit::Meter, 0), 0), dec(Unit::Millimeter, 0));
    assert_eq!(pick_unit(dec(Unit::Millimeter, 6), 4), dec(Unit::Foot, 6));
}

#[test]
fn picking_architectural_starts_at_a_sixteenth() {
    assert_eq!(pick_unit(dec(Unit::Inch, 2), 5), arch(16));
    assert_eq!(pick_unit(dec(Unit::Meter, 0), 5), arch(16));
}

#[test]
fn leaving_architectural_starts_at_two_places() {
    assert_eq!(pick_unit(arch(4), 3), dec(Unit::Inch, 2));
    assert_eq!(pick_unit(arch(64), 1), dec(Unit::Centimeter, 2));
}

#[test]
fn picking_the_shown_row_or_past_the_list_changes_nothing() {
    for format in [dec(Unit::Foot, 4), arch(32), dec(Unit::Millimeter, 0)] {
        assert_eq!(pick_unit(format, unit_index(format)), format);
        assert_eq!(pick_unit(format, 6), format);
        assert_eq!(pick_unit(format, 99), format);
    }
}

#[test]
fn precision_rows_for_a_decimal_unit() {
    assert_eq!(
        precisions(dec(Unit::Centimeter, 3)),
        (0..=6)
            .map(|d| dec(Unit::Centimeter, d))
            .collect::<Vec<_>>()
    );
    let labels: Vec<String> = precisions(dec(Unit::Inch, 0))
        .into_iter()
        .map(precision_label)
        .collect();
    assert_eq!(
        labels,
        ["0", "0.0", "0.00", "0.000", "0.0000", "0.00000", "0.000000"]
    );
}

#[test]
fn precision_rows_for_architectural() {
    assert_eq!(
        precisions(arch(8)),
        [1, 2, 4, 8, 16, 32, 64].map(arch).to_vec()
    );
    let labels: Vec<String> = precisions(arch(1))
        .into_iter()
        .map(precision_label)
        .collect();
    assert_eq!(
        labels,
        [
            "1\"", "1/2\"", "1/4\"", "1/8\"", "1/16\"", "1/32\"", "1/64\""
        ]
    );
}

#[test]
fn unit_abbreviations() {
    let names: Vec<&str> = ALL_UNITS.into_iter().map(unit_abbrev).collect();
    assert_eq!(names, ["mm", "cm", "m", "in", "ft"]);
}

// ---------------------------------------------------------------------------------------
// The Units section

struct Section {
    format: LengthFormat,
    picked: Vec<LengthFormat>,
}

/// The section as Model Info uses it: whatever is picked becomes the shown format.
fn section(format: LengthFormat) -> Harness<'static, Section> {
    let mut h = Harness::builder().with_size([420.0, 400.0]).build_ui_state(
        |ui, state: &mut Section| {
            if let Some(format) = units_section(ui, state.format) {
                state.picked.push(format);
                state.format = format;
            }
        },
        Section {
            format,
            picked: Vec::new(),
        },
    );
    h.run();
    h
}

fn combo_value(h: &Harness<'_, Section>, label: &str) -> String {
    let node = h.get_by_label(label);
    assert_eq!(node.accesskit_node().role(), Role::ComboBox, "{label}");
    node.value().expect("a shown value")
}

/// Open the combo box labelled `label` and click its row `row`.
fn choose(h: &mut Harness<'_, Section>, label: &str, row: &str) {
    h.get_by_label(label).click();
    h.run();
    h.get_by_label(row).click();
    h.run();
    h.run();
}

#[test]
fn section_shows_the_heading_and_both_lists() {
    let h = section(LengthFormat::default());
    h.get_by_label("Units");
    assert_eq!(combo_value(&h, "Unit:"), "Inches");
    assert_eq!(combo_value(&h, "Precision:"), "0.00");
    assert!(h.state().picked.is_empty());
}

#[test]
fn section_shows_architectural() {
    let h = section(arch(32));
    assert_eq!(combo_value(&h, "Unit:"), "Architectural");
    assert_eq!(combo_value(&h, "Precision:"), "1/32\"");
}

#[test]
fn choosing_a_unit_reports_it_once() {
    let mut h = section(dec(Unit::Inch, 3));
    choose(&mut h, "Unit:", "Millimeters");
    assert_eq!(h.state().picked, vec![dec(Unit::Millimeter, 3)]);
    assert_eq!(combo_value(&h, "Unit:"), "Millimeters");
    assert_eq!(combo_value(&h, "Precision:"), "0.000");
    h.run();
    assert_eq!(h.state().picked.len(), 1, "nothing more without a pick");
}

#[test]
fn choosing_architectural_then_a_precision() {
    let mut h = section(LengthFormat::default());
    choose(&mut h, "Unit:", "Architectural");
    assert_eq!(h.state().picked, vec![arch(16)]);
    assert_eq!(combo_value(&h, "Precision:"), "1/16\"");
    choose(&mut h, "Precision:", "1/4\"");
    assert_eq!(h.state().picked, vec![arch(16), arch(4)]);
    assert_eq!(combo_value(&h, "Precision:"), "1/4\"");
}

#[test]
fn choosing_decimal_places() {
    let mut h = section(dec(Unit::Meter, 2));
    choose(&mut h, "Precision:", "0");
    assert_eq!(h.state().picked, vec![dec(Unit::Meter, 0)]);
    choose(&mut h, "Precision:", "0.000000");
    assert_eq!(
        h.state().picked,
        vec![dec(Unit::Meter, 0), dec(Unit::Meter, 6)]
    );
}

#[test]
fn choosing_the_shown_row_reports_nothing() {
    let mut h = section(dec(Unit::Foot, 1));
    h.get_by_label("Unit:").click();
    h.run();
    // The open list and the closed box both read "Feet"; click the list row.
    let row = h
        .query_all_by_label("Feet")
        .find(|node| node.accesskit_node().role() != Role::ComboBox)
        .expect("the Feet row");
    row.click();
    h.run();
    h.run();
    assert!(h.state().picked.is_empty());
}

// ---------------------------------------------------------------------------------------
// The editor

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

#[test]
fn set_length_format_is_one_undo_step() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    assert!(editor.set_length_format(&mut model, dec(Unit::Millimeter, 1)));
    assert_eq!(model.length_format(), dec(Unit::Millimeter, 1));
    assert_eq!(editor.undo_stack().undo_count(), 1);
    assert_eq!(editor.undo_stack().undo_label(), Some("Set Units"));
    assert!(editor.message().is_none());
    let mut stack = editor.undo_stack().clone();
    stack.undo(&mut model).unwrap();
    assert_eq!(model.length_format(), LengthFormat::default());
}

#[test]
fn setting_the_same_format_adds_no_step() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    assert!(editor.set_length_format(&mut model, LengthFormat::default()));
    assert_eq!(editor.undo_stack().undo_count(), 0);
}

#[test]
fn an_invalid_format_is_refused() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    assert!(!editor.set_length_format(&mut model, dec(Unit::Meter, 9)));
    assert!(!editor.set_length_format(&mut model, arch(3)));
    assert_eq!(model.length_format(), LengthFormat::default());
    assert_eq!(editor.undo_stack().undo_count(), 0);
    assert!(editor.message().is_some());
}

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

fn px(view: &Viewport, p: DVec3) -> DVec2 {
    view.project(p).expect("visible")
}

#[test]
fn the_measurement_box_follows_the_chosen_units() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let view = top_view();
    editor.set_tool(Tool::Line);
    editor.click(&mut model, &view, px(&view, DVec3::ZERO), NONE, 1);
    editor.pointer_move(
        &mut model,
        &view,
        px(&view, DVec3::new(2.0, 0.0, 0.0)),
        NONE,
    );
    let (label, inches) = editor.measurement(&model).expect("a length");
    assert_eq!(label, "Length");
    assert_eq!(inches, format_with(2.0, LengthFormat::default()));
    assert!(editor.set_length_format(&mut model, dec(Unit::Millimeter, 0)));
    assert_eq!(
        editor.measurement(&model),
        Some(("Length", "2000mm".to_string()))
    );
    assert!(editor.set_length_format(&mut model, arch(16)));
    assert_eq!(
        editor.measurement(&model),
        Some(("Length", "6' 6 3/4\"".to_string()))
    );
}

#[test]
fn a_bare_typed_number_reads_in_the_chosen_unit() {
    let mut model = Model::new();
    let mut editor = Editor::new();
    let view = top_view();
    assert!(editor.set_length_format(&mut model, dec(Unit::Centimeter, 2)));
    editor.set_tool(Tool::Line);
    editor.click(&mut model, &view, px(&view, DVec3::ZERO), NONE, 1);
    editor.pointer_move(
        &mut model,
        &view,
        px(&view, DVec3::new(1.0, 0.0, 0.0)),
        NONE,
    );
    assert!(editor.text("150"));
    assert!(editor.key(EditKey::Enter, &mut model));
    let ctx = model.root().geometry();
    let ends: Vec<DVec3> = ctx
        .edges()
        .flat_map(|(_, e)| e.ends.map(|id| ctx.vertex(id).expect("vertex").position))
        .collect();
    assert!(
        ends.iter()
            .any(|p| p.distance(DVec3::new(1.5, 0.0, 0.0)) < 1e-9),
        "{ends:?}"
    );
}

// ---------------------------------------------------------------------------------------
// Entity Info

struct Info {
    panel: EntityInfoPanel,
    model: Model,
    selection: Vec<Entity>,
}

fn info(model: Model, selection: Vec<Entity>) -> Harness<'static, Info> {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut Info| {
            entity_info_panel(
                ui,
                &mut state.panel,
                &state.model,
                BodyId::Root,
                &state.selection,
            );
        },
        Info {
            panel: EntityInfoPanel::default(),
            model,
            selection,
        },
    );
    h.run();
    h
}

/// A 3 m by 4 m rectangle face in the root.
fn rectangle(model: &mut Model) {
    let pts = [
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(3.0, 0.0, 0.0),
        DVec3::new(3.0, 4.0, 0.0),
        DVec3::new(0.0, 4.0, 0.0),
    ];
    model
        .edit_geometry(BodyId::Root, &mut ModelEdit::new(), |ctx, edit| {
            for i in 0..4 {
                ctx.add_edge(pts[i], pts[(i + 1) % 4], edit).unwrap();
            }
        })
        .unwrap();
}

fn edge_selection(model: &Model) -> Vec<Entity> {
    model
        .root()
        .geometry()
        .edges()
        .map(|(id, _)| Entity::Edge(id))
        .collect()
}

fn face_selection(model: &Model) -> Vec<Entity> {
    model
        .root()
        .geometry()
        .faces()
        .map(|(id, _)| Entity::Face(id))
        .collect()
}

#[test]
fn entity_info_lengths_in_architectural() {
    let mut model = model_in(arch(16));
    rectangle(&mut model);
    let sel = edge_selection(&model);
    let h = info(model, sel);
    h.get_by_label(&format_with(14.0, arch(16)));
    h.get_by_label("45' 11 3/16\"");
    h.get_by_label("9' 10 1/8\"");
    h.get_by_label("13' 1 1/2\"");
    h.get_by_label("0\"");
}

#[test]
fn entity_info_lengths_in_decimal_feet() {
    let mut model = model_in(dec(Unit::Foot, 1));
    rectangle(&mut model);
    let sel = edge_selection(&model);
    let h = info(model, sel);
    h.get_by_label("45.9'");
    h.get_by_label("9.8'");
    h.get_by_label("13.1'");
}

#[test]
fn entity_info_lengths_follow_the_places() {
    let mut model = model_in(dec(Unit::Millimeter, 0));
    rectangle(&mut model);
    let sel = edge_selection(&model);
    let h = info(model, sel);
    h.get_by_label("14000mm");
    h.get_by_label("3000mm");
    h.get_by_label("4000mm");
}

#[test]
fn entity_info_areas_in_each_format() {
    for (format, text) in [
        (dec(Unit::Meter, 2), "12m²"),
        (dec(Unit::Centimeter, 0), "120000cm²"),
        (dec(Unit::Inch, 1), "18600in²"),
        (dec(Unit::Foot, 3), "129.167ft²"),
        (arch(8), "129.17ft²"),
    ] {
        let mut model = model_in(format);
        rectangle(&mut model);
        let sel = face_selection(&model);
        let h = info(model, sel);
        h.get_by_label("Area");
        assert!(h.query_by_label(text).is_some(), "{format:?}: {text}");
    }
}

// ---------------------------------------------------------------------------------------
// Model Info

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
fn model_info_size_in_meters() {
    let lines = model_info_lines(&stats(), true, dec(Unit::Meter, 3));
    assert_eq!(lines[3], "Size: 4.115m x 6.706m x 2.438m");
}

#[test]
fn model_info_size_in_each_format() {
    for format in [
        LengthFormat::default(),
        dec(Unit::Millimeter, 0),
        dec(Unit::Foot, 2),
        arch(4),
    ] {
        let lines = model_info_lines(&stats(), false, format);
        assert_eq!(
            lines[3],
            format!(
                "Size: {} x {} x {}",
                format_with(4.115, format),
                format_with(6.706, format),
                format_with(2.438, format)
            ),
            "{format:?}"
        );
    }
    let lines = model_info_lines(&stats(), true, arch(4));
    assert_eq!(lines[3], "Size: 13' 6\" x 22' 0\" x 8' 0\"");
}

// ---------------------------------------------------------------------------------------
// Texture Position panel

struct Texture {
    form: TextureForm,
    unit: Unit,
    ops: Vec<TextureOp>,
}

fn texture(unit: Unit, form: TextureForm) -> Harness<'static, Texture> {
    let mut h = Harness::new_ui_state(
        |ui, state: &mut Texture| {
            if let Some(op) = texture_panel(ui, &mut state.form, true, state.unit) {
                state.ops.push(op);
            }
        },
        Texture {
            form,
            unit,
            ops: Vec::new(),
        },
    );
    h.run();
    h
}

fn number<S>(h: &Harness<'_, S>, label: &str) -> f64 {
    h.get_by_label(label)
        .accesskit_node()
        .numeric_value()
        .unwrap_or_else(|| panic!("{label} has no number"))
}

#[test]
fn texture_labels_name_the_unit() {
    for (unit, name) in [
        (Unit::Millimeter, "mm"),
        (Unit::Centimeter, "cm"),
        (Unit::Meter, "m"),
        (Unit::Inch, "in"),
        (Unit::Foot, "ft"),
    ] {
        let h = texture(unit, TextureForm::default());
        h.get_by_label(&format!("Move X ({name}):"));
        h.get_by_label(&format!("Move Y ({name}):"));
    }
    let h = texture(Unit::Inch, TextureForm::default());
    for label in ["Turn (deg):", "Scale X:", "Scale Y:"] {
        h.get_by_label(label);
    }
}

#[test]
fn texture_moves_show_in_the_unit_and_stay_in_meters() {
    let form = TextureForm {
        shift: [0.0254 * 12.0, -0.3048],
        ..TextureForm::default()
    };
    let mut h = texture(Unit::Inch, form);
    assert!((number(&h, "Move X (in):") - 12.0).abs() < 1e-9);
    assert!((number(&h, "Move Y (in):") + 12.0).abs() < 1e-9);
    h.get_by_label("Move").click();
    h.run();
    assert_eq!(
        h.state().ops,
        vec![TextureOp::Move(DVec2::new(0.0254 * 12.0, -0.3048))]
    );
}

#[test]
fn texture_moves_in_millimeters() {
    let form = TextureForm {
        shift: [0.125, 2.0],
        ..TextureForm::default()
    };
    let h = texture(Unit::Millimeter, form);
    assert!((number(&h, "Move X (mm):") - 125.0).abs() < 1e-9);
    assert!((number(&h, "Move Y (mm):") - 2000.0).abs() < 1e-9);
}

// ---------------------------------------------------------------------------------------
// The material form

struct Materials {
    panel: MaterialsPanel,
    model: Model,
    current: Option<MaterialId>,
    actions: Vec<MaterialAction>,
}

fn materials(model: Model, current: Option<MaterialId>) -> Harness<'static, Materials> {
    let mut h = Harness::builder().with_size([420.0, 700.0]).build_ui_state(
        |ui, state: &mut Materials| {
            if let Some(action) = materials_panel(ui, &mut state.panel, &state.model, state.current)
            {
                state.actions.push(action);
            }
        },
        Materials {
            panel: MaterialsPanel::default(),
            model,
            current,
            actions: Vec::new(),
        },
    );
    h.run();
    h
}

fn textured_material(model: &mut Model) -> MaterialId {
    let mut material = Material::new("Oak", [1.0, 1.0, 1.0, 1.0]);
    material.texture = Some(drydock_kernel::model::Texture::new(
        "/tex/oak.png",
        DVec2::new(0.5, 2.0),
    ));
    model.add_material(material, &mut ModelEdit::new())
}

#[test]
fn the_material_form_sizes_in_the_model_unit() {
    let mut model = model_in(dec(Unit::Centimeter, 1));
    let oak = textured_material(&mut model);
    let mut h = materials(model, Some(oak));
    h.get_by_label("Edit...").click();
    h.run();
    assert!((number(&h, "Width (cm):") - 50.0).abs() < 1e-9);
    assert!((number(&h, "Height (cm):") - 200.0).abs() < 1e-9);
    assert!(h.query_by_label("Width (m):").is_none());
    h.get_by_label("OK").click();
    h.run();
    let Some(MaterialAction::Edit(_, material)) = h.state().actions.last() else {
        panic!("an edit: {:?}", h.state().actions);
    };
    let size = material.texture.as_ref().expect("texture").size;
    assert!((size - DVec2::new(0.5, 2.0)).length() < 1e-12, "{size:?}");
}

#[test]
fn architectural_material_sizes_are_in_inches() {
    let mut h = materials(model_in(arch(8)), None);
    h.get_by_label("New...").click();
    h.run();
    h.get_by_label("Width (in):");
    h.get_by_label("Height (in):");
    assert!((number(&h, "Width (in):") - 1.0 / 0.0254).abs() < 1e-9);
}
