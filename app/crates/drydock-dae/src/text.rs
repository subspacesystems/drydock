// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Text to flat outlines: a font's glyph outlines flattened to closed polygon loops.
//!
//! Behaviour: `docs/specs/m7-text-outline.md`.

use std::fmt;

use glam::DVec2;
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::prelude::{FontRef, LocationRef, Size};
use skrifa::{GlyphId, MetadataProvider};

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

/// A point of an unscaled glyph outline, on or off the curve.
#[derive(Clone, Copy, Debug)]
enum OutlinePoint {
    On(DVec2),
    Off(DVec2),
}

/// A path sink that records the point stream of an unscaled glyph outline.
struct ContourPen {
    /// Contours of the glyph drawn so far.
    contours: Vec<Vec<OutlinePoint>>,
    /// Point stream of the contour currently being drawn.
    current: Vec<OutlinePoint>,
}

impl OutlinePen for ContourPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.current = vec![OutlinePoint::On(DVec2::new(x as f64, y as f64))];
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.current
            .push(OutlinePoint::On(DVec2::new(x as f64, y as f64)));
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.current
            .push(OutlinePoint::Off(DVec2::new(cx0 as f64, cy0 as f64)));
        self.current
            .push(OutlinePoint::On(DVec2::new(x as f64, y as f64)));
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.current
            .push(OutlinePoint::Off(DVec2::new(cx0 as f64, cy0 as f64)));
        self.current
            .push(OutlinePoint::Off(DVec2::new(cx1 as f64, cy1 as f64)));
        self.current
            .push(OutlinePoint::On(DVec2::new(x as f64, y as f64)));
    }

    fn close(&mut self) {
        self.contours.push(std::mem::take(&mut self.current));
    }
}

/// Number of equal steps to flatten a quadratic within `tol` font units.
fn quad_steps(p0: DVec2, p1: DVec2, p2: DVec2, tol: f64) -> usize {
    ((p0 - 2.0 * p1 + p2).length() / (4.0 * tol))
        .sqrt()
        .ceil()
        .max(1.0) as usize
}

/// Number of equal steps to flatten a cubic within `tol` font units.
fn cubic_steps(p0: DVec2, p1: DVec2, p2: DVec2, p3: DVec2, tol: f64) -> usize {
    let m = (p0 - 2.0 * p1 + p2)
        .length()
        .max((p1 - 2.0 * p2 + p3).length());
    (3.0 * m / (4.0 * tol)).sqrt().ceil().max(1.0) as usize
}

/// De Casteljau point of the quadratic through `p0 p1 p2` at `t`.
fn quad_point(p0: DVec2, p1: DVec2, p2: DVec2, t: f64) -> DVec2 {
    p0.lerp(p1, t).lerp(p1.lerp(p2, t), t)
}

/// De Casteljau point of the cubic through `p0 p1 p2 p3` at `t`.
fn cubic_point(p0: DVec2, p1: DVec2, p2: DVec2, p3: DVec2, t: f64) -> DVec2 {
    p0.lerp(p1, t)
        .lerp(p1.lerp(p2, t), t)
        .lerp(p1.lerp(p2, t).lerp(p2.lerp(p3, t), t), t)
}

/// Flattens one contour of a glyph, working in font units at `tol` font units.
/// The point stream starts with a move-to point; each run of off-curve points is
/// followed by its on-curve end point.
fn flatten_contour(pts: &[OutlinePoint], tol: f64) -> Vec<DVec2> {
    let n = pts.len();
    if n < 2 {
        return vec![];
    }
    let Some(start_at) = pts.iter().position(|p| matches!(p, OutlinePoint::On(_))) else {
        return vec![];
    };
    let start = match pts[start_at] {
        OutlinePoint::On(p) => p,
        OutlinePoint::Off(_) => unreachable!(),
    };
    let mut out = vec![start];
    let mut current = start;
    let mut offs: Vec<DVec2> = Vec::new();
    for step in 1..=n {
        match pts[(start_at + step) % n] {
            OutlinePoint::Off(p) => offs.push(p),
            OutlinePoint::On(p) => {
                match offs.len() {
                    0 => out.push(p),
                    1 => {
                        let control = offs[0];
                        let steps = quad_steps(current, control, p, tol);
                        for i in 1..=steps {
                            out.push(quad_point(current, control, p, i as f64 / steps as f64));
                        }
                    }
                    2 => {
                        let (c0, c1) = (offs[0], offs[1]);
                        let steps = cubic_steps(current, c0, c1, p, tol);
                        for i in 1..=steps {
                            out.push(cubic_point(current, c0, c1, p, i as f64 / steps as f64));
                        }
                    }
                    // A run this long is not produced by the documented outline
                    // formats; keep the end point rather than invent a curve.
                    _ => out.push(p),
                }
                offs.clear();
                current = p;
            }
        }
    }
    out
}

