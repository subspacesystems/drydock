// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Materials panel: the model's materials with swatches, choosing the Paint Bucket's
//! material, and a form to create or edit one (name, color, opacity, texture image and
//! size). Kept apart from `App` so tests can drive it with `egui_kittest`; the panel returns
//! what the user asked for and the app applies it through the editor.
//!
//! Behaviour: `docs/specs/m4-app-materials-ui.md`.

use modeler_kernel::model::{Material, MaterialId, Model};

/// What the user asked for in the panel this frame.
#[derive(Clone, Debug, PartialEq)]
pub enum MaterialAction {
    /// Make this the Paint Bucket's material (`None` = the default material).
    Choose(Option<MaterialId>),
    /// Add this material (the form's OK for a new material).
    Create(Material),
    /// Replace material `id` (the form's OK while editing it).
    Edit(MaterialId, Material),
    /// Remove the materials nothing uses.
    Purge,
}

/// The fields of the new / edit form.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialForm {
    /// The material being edited; `None` for a new one.
    pub editing: Option<MaterialId>,
    pub name: String,
    /// Red, green and blue, each 0 to 1.
    pub color: [f32; 3],
    /// 0 (invisible) to 1 (opaque).
    pub opacity: f32,
    /// The image file as typed; blank means no texture.
    pub texture: String,
    /// Width and height in meters that one copy of the image covers.
    pub size: [f64; 2],
}

impl MaterialForm {
    /// The form for a new material.
    pub fn new_material() -> MaterialForm {
        unimplemented!("m4-app-materials-ui")
    }

    /// The form for editing material `id`, filled from `material`.
    pub fn edit(id: MaterialId, material: &Material) -> MaterialForm {
        let _ = (id, material);
        unimplemented!("m4-app-materials-ui")
    }

    /// The material the fields describe; `None` while a field is out of range.
    pub fn material(&self) -> Option<Material> {
        unimplemented!("m4-app-materials-ui")
    }
}

/// The panel's own state between frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialsPanel {
    /// The open new / edit form, if any.
    pub form: Option<MaterialForm>,
}

/// Draw the panel into `ui` for `model`, with `current` the Paint Bucket's material.
/// `Some` on the frame the user asks for something.
pub fn materials_panel(
    ui: &mut eframe::egui::Ui,
    panel: &mut MaterialsPanel,
    model: &Model,
    current: Option<MaterialId>,
) -> Option<MaterialAction> {
    let _ = (ui, panel, model, current);
    unimplemented!("m4-app-materials-ui")
}
