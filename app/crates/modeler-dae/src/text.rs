// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Text to flat outlines: a font's glyph outlines flattened to closed polygon loops.
//!
//! Behaviour: `docs/specs/m7-text-outline.md`.

use std::fmt;

use glam::DVec2;

/// One closed polygon loop of a glyph.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLoop {
    /// Corners in order; the loop closes from the last back to the first, which is not repeated.
    /// At least 3, no two consecutive equal.
    pub points: Vec<DVec2>,
    /// True for a counter (the inside of an `O`); false for an outer loop.
    pub hole: bool,
    /// For a hole, the index in [`TextOutline::loops`] of the outer loop that encloses it.
    /// `None` for outer loops.
    pub parent: Option<usize>,
}

/// The outlines of a piece of text, y up, in the unit of `size`.
#[derive(Clone, Debug, PartialEq)]
pub struct TextOutline {
    /// Every loop, by line, then character, then the glyph's own contour order.
    pub loops: Vec<TextLoop>,
    /// Advance of the widest line.
    pub width: f64,
    /// Distance between baselines.
    pub line_height: f64,
    /// Number of lines (`text.split('\n').count()`).
    pub lines: usize,
}

/// Why [`text_outline`] refused.
#[derive(Debug, PartialEq, Eq)]
pub enum TextError {
    /// `size` is not a finite number above zero.
    BadSize,
    /// `tolerance` is not a finite number above zero.
    BadTolerance,
    /// The bytes are not a font this reader understands.
    BadFont,
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextError::BadSize => write!(f, "text size must be a positive number"),
            TextError::BadTolerance => write!(f, "chord tolerance must be a positive number"),
            TextError::BadFont => write!(f, "not a readable font"),
        }
    }
}

impl std::error::Error for TextError {}

/// Flattens `text` set in `font` (TTF/OTF bytes) to closed loops. `size` is the em height and
/// `tolerance` the largest distance between a curve and its polyline, both in the caller's unit.
pub fn text_outline(
    font: &[u8],
    text: &str,
    size: f64,
    tolerance: f64,
) -> Result<TextOutline, TextError> {
    let _ = (font, text, size, tolerance);
    unimplemented!("m7-text-outline")
}
