// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Entity Info panel: a description of the whole selection in the open body (what it
//! is, its length, area, size and materials, and for one group or component its names),
//! with fields to rename the definition and the instance. Kept apart from `App` so tests
//! can drive it with `egui_kittest`; the panel returns what the user asked for and the app
//! applies it through the editor.
//!
//! It always describes the whole selection and lives in a docked panel or its own window,
//! never at the click position in the viewport.
//!
//! Behaviour: `docs/specs/m6-entity-info.md`.

use eframe::egui;
use glam::DVec3;
use modeler_kernel::model::{BodyId, DefinitionId, InstanceId, MaterialId, Model};
use modeler_kernel::select::Entity;
use modeler_kernel::tools::scale::bounds;
use modeler_kernel::units::Unit;

/// What the selection is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfoKind {
    Nothing,
    Edges,
    Faces,
    Groups,
    Components,
    Mixed,
}

/// A value over several entities: all the same, or not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shared<T> {
    Same(T),
    Varies,
}

/// The names and material of the one selected group or component.
#[derive(Clone, Debug, PartialEq)]
pub struct InstanceInfo {
    pub id: InstanceId,
    pub definition: DefinitionId,
    pub group: bool,
    pub definition_name: String,
    pub instance_name: String,
    pub material: Option<MaterialId>,
    /// Instances of the definition in the whole model.
    pub copies: usize,
}

/// What the panel shows for a selection.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityInfo {
    pub kind: InfoKind,
    /// Selected entities the body has, by kind.
    pub edges: usize,
    pub faces: usize,
    pub instances: usize,
    /// Total length of the edges, in meters, when only edges are selected.
    pub length: Option<f64>,
    /// Total area of the faces (holes left out), in square meters, when only faces are
    /// selected.
    pub area: Option<f64>,
    /// The faces' front and back materials (`None` inside = the default material), when
    /// only faces are selected.
    pub front: Option<Shared<Option<MaterialId>>>,
    pub back: Option<Shared<Option<MaterialId>>>,
    /// Extent along the body's x, y and z of the box around the selection, when it has one.
    pub size: Option<DVec3>,
    /// When the selection is exactly one instance.
    pub instance: Option<InstanceInfo>,
}

/// Describe `selection` in `body`.
pub fn describe(model: &Model, body: BodyId, selection: &[Entity]) -> EntityInfo {
    let Some(target) = model.body(body) else {
        return EntityInfo {
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
        };
    };

    let mut kept = Vec::with_capacity(selection.len());
    let mut edge_ids = Vec::new();
    let mut face_ids = Vec::new();
    let mut instance_ids = Vec::new();
    for entity in selection {
        match *entity {
            Entity::Edge(id) if target.geometry().edge(id).is_some() => {
                edge_ids.push(id);
                kept.push(*entity);
            }
            Entity::Face(id) if target.geometry().face(id).is_some() => {
                face_ids.push(id);
                kept.push(*entity);
            }
            Entity::Instance(id) if target.instance(id).is_some() => {
                instance_ids.push(id);
                kept.push(*entity);
            }
            _ => {}
        }
    }
    let edges = edge_ids.len();
    let faces = face_ids.len();
    let instances = instance_ids.len();

    let kind = match (edges > 0, faces > 0, instances > 0) {
        (false, false, false) => InfoKind::Nothing,
        (true, false, false) => InfoKind::Edges,
        (false, true, false) => InfoKind::Faces,
        (false, false, _) => {
            let groups = instance_ids
                .iter()
                .filter(|id| {
                    target
                        .instance(**id)
                        .and_then(|instance| model.definition(instance.definition))
                        .is_some_and(|definition| definition.group)
                })
                .count();
            if groups == instances {
                InfoKind::Groups
            } else if groups == 0 {
                InfoKind::Components
            } else {
                InfoKind::Mixed
            }
        }
        _ => InfoKind::Mixed,
    };

    let length = (kind == InfoKind::Edges).then(|| {
        let geometry = target.geometry();
        edge_ids
            .iter()
            .filter_map(|id| {
                let edge = geometry.edge(*id)?;
                let a = geometry.vertex(edge.ends[0])?.position;
                let b = geometry.vertex(edge.ends[1])?.position;
                Some((b - a).length())
            })
            .sum()
    });

    let area = (kind == InfoKind::Faces).then(|| {
        let geometry = target.geometry();
        face_ids
            .iter()
            .filter_map(|id| {
                let face = geometry.face(*id)?;
                let outer =
                    modeler_kernel::geom::area_vector(&geometry.loop_points(&face.outer)).length();
                let holes: f64 = face
                    .inner
                    .iter()
                    .map(|hole| {
                        modeler_kernel::geom::area_vector(&geometry.loop_points(hole)).length()
                    })
                    .sum();
                Some(outer - holes)
            })
            .sum()
    });

    let shared =
        |ids: &[modeler_kernel::topology::FaceId],
         side: fn(&modeler_kernel::model::FaceMaterials) -> Option<MaterialId>| {
            let mut materials = ids.iter().map(|id| side(&target.face_materials(*id)));
            let first = materials.next()?;
            let mut shared = Some(first);
            for material in materials {
                if Some(material) != shared {
                    shared = None;
                    break;
                }
            }
            Some(match shared {
                Some(value) => Shared::Same(value),
                None => Shared::Varies,
            })
        };
    let front = (kind == InfoKind::Faces)
        .then(|| shared(&face_ids, |m| m.front))
        .flatten();
    let back = (kind == InfoKind::Faces)
        .then(|| shared(&face_ids, |m| m.back))
        .flatten();

    let size = bounds(model, body, &kept).map(|box_| box_.max - box_.min);

    let instance = if kept.len() == 1 && instances == 1 {
        instance_ids.first().and_then(|id| {
            let instance = target.instance(*id)?;
            let definition = model.definition(instance.definition);
            Some(InstanceInfo {
                id: *id,
                definition: instance.definition,
                group: definition.is_some_and(|definition| definition.group),
                definition_name: definition.map_or_else(String::new, |d| d.name.clone()),
                instance_name: instance.name.clone(),
                material: instance.material,
                copies: model.instances_of(instance.definition).len(),
            })
        })
    } else {
        None
    };

    EntityInfo {
        kind,
        edges,
        faces,
        instances,
        length,
        area,
        front,
        back,
        size,
        instance,
    }
}