/// Drops repeated consecutive points and trailing points equal to the first.
/// Returns an empty vector for a loop with fewer than three distinct corners.
fn clean_loop(points: Vec<DVec2>) -> Vec<DVec2> {
    let mut out: Vec<DVec2> = Vec::with_capacity(points.len());
    for p in points {
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    while out.len() >= 2 && out.last() == out.first() {
        out.pop();
    }
    if out.len() < 3 {
        out.clear();
    }
    out
}

/// Signed shoelace area of a closed polygon.
fn signed_area(points: &[DVec2]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| points[i].perp_dot(points[(i + 1) % n]))
        .sum::<f64>()
        / 2.0
}

/// Even-odd crossing test: whether `point` lies inside `poly`.
fn polygon_contains(point: DVec2, poly: &[DVec2]) -> bool {
    let n = poly.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.y > point.y) != (b.y > point.y)
            && point.x < a.x + (point.y - a.y) / (b.y - a.y) * (b.x - a.x)
        {
            inside = !inside;
        }
    }
    inside
}

/// Tells the loops of one glyph apart into outers and holes and orients them.
fn classify_loops(loops: Vec<Vec<DVec2>>) -> Vec<TextLoop> {
    let areas: Vec<f64> = loops.iter().map(|l| signed_area(l)).collect();
    let mut classes = Vec::with_capacity(loops.len());
    for (i, points) in loops.iter().enumerate() {
        let containers: Vec<usize> = (0..loops.len())
            .filter(|&j| j != i && polygon_contains(points[0], &loops[j]))
            .collect();
        let (hole, parent) = if containers.len() % 2 == 1 {
            let parent = *containers
                .iter()
                .min_by(|&&a, &&b| areas[a].abs().total_cmp(&areas[b].abs()))
                .expect("odd container count is at least one");
            (true, Some(parent))
        } else {
            (false, None)
        };
        classes.push((hole, parent));
    }
    loops
        .into_iter()
        .zip(classes)
        .map(|(mut points, (hole, parent))| {
            let area = signed_area(&points);
            if (hole && area > 0.0) || (!hole && area < 0.0) {
                points.reverse();
            }
            TextLoop {
                points,
                hole,
                parent,
            }
        })
        .collect()
}

/// Flattens `text` set in `font` (TTF/OTF bytes) to closed loops. `size` is the em height and
/// `tolerance` the largest distance between a curve and its polyline, both in the caller's unit.
pub fn text_outline(
    font: &[u8],
    text: &str,
    size: f64,
    tolerance: f64,
) -> Result<TextOutline, TextError> {
    if !size.is_finite() || size <= 0.0 {
        return Err(TextError::BadSize);
    }
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(TextError::BadTolerance);
    }
    let font = FontRef::new(font).map_err(|_| TextError::BadFont)?;
    let location = LocationRef::default();
    let metrics = font.metrics(Size::unscaled(), location);
    let units_per_em = metrics.units_per_em;
    if units_per_em == 0 {
        return Err(TextError::BadFont);
    }
    let k = size / units_per_em as f64;
    let line_height = (metrics.ascent - metrics.descent + metrics.leading) as f64 * k;
    let charmap = font.charmap();
    let glyph_metrics = font.glyph_metrics(Size::unscaled(), location);
    let outline_glyphs = font.outline_glyphs();
    let tol = tolerance / k;
    let space_advance = {
        let gid = charmap.map(' ').unwrap_or(GlyphId::new(0));
        glyph_metrics.advance_width(gid).unwrap_or(0.0) as f64
    };

    let pieces: Vec<&str> = text.split('\n').collect();
    let mut loops = Vec::new();
    let mut width: f64 = 0.0;
    for (line, piece) in pieces.iter().enumerate() {
        let baseline = -(line as f64) * line_height;
        let mut pen = 0.0f64;
        for ch in piece.chars() {
            if ch == '\t' {
                pen += 4.0 * space_advance * k;
                continue;
            }
            if ch.is_control() {
                continue;
            }
            let gid = charmap.map(ch).unwrap_or(GlyphId::new(0));
            if let Some(glyph) = outline_glyphs.get(gid) {
                let mut sink = ContourPen {
                    contours: Vec::new(),
                    current: Vec::new(),
                };
                let settings = DrawSettings::unhinted(Size::unscaled(), location);
                glyph
                    .draw(settings, &mut sink)
                    .map_err(|_| TextError::BadFont)?;
                let contours = sink
                    .contours
                    .iter()
                    .filter_map(|contour| {
                        let points = clean_loop(flatten_contour(contour, tol));
                        (!points.is_empty()).then(|| {
                            points
                                .into_iter()
                                .map(|p| DVec2::new(p.x * k + pen, p.y * k + baseline))
                                .collect()
                        })
                    })
                    .collect();
                let offset = loops.len();
                for mut loop_ in classify_loops(contours) {
                    if let Some(parent) = loop_.parent {
                        loop_.parent = Some(parent + offset);
                    }
                    loops.push(loop_);
                }
            }
            pen += glyph_metrics.advance_width(gid).unwrap_or(0.0) as f64 * k;
        }
        width = width.max(pen);
    }

    Ok(TextOutline {
        loops,
        width,
        line_height,
        lines: pieces.len(),
    })
}
