// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Tags panel: one row per tag with an eye toggle (show / hide), the name (double-click
//! renames) and a delete button, and an Add button. Kept apart from `App` so tests can drive
//! it with `egui_kittest`; the panel returns what the user asked for and the app applies it
//! through the editor (one undo step each).
//!
//! Behaviour: `docs/specs/m10-tags-panel.md`.

use drydock_kernel::model::Model;
use drydock_kernel::tags::{TagId, tag_name_is_good};
use eframe::egui::{self, Sense, Stroke, Vec2, vec2};

/// What the panel remembers between frames.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TagsPanel {
    /// The tag being renamed and the text typed so far.
    pub editing: Option<(TagId, String)>,
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
                    if editing_this {
                        let mut text = panel
                            .editing
                            .as_ref()
                            .map(|(_, text)| text.clone())
                            .unwrap_or_default();
                        let text_id = ui.make_persistent_id(("tag-rename", id));
                        let mut output = egui::TextEdit::singleline(&mut text)
                            .id(text_id)
                            .desired_width(f32::INFINITY)
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
                        let response = ui.add(egui::Label::new(&tag.name).sense(Sense::click()));
                        if response.double_clicked() {
                            panel.editing = Some((id, tag.name.clone()));
                            ui.ctx().request_repaint();
                        }
                    }

                    let enabled = ui.is_enabled();
                    let response = ui.button("x");
                    response.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::Button,
                            enabled,
                            format!("Delete {}", tag.name),
                        )
                    });
                    if response.clicked() && action.is_none() {
                        action = Some(TagAction::Delete(id));
                    }
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

/// The eye toggle: an outline eye with a strike when hidden, in the text colour.
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

    let color = ui.visuals().text_color();
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.3;
    let painter = ui.painter();
    painter.circle_stroke(center, radius, Stroke::new(1.5, color));
    painter.circle_filled(center, radius * 0.35, color);
    if !visible {
        painter.line_segment(
            [
                rect.left_top() + vec2(3.0, 3.0),
                rect.right_bottom() - vec2(3.0, 3.0),
            ],
            Stroke::new(1.5, color),
        );
    }
    response
}
