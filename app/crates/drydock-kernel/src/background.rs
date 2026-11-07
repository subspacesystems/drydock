// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The model's background settings: the plain background color, and a sky and a ground
//! that can each be on or off with their own color. A setting of the model, not of the
//! app's theme. Behaviour: `docs/specs/m10-background-kernel.md`.

/// The background of a new model: a light blue-grey, `#F4F8FC`.
pub const DEFAULT_BACKGROUND: [f32; 3] = [244.0 / 255.0, 248.0 / 255.0, 252.0 / 255.0];

/// The sky of a new model: a light blue, `#A8C8E8`.
pub const DEFAULT_SKY: [f32; 3] = [168.0 / 255.0, 200.0 / 255.0, 232.0 / 255.0];

/// The ground of a new model: a neutral grey, `#9A9A96`.
pub const DEFAULT_GROUND: [f32; 3] = [154.0 / 255.0, 154.0 / 255.0, 150.0 / 255.0];

/// Colors are sRGB, each channel 0 to 1 (like `Material::color`, without alpha).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Background {
    /// Shown wherever neither the sky nor the ground is drawn.
    pub color: [f32; 3],
    /// Draw the sky above the horizon.
    pub sky: bool,
    pub sky_color: [f32; 3],
    /// Draw the ground below the horizon.
    pub ground: bool,
    pub ground_color: [f32; 3],
}

impl Default for Background {
    /// Sky on, ground on, the three default colors.
    fn default() -> Self {
        Background {
            color: DEFAULT_BACKGROUND,
            sky: true,
            sky_color: DEFAULT_SKY,
            ground: true,
            ground_color: DEFAULT_GROUND,
        }
    }
}

impl Background {
    /// Every channel of the three colors is finite and in `0..=1`. The on/off flags don't
    /// matter: a color that isn't drawn must still be valid.
    pub fn is_valid(&self) -> bool {
        unimplemented!()
    }
}
