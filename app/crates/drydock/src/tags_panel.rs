// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Tags panel: one row per tag with an eye toggle (show / hide), the name (double-click
//! renames) and a delete button, and an Add button. Kept apart from `App` so tests can drive
//! it with `egui_kittest`; the panel returns what the user asked for and the app applies it
//! through the editor (one undo step each).
//!
//! Behaviour: `docs/specs/m10-tags-panel.md`.

use crate::icons::{self, Icon};
use drydock_kernel::model::Model;
use drydock_kernel::tags::{TagId, tag_name_is_good};
use eframe::egui::{self, Sense, Vec2};

/// What the panel remembers between frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagsPanel {
    /// The tag being renamed and the text typed so far.
    pub editing: Option<(TagId, String)>,
    /// Group and component instances in the selection; the app sets it every frame. The
    /// assign buttons are enabled only when it is above 0.
    pub selected: usize,
}

/// What the user asked for in the panel this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TagAction {
    /// The Add button: the app makes a tag named `default_tag_name`.
    Add,
    /// A rename was confirmed (Enter, or focus left the box) with this text.
    Rename(TagId, String),
    /// The eye was clicked: show (`true`) or hide (`false`) the tag.
    SetVisible(TagId, bool),
    /// The delete button was clicked.
    Delete(TagId),
    /// An assign button was clicked: give the selected instances this tag (`None` = the
    /// "No tag" button, which untags them).
    Assign(Option<TagId>),
}

/// The name for a new tag: `Tag N` with the smallest N >= 1 no tag already has (compared
/// exactly).
pub fn default_tag_name(model: &Model) -> String {
    let mut n = 1;
    loop {
        let name = format!("Tag {n}");
        if !model.tags().iter().any(|(_, tag)| tag.name == name) {
            return name;
        }
        n += 1;
    }
}

/// The tags in the order the panel lists them: by name ignoring case, then by id.
pub fn listed_tags(model: &Model) -> Vec<TagId> {
    let mut ids: Vec<TagId> = model.tags().iter().map(|(id, _)| id).collect();
    ids.sort_by(|a, b| {
        let a_name = model
            .tag(*a)
            .map(|tag| tag.name.to_lowercase())
            .unwrap_or_default();
        let b_name = model
            .tag(*b)
            .map(|tag| tag.name.to_lowercase())
            .unwrap_or_default();
        a_name.cmp(&b_name).then_with(|| a.cmp(b))
    });
    ids
}

/// Draw the panel. See the spec for the widgets and their accessible labels.
pub fn tags_panel(ui: &mut egui::Ui, panel: &mut TagsPanel, model: &Model) -> Option<TagAction> {
    if panel
        .editing
        .as_ref()
        .is_some_and(|(id, _)| model.tag(*id).is_none())
    {
        panel.editing = None;
    }

    let mut action = None;
    ui.horizontal(|ui| {
        if ui.button("Add").clicked() {
            action = Some(TagAction::Add);
        }
        let response = ui.add_enabled(panel.selected > 0, egui::Button::new("No tag"));
        if response.clicked() && action.is_none() {
            action = Some(TagAction::Assign(None));
        }
    });

    let ids = listed_tags(model);
    egui::ScrollArea::vertical()
        .id_salt("tags-list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if ids.is_empty() {
                ui.label("No tags");
                return;
            }
            for id in ids {
                let Some(tag) = model.tag(id) else {
                    continue;
                };
                let mut end_rename: Option<(TagId, Option<String>)> = None;
                ui.horizontal(|ui| {
                    if eye_toggle(ui, &tag.name, tag.visible).clicked() && action.is_none() {
                        action = Some(TagAction::SetVisible(id, !tag.visible));
                    }

                    let editing_this = panel
                        .editing
                        .as_ref()
                        .is_some_and(|(editing_id, _)| *editing_id == id);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let delete_enabled = ui.is_enabled();
                        let delete = ui.button("x");
                        delete.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                delete_enabled,
                                format!("Delete {}", tag.name),
                            )
                        });
                        if delete.clicked() && action.is_none() {
                            action = Some(TagAction::Delete(id));
                        }

                        let enabled = panel.selected > 0;
                        let assign = ui.add_enabled(enabled, egui::Button::new("Assign"));
                        assign.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                enabled,
                                format!("Assign selection to {}", tag.name),
                            )
                        });
                        if assign.clicked() && action.is_none() {
                            action = Some(TagAction::Assign(Some(id)));
                        }

                        let left_width = ui.available_width();
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            if editing_this {
                                let mut text = panel
                                    .editing
                                    .as_ref()
                                    .map(|(_, text)| text.clone())
                                    .unwrap_or_default();
                                let text_id = ui.make_persistent_id(("tag-rename", id));
                                let mut output = egui::TextEdit::singleline(&mut text)
                                    .id(text_id)
                                    .desired_width(f32::max(left_width, 40.0))
                                    .show(ui);
                                let has_focus = output.response.has_focus();
                                let lost_focus = output.response.lost_focus();
                                let escape = ui.input(|input| input.key_pressed(egui::Key::Escape));
                                if escape {
                                    end_rename = Some((id, None));
                                } else if lost_focus {
                                    end_rename = Some((id, Some(text.clone())));
                                } else {
                                    if !has_focus {
                                        output.response.request_focus();
                                        output.state.cursor.set_char_range(Some(
                                            egui::text::CCursorRange::select_all(&output.galley),
                                        ));
                                        output.state.store(ui.ctx(), output.response.id);
                                        ui.ctx().request_repaint();
                                    }
                                    panel.editing = Some((id, text));
                                }
                            } else {
                                let response = ui.add(
                                    egui::Label::new(&tag.name).truncate().sense(Sense::click()),
                                );
                                if response.double_clicked() {
                                    panel.editing = Some((id, tag.name.clone()));
                                    ui.ctx().request_repaint();
                                }
                            }
                        });
                    });
                });
                if let Some((id, text)) = end_rename {
                    panel.editing = None;
                    if let Some(text) = text
                        && tag_name_is_good(&text)
                        && text != tag.name
                        && action.is_none()
                    {
                        action = Some(TagAction::Rename(id, text));
                    }
                }
            }
        });
    action
}

/// The eye toggle: the `Eye` icon in the text colour when visible, `EyeOff` in the weak
/// text colour when hidden.
fn eye_toggle(ui: &mut egui::Ui, name: &str, visible: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            enabled,
            visible,
            format!("Visible: {name}"),
        )
    });

    let (icon, color) = if visible {
        (Icon::Eye, ui.visuals().text_color())
    } else {
        (Icon::EyeOff, ui.visuals().weak_text_color())
    };
    icons::paint(ui.painter(), rect, icon, color);
    response
}
