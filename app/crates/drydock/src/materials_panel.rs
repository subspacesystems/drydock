// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Materials panel: the model's materials with swatches, choosing the Paint Bucket's
//! material, and a form to create or edit one (name, color, opacity, texture image and
//! size). Kept apart from `App` so tests can drive it with `egui_kittest`; the panel returns
//! what the user asked for and the app applies it through the editor.
//!
//! Behaviour: `docs/specs/m4-app-materials-ui.md`.

use drydock_kernel::model::{Material, MaterialId, Model, Texture};
use drydock_kernel::paint;
use eframe::egui::{self, Color32};
use glam::DVec2;

use crate::color_ui::srgb_color_button;
use crate::dialog::{DialogButton, button_row, field_label};
use crate::icons::{self, Icon};
use crate::swatches::Swatches;
use crate::units_ui::{length_drag, unit_abbrev};

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
    /// Arm the eyedropper: the next click on a face or instance picks its material.
    Eyedropper,
    /// Disarm the eyedropper (its button clicked again while it is pushed in).
    CancelEyedropper,
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
    /// Texture previews for textured materials (`docs/specs/m4-material-swatch-texture.md`).
    pub swatches: Swatches,
    /// The name field takes focus on the next draw.
    focus_name: bool,
    /// The Purge Unused confirmation shows (`docs/specs/m4-purge-confirm.md`).
    purge_confirm: bool,
    /// The editor's eyedropper is armed; the app sets this before each draw and the
    /// eyedropper button shows pushed in (`docs/specs/m6-eyedropper-2.md`).
    pub eyedropper_armed: bool,
    /// The material that was current when the panel last drew; `None` before the first
    /// draw. Used to scroll a newly current material into view
    /// (`docs/specs/m6-eyedropper-2.md`).
    last_current: Option<Option<MaterialId>>,
}

impl MaterialsPanel {
    /// True while the Purge Unused confirmation shows; the app keeps viewport keys away
    /// from the model then, as for its other dialogs. Behaviour:
    /// `docs/specs/m4-purge-confirm.md`.
    pub fn confirming(&self) -> bool {
        self.purge_confirm
    }
}

