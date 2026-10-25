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
use crate::geo_panel::GeoForm;
use crate::shadows_panel::ShadowsEvent;

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
        let _ = (event, settings, model, editor, &mut self.preview);
        unimplemented!("m5-app-sun-windows")
    }

    /// The sun to draw: the previewed settings (with the model's geolocation) while a drag is
    /// in progress, otherwise the model's own (`shading::sun_light`).
    pub fn sun_light(&self, model: &Model) -> Option<SunLight> {
        let _ = (model, &self.preview);
        unimplemented!("m5-app-sun-windows")
    }

    /// Draw whichever windows are open. Returns true when the model changed (a committed
    /// shadow change or an applied geolocation).
    pub fn show(&mut self, ctx: &egui::Context, model: &mut Model, editor: &mut Editor) -> bool {
        let _ = (ctx, model, editor, &mut self.form);
        unimplemented!("m5-app-sun-windows")
    }
}