/// The panel's title line for `info`: "No Selection", "Edge", "3 Edges", ...
pub fn title(info: &EntityInfo) -> String {
    fn plural(n: usize, one: &str, many: &str) -> String {
        if n == 1 {
            one.to_string()
        } else {
            format!("{n} {many}")
        }
    }
    match info.kind {
        InfoKind::Nothing => "No Selection".to_string(),
        InfoKind::Edges => plural(info.edges, "Edge", "Edges"),
        InfoKind::Faces => plural(info.faces, "Face", "Faces"),
        InfoKind::Groups => plural(info.instances, "Group", "Groups"),
        InfoKind::Components => plural(info.instances, "Component", "Components"),
        InfoKind::Mixed => format!("{} Entities", info.edges + info.faces + info.instances),
    }
}

/// An area in square meters as text in `unit`'s square.
pub fn format_area(square_meters: f64, unit: Unit) -> String {
    let suffix = match unit {
        Unit::Millimeter => "mm²",
        Unit::Centimeter => "cm²",
        Unit::Meter => "m²",
        Unit::Inch => "in²",
        Unit::Foot => "ft²",
    };
    if !square_meters.is_finite() {
        return format!("0{suffix}");
    }
    let meters = modeler_kernel::units::meters_per(unit);
    let value = square_meters / (meters * meters);
    let mut text = format!("{value:.2}");
    if text.contains('.') {
        text = text.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    format!("{text}{suffix}")
}

/// A material's name for the panel: "Default" for `None`, "Varies", or the material's
/// name (`"?"` when the model doesn't have it).
pub fn material_label(model: &Model, material: Shared<Option<MaterialId>>) -> String {
    match material {
        Shared::Same(None) => "Default".to_string(),
        Shared::Same(Some(id)) => model
            .material(id)
            .map_or_else(|| "?".to_string(), |material| material.name.clone()),
        Shared::Varies => "Varies".to_string(),
    }
}

/// What the user asked for in the panel this frame.
#[derive(Clone, Debug, PartialEq)]
pub enum InfoAction {
    RenameDefinition(DefinitionId, String),
    RenameInstance(InstanceId, String),
}

/// The panel's state between frames: the name fields' text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntityInfoPanel {
    pub definition_name: String,
    pub instance_name: String,
    /// The instance and the names the fields were last filled from.
    pub shown: Option<(InstanceId, String, String)>,
    /// The last description, so a frame doesn't redo it (`docs/specs/m6-panel-speed.md`).
    pub cache: DescribeCache,
}

/// The last `describe` result, reused while the model (`Model::cache_key`), the body and
/// the selection are the same.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DescribeCache {
    key: Option<((u64, u64), BodyId, Vec<Entity>)>,
    info: Option<EntityInfo>,
    computed: u64,
}

impl DescribeCache {
    /// `describe(model, body, selection)`, computed only when one of them changed since
    /// the last call.
    pub fn get(&mut self, model: &Model, body: BodyId, selection: &[Entity]) -> &EntityInfo {
        let key = (model.cache_key(), body, selection.to_vec());
        if !self.key.as_ref().is_some_and(|current| *current == key) {
            self.info = Some(describe(model, body, selection));
            self.key = Some(key);
            self.computed += 1;
        }
        self.info
            .as_ref()
            .expect("the description exists once the key does")
    }