/// Whether a material called `name` shows for the search text `query`: the trimmed query
/// is empty, or `name` contains it ignoring case. Behaviour: `docs/specs/m4-ui-materials-layout.md`.
pub fn matches_search(name: &str, query: &str) -> bool {
    let query = query.trim();
    query.is_empty() || name.to_lowercase().contains(&query.to_lowercase())
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

    let unit = model.length_format().input_unit();
    let mut action = None;
    let mut new_form = false;
    let mut edit_form = None;
    ui.horizontal_wrapped(|ui| {
        if icons::icon_button(ui, Icon::New, "New...", "New material", false).clicked() {
            new_form = true;
        }
        let editable = current.and_then(|id| model.material(id).map(|material| (id, material)));
        ui.add_enabled_ui(editable.is_some(), |ui| {
            if icons::icon_button(
                ui,
                Icon::Edit,
                "Edit...",
                "Edit the current material",
                false,
            )
            .clicked()
                && let Some((id, material)) = editable
            {
                edit_form = Some(MaterialForm::edit(id, material));
            }
        });
        if icons::icon_button(
            ui,
            Icon::Purge,
            "Purge Unused",
            "Remove the materials nothing uses",
            false,
        )
        .clicked()
        {
            panel.purge_confirm = true;
        }
        if icons::icon_button(
            ui,
            Icon::Eyedropper,
            "Eyedropper",
            "Pick a material from the model",
            panel.eyedropper_armed,
        )
        .clicked()
        {
            action = Some(if panel.eyedropper_armed {
                MaterialAction::CancelEyedropper
            } else {
                MaterialAction::Eyedropper
            });
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

    ui.add(
        egui::TextEdit::singleline(&mut panel.search)
            .hint_text("Search")
            .desired_width(f32::INFINITY),
    );

    ui.horizontal_wrapped(|ui| {
        if icons::icon_button(ui, Icon::List, "List", "Show as a list", !panel.tiles).clicked() {
            panel.tiles = false;
        }
        if icons::icon_button(ui, Icon::Tiles, "Tiles", "Show as tiles", panel.tiles).clicked() {
            panel.tiles = true;
        }
    });

    let search = panel.search.clone();
    let tiles = panel.tiles;
    let mut swatches = panel.swatches.clone();
    let focus_name = std::mem::take(&mut panel.focus_name);
    // Scroll to the current material only when it changes, so the user's own scrolling
    // sticks (`docs/specs/m6-eyedropper-2.md`).
    let scroll_to_current = panel.last_current != Some(current);
    panel.last_current = Some(current);
    let mut close = false;
    // Its own id salt: sibling panels' uis share an id, so unsalted lists would share
    // one scroll state.
    egui::ScrollArea::vertical()
        .id_salt("materials-list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if let Some(form) = &mut panel.form {
                ui.separator();
                ui.heading(if form.editing.is_some() {
                    "Edit Material"
                } else {
                    "New Material"
                });

                ui.horizontal(|ui| {
                    ui.label(field_label("Name"));
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
                    ui.label(field_label("Color"));
                    srgb_color_button(ui, &mut form.color);
                });

                ui.horizontal(|ui| {
                    ui.label(field_label("Opacity"));
                    let spacing = ui.spacing().item_spacing.x;
                    let padding = ui.spacing().button_padding.x;
                    let scroll = ui.spacing().scroll.allocated_width();
                    let value_width = ui.spacing().interact_size.x + 2.0 * padding;
                    ui.spacing_mut().slider_width =
                        (ui.available_width() - value_width - 3.0 * spacing - scroll).max(20.0);
                    crate::look::slider(ui, egui::Slider::new(&mut form.opacity, 0.0..=1.0));
                });

                ui.label(field_label("Texture image"));
                ui.add(egui::TextEdit::singleline(&mut form.texture).desired_width(f32::INFINITY));

                let unit_name = unit_abbrev(unit);
                ui.horizontal(|ui| {
                    let label = ui.label(field_label(&format!("Width ({unit_name})")));
                    ui.add(length_drag(&mut form.size[0], unit))
                        .labelled_by(label.id);
                });
                ui.horizontal(|ui| {
                    let label = ui.label(field_label(&format!("Height ({unit_name})")));
                    ui.add(length_drag(&mut form.size[1], unit))
                        .labelled_by(label.id);
                });

                let material = form.material();
                let editing = form.editing;
                match button_row(
                    ui,
                    &[
                        DialogButton {
                            label: "OK",
                            default: true,
                            enabled: material.is_some(),
                        },
                        DialogButton {
                            label: "Cancel",
                            default: false,
                            enabled: true,
                        },
                    ],
                ) {
                    Some(0) => {
                        if let Some(material) = material {
                            action = Some(match editing {
                                Some(id) => MaterialAction::Edit(id, material),
                                None => MaterialAction::Create(material),
                            });
                            close = true;
                        }
                    }
                    Some(1) => close = true,
                    _ => {}
                }
            }

            let entries = if tiles {
                ui.horizontal_wrapped(|ui| {
                    material_entries(
                        ui,
                        &mut swatches,
                        model,
                        current,
                        &search,
                        true,
                        scroll_to_current,
                    )
                })
                .inner
            } else {
                material_entries(
                    ui,
                    &mut swatches,
                    model,
                    current,
                    &search,
                    false,
                    scroll_to_current,
                )
            };
            if action.is_none() {
                action = entries;
            }
        });
    if close {
        panel.form = None;
    }

    if panel.purge_confirm {
        let unused = paint::unused_materials(model);
        let mut purge = false;
        let mut close = false;
        let response = egui::Modal::new(egui::Id::new("purge-unused")).show(ui.ctx(), |ui| {
            ui.set_width(360.0);
            if unused.is_empty() {
                ui.label("No unused materials to remove.");
                if button_row(
                    ui,
                    &[DialogButton {
                        label: "OK",
                        default: true,
                        enabled: true,
                    }],
                ) == Some(0)
                {
                    close = true;
                }
            } else {
                ui.label("The following unused materials will be removed from the model:");
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for &id in &unused {
                            if let Some(material) = model.material(id) {
                                ui.label(&material.name);
                            }
                        }
                    });
                match button_row(
                    ui,
                    &[
                        DialogButton {
                            label: "Purge",
                            default: true,
                            enabled: true,
                        },
                        DialogButton {
                            label: "Cancel",
                            default: false,
                            enabled: true,
                        },
                    ],
                ) {
                    Some(0) => {
                        purge = true;
                        close = true;
                    }
                    Some(1) => close = true,
                    _ => {}
                }
            }
        });
        if close || response.should_close() {
            panel.purge_confirm = false;
        }
        if purge {
            action = Some(MaterialAction::Purge);
        }
    }

    action
}

