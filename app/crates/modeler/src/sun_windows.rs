// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The Shadows and Geolocation windows and what they do to the model: slider drags preview
//! the sun without recording anything, a final change records one undo step through the
//! editor. Kept apart from `App` so tests can drive it with `egui_kittest`.
//!
//! Behaviour: `docs/specs/m5-app-sun-windows.md`.

use eframe::egui;
use modeler_kernel::model::Model;
use modeler_kernel::sun::ShadowSettings;
use modeler_render::SunLight;

use crate::editor::Editor;
use crate::geo_panel::{self, GeoForm};
use crate::globe::GlobeView;
use crate::shadows_panel::{self, ShadowsEvent};

/// State of the two windows.
#[derive(Debug, Default)]
pub struct SunWindows {
    /// The Shadows window is showing.
    pub shadows_open: bool,
    /// The Geolocation window is showing.
    pub geo_open: bool,
    /// Settings being dragged, not yet recorded.
    preview: Option<ShadowSettings>,
    /// The text in the Geolocation window; `Some` while the window is open.
    form: Option<GeoForm>,
    /// Which way the Geolocation window's globe faces; `Some` while the window is open.
    globe: Option<GlobeView>,
}

impl SunWindows {
    /// Both windows closed, nothing previewed.
    pub fn new() -> SunWindows {
        SunWindows::default()
    }

    /// Act on what the Shadows panel reported for `settings`: `Preview` remembers them as the
    /// sun to draw (nothing recorded); `Commit` forgets any preview and records them with
    /// `Editor::set_shadow_settings`; `None` does nothing. True when the model changed.
    pub fn apply_shadows_event(
        &mut self,
        event: ShadowsEvent,
        settings: ShadowSettings,
        model: &mut Model,
        editor: &mut Editor,
    ) -> bool {
        match event {
            ShadowsEvent::Preview => {
                self.preview = Some(settings);
                false
            }
            ShadowsEvent::Commit => {
                self.preview = None;
                if settings == model.shadow_settings() {
                    false
                } else {
                    editor.set_shadow_settings(model, settings)
                }
            }
            ShadowsEvent::None => false,
        }
    }

    /// The sun to draw: the previewed settings (with the model's geolocation) while a drag is
    /// in progress, otherwise the model's own (`shading::sun_light`).
    pub fn sun_light(&self, model: &Model) -> Option<SunLight> {
        match &self.preview {
            Some(preview) => crate::shading::sun_light_for(&model.geolocation(), preview),
            None => crate::shading::sun_light(model),
        }
    }

    /// Draw whichever windows are open. Returns true when the model changed (a committed
    /// shadow change or an applied geolocation).
    pub fn show(&mut self, ctx: &egui::Context, model: &mut Model, editor: &mut Editor) -> bool {
        let mut changed = false;

        if self.shadows_open {
            let mut settings = self.preview.unwrap_or_else(|| model.shadow_settings());
            let mut event = ShadowsEvent::None;
            let mut open = self.shadows_open;
            let mut close = false;
            egui::Window::new("Shadows")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    event = shadows_panel::shadows_panel(ui, &mut settings);
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            if !open || close {
                self.shadows_open = false;
                self.preview = None;
            } else if self.apply_shadows_event(event, settings, model, editor) {
                changed = true;
            }
        }

        if self.geo_open {
            if self.form.is_none() {
                let geolocation = model.geolocation();
                self.form = Some(GeoForm::from_geolocation(&geolocation));
                self.globe = Some(GlobeView::facing(
                    geolocation.latitude,
                    geolocation.longitude,
                ));
            }
            let mut open = self.geo_open;
            let mut close = false;
            let mut applied = None;
            egui::Window::new("Geolocation")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ctx, |ui| {
                    if let (Some(form), Some(globe)) = (&mut self.form, &mut self.globe) {
                        applied = geo_panel::geo_panel_with_globe(ui, form, globe);
                    }
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });
            if !open || close {
                self.geo_open = false;
                self.form = None;
                self.globe = None;
            } else if let Some(geolocation) = applied {
                if editor.set_geolocation(model, geolocation) {
                    changed = true;
                }
                self.geo_open = false;
                self.form = None;
                self.globe = None;
            }
        }

        changed
    }

    /// Close both windows and forget any preview or typed form.
    pub fn close(&mut self) {
        self.shadows_open = false;
        self.geo_open = false;
        self.preview = None;
        self.form = None;
        self.globe = None;
    }
}
