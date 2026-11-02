// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The sunlight the viewport draws with, from the model's geolocation and shadow settings
//! (`docs/specs/m5-app-sun.md`).

use drydock_kernel::model::Model;
use drydock_kernel::sun::{Geolocation, ShadowSettings, sun_direction};
use drydock_render::{Style, SunLight};

/// The sunlight `settings` ask for at `geolocation`: `None` unless shadows or sun shading
/// is on.
pub fn sun_light_for(geolocation: &Geolocation, settings: &ShadowSettings) -> Option<SunLight> {
    if !settings.shadows && !settings.sun_shading {
        return None;
    }
    Some(SunLight {
        direction: sun_direction(geolocation, &settings.time),
        light: settings.light as f32,
        dark: settings.dark as f32,
        shadows: settings.shadows,
        on_ground: settings.shadows && settings.on_ground,
    })
}

/// The sunlight of the model's own geolocation and shadow settings.
pub fn sun_light(model: &Model) -> Option<SunLight> {
    sun_light_for(&model.geolocation(), &model.shadow_settings())
}

/// Put `sun` into `style`; true when it changed, so the viewport must redraw.
pub fn update_style(style: &mut Style, sun: Option<SunLight>) -> bool {
    if style.sun == sun {
        return false;
    }
    style.sun = sun;
    true
}
