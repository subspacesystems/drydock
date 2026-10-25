// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The sunlight the viewport draws with, from the model's geolocation and shadow settings
//! (`docs/specs/m5-app-sun.md`).

use modeler_kernel::model::Model;
use modeler_kernel::sun::{Geolocation, ShadowSettings};
use modeler_render::{Style, SunLight};

/// The sunlight `settings` ask for at `geolocation`: `None` unless shadows or sun shading
/// is on.
pub fn sun_light_for(geolocation: &Geolocation, settings: &ShadowSettings) -> Option<SunLight> {
    let _ = (geolocation, settings);
    unimplemented!()
}

/// The sunlight of the model's own geolocation and shadow settings.
pub fn sun_light(model: &Model) -> Option<SunLight> {
    let _ = model;
    unimplemented!()
}

/// Put `sun` into `style`; true when it changed, so the viewport must redraw.
pub fn update_style(style: &mut Style, sun: Option<SunLight>) -> bool {
    let _ = (style, sun);
    unimplemented!()
}
