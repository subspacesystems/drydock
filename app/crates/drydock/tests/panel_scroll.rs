// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Materials and Components stacked in one column scroll on their own (round 4:
//! the wheel over one list scrolled both). egui gives sibling child uis the same id, so two
//! lists' scroll areas without their own id salt shared one scroll state.

use drydock::components_panel::{ComponentsPanel, components_panel};
use drydock::materials_panel::{MaterialsPanel, materials_panel};
use drydock_kernel::component::make_component;
use drydock_kernel::model::*;
use drydock_kernel::select::Entity;
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glam::DVec3;

const ROW: f32 = 200.0;

struct State {
    model: Model,
    materials: MaterialsPanel,
    components: ComponentsPanel,
}

/// 40 materials and 30 components, enough for both lists to scroll in a 200 pt row.
fn sample() -> Model {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    for i in 0..40 {
        model.add_material(
            Material::new(&format!("Mat{i:02}"), [0.5, 0.5, 0.5, 1.0]),
            &mut log,
        );
    }
    let mut undo = UndoStack::new();
    for i in 0..30 {
        let x = i as f64 * 3.0;
        let pts = [
            DVec3::new(x, 0.0, 0.0),
            DVec3::new(x + 1.0, 0.0, 0.0),
            DVec3::new(x + 1.0, 1.0, 0.0),
            DVec3::new(x, 1.0, 0.0),
        ];
        let before: Vec<Entity> = entities(&model);
        let mut log = ModelEdit::new();
        model
            .edit_geometry(BodyId::Root, &mut log, |ctx, edit| {
                for k in 0..4 {
                    ctx.add_edge(pts[k], pts[(k + 1) % 4], edit).unwrap();
                }
            })
            .unwrap();
        let selection: Vec<Entity> = entities(&model)
            .into_iter()
            .filter(|e| !before.contains(e))
            .collect();
        make_component(
            &mut model,
            BodyId::Root,
            &selection,
            DefinitionInfo {
                name: format!("Comp{i:02}"),
                group: false,
            },
            &mut undo,
            "Make Component",
        )
        .unwrap();
    }
    model
}

fn entities(model: &Model) -> Vec<Entity> {
    let geometry = model.body(BodyId::Root).unwrap().geometry();
    let mut out: Vec<Entity> = geometry.edges().map(|(id, _)| Entity::Edge(id)).collect();
    out.extend(geometry.faces().map(|(id, _)| Entity::Face(id)));
    out
}

/// One docked row as the app draws it: a fixed-height child with its own body scroll area.
fn row(ui: &mut egui::Ui, salt: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), ROW),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            egui::ScrollArea::vertical()
                .id_salt(("panel-body", salt))
                .max_height(ROW)
                .auto_shrink([false, false])
                .show(ui, add);
        },
    );
}

fn harness() -> Harness<'static, State> {
    let mut h = Harness::builder()
        .with_size(egui::vec2(260.0, 2.0 * ROW + 20.0))
        .build_ui_state(
            |ui, state: &mut State| {
                row(ui, "materials", |ui| {
                    materials_panel(ui, &mut state.materials, &state.model, None);
                });
                row(ui, "components", |ui| {
                    components_panel(ui, &mut state.components, &state.model);
                });
            },
            State {
                model: sample(),
                materials: MaterialsPanel::default(),
                components: ComponentsPanel::default(),
            },
        );
    h.run();
    h
}

fn top(h: &Harness<'_, State>, label: &str) -> Option<f32> {
    h.query_by_label(label).map(|node| node.rect().top())
}

fn wheel_over(h: &mut Harness<'static, State>, label: &str) {
    h.get_by_label(label).hover();
    h.step();
    for _ in 0..6 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -120.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.step();
    }
    h.run_steps(30);
}

#[test]
fn the_wheel_over_materials_leaves_components_alone() {
    let mut h = harness();
    let comp = top(&h, "Comp01").expect("components listed");
    let mat = top(&h, "Mat01").expect("materials listed");
    wheel_over(&mut h, "Mat01");
    assert_ne!(top(&h, "Mat01"), Some(mat), "materials scrolled");
    assert_eq!(top(&h, "Comp01"), Some(comp), "components did not move");
}

#[test]
fn the_wheel_over_components_leaves_materials_alone() {
    let mut h = harness();
    let comp = top(&h, "Comp01").expect("components listed");
    let mat = top(&h, "Mat01").expect("materials listed");
    wheel_over(&mut h, "Comp01");
    assert_ne!(top(&h, "Comp01"), Some(comp), "components scrolled");
    assert_eq!(top(&h, "Mat01"), Some(mat), "materials did not move");
}
