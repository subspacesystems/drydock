// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Materials panel: the model's materials with swatches, choosing the Paint Bucket's
//! material, and a form to create or edit one (name, color, opacity, texture image and
//! size). Kept apart from `App` so tests can drive it with `egui_kittest`; the panel returns
//! what the user asked for and the app applies it through the editor.
//!
//! Behaviour: `docs/specs/m4-app-materials-ui.md`.

use eframe::egui::{self, Color32};
use glam::DVec2;
use modeler_kernel::model::{Material, MaterialId, Model, Texture};
use modeler_kernel::paint;

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
        MaterialForm {
            editing: None,
            name: String::new(),
            color: [0.8, 0.8, 0.8],
            opacity: 1.0,
            texture: String::new(),
            size: [1.0, 1.0],
        }
    }

    /// The form for editing material `id`, filled from `material`.
    pub fn edit(id: MaterialId, material: &Material) -> MaterialForm {
        MaterialForm {
            editing: Some(id),
            name: material.name.clone(),
            color: [material.color[0], material.color[1], material.color[2]],
            opacity: material.color[3],
            texture: material
                .texture
                .as_ref()
                .map_or_else(String::new, |texture| texture.path.clone()),
            size: material
                .texture
                .as_ref()
                .map_or([1.0, 1.0], |texture| [texture.size.x, texture.size.y]),
        }
    }

    /// The material the fields describe; `None` while a field is out of range.
    pub fn material(&self) -> Option<Material> {
        let color = [self.color[0], self.color[1], self.color[2], self.opacity];
        if color
            .iter()
            .any(|channel| !channel.is_finite() || !(0.0..=1.0).contains(channel))
        {
            return None;
        }
        let path = self.texture.trim();
        let texture = if path.is_empty() {
            None
        } else {
            let size = DVec2::new(self.size[0], self.size[1]);
            if !size.x.is_finite() || size.x <= 0.0 || !size.y.is_finite() || size.y <= 0.0 {
                return None;
            }
            Some(Texture::new(path, size))
        };
        Some(Material {
            name: self.name.trim().to_string(),
            color,
            texture,
        })
    }
}

/// The panel's own state between frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialsPanel {
    /// The open new / edit form, if any.
    pub form: Option<MaterialForm>,
    /// The search box's text (`docs/specs/m4-ui-materials-layout.md`).
    pub search: String,
    /// Show the materials as tiles rather than a list.
    pub tiles: bool,
    /// The name field takes focus on the next draw.
    focus_name: bool,
}

/// Whether a material called `name` shows for the search text `query`: the trimmed query
/// is empty, or `name` contains it ignoring case. Behaviour: `docs/specs/m4-ui-materials-layout.md`.
pub fn matches_search(name: &str, query: &str) -> bool {
    let _ = (name, query);
    unimplemented!("m4-ui-materials-layout")
}

/// Draw the panel into `ui` for `model`, with `current` the Paint Bucket's material.
/// `Some` on the frame the user asks for something.
pub fn materials_panel(
    ui: &mut egui::Ui,
    panel: &mut MaterialsPanel,
    model: &Model,
    current: Option<MaterialId>,
) -> Option<MaterialAction> {
    if let Some(form) = &panel.form
        && let Some(id) = form.editing
        && model.material(id).is_none()
    {
        panel.form = None;
    }

    let mut action = None;
    ui.heading("Materials");

    if material_row(
        ui,
        "Default",
        Color32::from_gray(230),
        current.is_none(),
        None,
    )
    .clicked()
    {
        action = Some(MaterialAction::Choose(None));
    }
    for (id, material) in model.materials() {
        let color = Color32::from_rgb(
            channel_to_byte(material.color[0]),
            channel_to_byte(material.color[1]),
            channel_to_byte(material.color[2]),
        );
        let texture = material
            .texture
            .as_ref()
            .map(|texture| texture.path.as_str());
        if material_row(ui, &material.name, color, current == Some(id), texture).clicked() {
            action = Some(MaterialAction::Choose(Some(id)));
        }
    }

    let mut new_form = false;
    let mut edit_form = None;
    ui.horizontal_wrapped(|ui| {
        if ui.button("New...").clicked() {
            new_form = true;
        }
        let editable = current.and_then(|id| model.material(id).map(|material| (id, material)));
        if ui
            .add_enabled(editable.is_some(), egui::Button::new("Edit..."))
            .clicked()
            && let Some((id, material)) = editable
        {
            edit_form = Some(MaterialForm::edit(id, material));
        }
        if ui.button("Purge Unused").clicked() {
            action = Some(MaterialAction::Purge);
        }
    });
    if new_form {
        panel.form = Some(MaterialForm::new_material());
        panel.focus_name = true;
    }
    if let Some(form) = edit_form {
        panel.form = Some(form);
        panel.focus_name = true;
    }

    let focus_name = std::mem::take(&mut panel.focus_name);
    let mut close = false;
    if let Some(form) = &mut panel.form {
        ui.separator();
        ui.heading(if form.editing.is_some() {
            "Edit Material"
        } else {
            "New Material"
        });

        ui.horizontal(|ui| {
            ui.label("Name");
            let hint = paint::unique_material_name(model, "Material");
            let response = ui.add(
                egui::TextEdit::singleline(&mut form.name)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY),
            );
            if focus_name {
                response.request_focus();
            }
        });

        ui.horizontal(|ui| {
            ui.label("Color");
            let mut color = form.color;
            if ui.color_edit_button_rgb(&mut color).changed() {
                form.color = color;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Opacity");
            let value_width = ui.spacing().interact_size.x;
            let spacing = ui.spacing().item_spacing.x;
            ui.spacing_mut().slider_width =
                (ui.available_width() - value_width - spacing).max(20.0);
            ui.add(egui::Slider::new(&mut form.opacity, 0.0..=1.0));
        });

        ui.label("Texture image");
        ui.add(egui::TextEdit::singleline(&mut form.texture).desired_width(f32::INFINITY));

        ui.horizontal(|ui| {
            ui.label("Width (m)");
            ui.add(egui::DragValue::new(&mut form.size[0]));
        });
        ui.horizontal(|ui| {
            ui.label("Height (m)");
            ui.add(egui::DragValue::new(&mut form.size[1]));
        });

        let material = form.material();
        let editing = form.editing;
        ui.horizontal(|ui| {
            if ui
                .add_enabled(material.is_some(), egui::Button::new("OK"))
                .clicked()
                && let Some(material) = material
            {
                action = Some(match editing {
                    Some(id) => MaterialAction::Edit(id, material),
                    None => MaterialAction::Create(material),
                });
                close = true;
            }
            if ui.button("Cancel").clicked() {
                close = true;
            }
        });
    }
    if close {
        panel.form = None;
    }

    action
}

/// One selectable row: a color swatch, then the material's name.
fn material_row(
    ui: &mut egui::Ui,
    label: &str,
    color: Color32,
    selected: bool,
    hover: Option<&str>,
) -> egui::Response {
    let response = ui
        .horizontal(|ui| {
            let (rect, swatch) =
                ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
            ui.painter().rect_filled(rect, 2.0, color);
            let text = ui.add(egui::Button::selectable(selected, label).truncate());
            swatch.union(text)
        })
        .inner;
    let tooltip = match hover {
        Some(texture) => format!("{label}\n{texture}"),
        None => label.to_owned(),
    };
    response.on_hover_text(tooltip)
}

fn channel_to_byte(channel: f32) -> u8 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}