    /// How many times `get` computed a description.
    pub fn computed(&self) -> u64 {
        self.computed
    }
}

/// Draw the panel into `ui` for `selection` in `body`, lengths and areas in `model`'s length
/// format (`docs/specs/m6-units-ui.md`). `Some` on the frame the user renames something.
pub fn entity_info_panel(
    ui: &mut egui::Ui,
    panel: &mut EntityInfoPanel,
    model: &Model,
    body: BodyId,
    selection: &[Entity],
) -> Option<InfoAction> {
    // m6-units-ui: write lengths with `format_with` and areas with `format_area_with`.
    let unit = model.length_format().input_unit();
    let info = panel.cache.get(model, body, selection).clone();
    let mut action = None;

    if let Some(instance) = &info.instance {
        let current = Some((
            instance.id,
            instance.definition_name.clone(),
            instance.instance_name.clone(),
        ));
        if panel.shown != current {
            panel.definition_name = instance.definition_name.clone();
            panel.instance_name = instance.instance_name.clone();
            panel.shown = current;
        }
    } else {
        panel.shown = None;
        panel.definition_name.clear();
        panel.instance_name.clear();
    }

    ui.label(title(&info));

    if info.kind == InfoKind::Edges {
        value_row(
            ui,
            "Length",
            &modeler_kernel::units::format_length(info.length.unwrap_or(0.0), unit, 2),
        );
    }

    if info.kind == InfoKind::Faces {
        value_row(ui, "Area", &format_area(info.area.unwrap_or(0.0), unit));
        value_row(
            ui,
            "Front",
            &material_label(model, info.front.unwrap_or(Shared::Same(None))),
        );
        value_row(
            ui,
            "Back",
            &material_label(model, info.back.unwrap_or(Shared::Same(None))),
        );
    }

    if let Some(instance) = &info.instance {
        if instance.group {
            let response = name_field(ui, "Name", &mut panel.definition_name);
            commit_definition(ui, panel, instance, response, &mut action);
        } else {
            let response = name_field(ui, "Definition", &mut panel.definition_name);
            commit_definition(ui, panel, instance, response, &mut action);
            let response = name_field(ui, "Instance", &mut panel.instance_name);
            commit_instance(ui, panel, instance, response, &mut action);
        }
        value_row(
            ui,
            "Material",
            &material_label(model, Shared::Same(instance.material)),
        );
        if !instance.group {
            value_row(ui, "In model", &instance.copies.to_string());
        }
    }

    if let Some(size) = info.size {
        value_row(
            ui,
            "Width",
            &modeler_kernel::units::format_length(size.x, unit, 2),
        );
        value_row(
            ui,
            "Depth",
            &modeler_kernel::units::format_length(size.y, unit, 2),
        );
        value_row(
            ui,
            "Height",
            &modeler_kernel::units::format_length(size.z, unit, 2),
        );
    }

    if action.is_some() {
        panel.shown = None;
    }
    action
}

fn value_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.label(value);
    });
}

fn name_field(ui: &mut egui::Ui, label: &str, text: &mut String) -> egui::Response {
    ui.horizontal(|ui| {
        let label = ui.label(label);
        ui.add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY))
            .labelled_by(label.id)
    })
    .inner
}

fn field_escape(ui: &egui::Ui, response: &egui::Response) -> bool {
    (response.has_focus() || response.lost_focus())
        && ui.input(|input| input.key_pressed(egui::Key::Escape))
}

fn commit_definition(
    ui: &egui::Ui,
    panel: &mut EntityInfoPanel,
    instance: &InstanceInfo,
    response: egui::Response,
    action: &mut Option<InfoAction>,
) {
    if field_escape(ui, &response) {
        panel.definition_name = instance.definition_name.clone();
    } else if response.lost_focus() {
        let name = panel.definition_name.trim();
        if name.is_empty() || name == instance.definition_name {
            panel.definition_name = instance.definition_name.clone();
        } else if action.is_none() {
            *action = Some(InfoAction::RenameDefinition(
                instance.definition,
                name.to_string(),
            ));
        }
    }
}

fn commit_instance(
    ui: &egui::Ui,
    panel: &mut EntityInfoPanel,
    instance: &InstanceInfo,
    response: egui::Response,
    action: &mut Option<InfoAction>,
) {
    if field_escape(ui, &response) {
        panel.instance_name = instance.instance_name.clone();
    } else if response.lost_focus() {
        let name = panel.instance_name.trim();
        if name == instance.instance_name {
            panel.instance_name = instance.instance_name.clone();
        } else if action.is_none() {
            *action = Some(InfoAction::RenameInstance(instance.id, name.to_string()));
        }
    }
}