/// The Default entry and the matching materials, as rows or tiles.
fn material_entries(
    ui: &mut egui::Ui,
    swatches: &mut Swatches,
    model: &Model,
    current: Option<MaterialId>,
    search: &str,
    tiles: bool,
    scroll_to_current: bool,
) -> Option<MaterialAction> {
    let mut action = None;
    if matches_search("Default", search) {
        let response = if tiles {
            material_tile(
                ui,
                swatches,
                "Default",
                Color32::from_gray(230),
                current.is_none(),
                None,
            )
        } else {
            material_row(
                ui,
                swatches,
                "Default",
                Color32::from_gray(230),
                current.is_none(),
                None,
            )
        };
        if current.is_none() && scroll_to_current {
            response.scroll_to_me(None);
        }
        if response.clicked() {
            action = Some(MaterialAction::Choose(None));
        }
    }
    for (id, material) in model.materials() {
        if !matches_search(&material.name, search) {
            continue;
        }
        let color = Color32::from_rgb(
            channel_to_byte(material.color[0]),
            channel_to_byte(material.color[1]),
            channel_to_byte(material.color[2]),
        );
        let texture = material
            .texture
            .as_ref()
            .map(|texture| texture.path.as_str());
        let selected = current == Some(id);
        let response = if tiles {
            material_tile(ui, swatches, &material.name, color, selected, texture)
        } else {
            material_row(ui, swatches, &material.name, color, selected, texture)
        };
        if selected && scroll_to_current {
            response.scroll_to_me(None);
        }
        if response.clicked() {
            action = Some(MaterialAction::Choose(Some(id)));
        }
    }
    action
}

/// One selectable row: a color swatch, then the material's name.
fn material_row(
    ui: &mut egui::Ui,
    swatches: &mut Swatches,
    label: &str,
    color: Color32,
    selected: bool,
    hover: Option<&str>,
) -> egui::Response {
    let response = ui
        .horizontal(|ui| {
            let (rect, swatch) =
                ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
            paint_swatch(ui, swatches, rect, color, hover);
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

/// One selectable tile: the whole tile is one widget, with a color swatch and the
/// material's name under it.
fn material_tile(
    ui: &mut egui::Ui,
    swatches: &mut Swatches,
    label: &str,
    color: Color32,
    selected: bool,
    hover: Option<&str>,
) -> egui::Response {
    let height = 40.0 + ui.spacing().item_spacing.y + ui.spacing().interact_size.y;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(64.0, height), egui::Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label)
    });

    let widgets = ui.style().interact_selectable(&response, selected);
    let selection = ui.style().visuals.selection;
    if selected {
        let fill = if response.hovered() {
            widgets.weak_bg_fill
        } else {
            selection.bg_fill
        };
        ui.painter().rect_filled(rect, 2.0, fill);
        ui.painter()
            .rect_stroke(rect, 2.0, selection.stroke, egui::StrokeKind::Inside);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 2.0, widgets.weak_bg_fill);
    }

    let swatch = egui::Rect::from_min_size(
        rect.min + egui::vec2(2.0, 2.0),
        egui::vec2(rect.width() - 4.0, 36.0),
    );
    paint_swatch(ui, swatches, swatch, color, hover);

    let text = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 2.0, rect.top() + 40.0),
        egui::pos2(rect.right() - 2.0, rect.bottom()),
    );
    let text_color = if selected {
        selection.stroke.color
    } else {
        ui.style().visuals.text_color()
    };
    paint_tile_name(ui, text, label, text_color);

    let tooltip = match hover {
        Some(texture) => format!("{label}\n{texture}"),
        None => label.to_owned(),
    };
    response.on_hover_text(tooltip)
}

/// Draw `name` on one line, centred in `rect` and truncated with an ellipsis.
fn paint_tile_name(ui: &egui::Ui, rect: egui::Rect, name: &str, color: Color32) {
    let font = egui::TextStyle::Button.resolve(ui.style());
    let mut job = egui::text::LayoutJob::simple_singleline(name.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width().max(0.0));
    let galley = ui.painter().layout_job(job);
    let pos = egui::pos2(
        rect.center().x - galley.size().x / 2.0,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(pos, galley, color);
}

/// The swatch: a texture preview when it's ready, otherwise the flat color.
fn paint_swatch(
    ui: &mut egui::Ui,
    swatches: &mut Swatches,
    rect: egui::Rect,
    color: Color32,
    texture: Option<&str>,
) {
    if let Some(path) = texture
        && let Some(id) = swatches.get(ui.ctx(), path)
        && let crate::swatches::SwatchState::Ready { size } = swatches.state(path)
    {
        ui.painter().image(
            id,
            rect,
            crate::swatches::cover_uv(size, rect.size()),
            Color32::WHITE,
        );
        return;
    }
    ui.painter().rect_filled(rect, 2.0, color);
}

fn channel_to_byte(channel: f32) -> u8 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}
