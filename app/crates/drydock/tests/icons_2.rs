// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-icons-2.md`: icons centred in their square.

use drydock::icons::{self, Icon};
use drydock::input::Tool;
use eframe::egui::{
    Color32, Pos2, Rect, Vec2,
    epaint::{Mesh, TessellationOptions, Tessellator},
    pos2,
};

fn mesh(icon: Icon, side: f32) -> Mesh {
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(side));
    let mut tessellator = Tessellator::new(1.0, TessellationOptions::default(), [1, 1], vec![]);
    let mut mesh = Mesh::default();
    for shape in icons::shapes(icon, rect, Color32::WHITE) {
        tessellator.tessellate_shape(shape, &mut mesh);
    }
    mesh
}

fn bounds(icon: Icon) -> Rect {
    let mut rect = Rect::NOTHING;
    for vertex in &mesh(icon, 32.0).vertices {
        rect.extend_with(vertex.pos);
    }
    rect
}

/// Pixels of an `n` x `n` grid the icon covers at least half-way.
fn coverage(icon: Icon, n: usize) -> Vec<bool> {
    let mesh = mesh(icon, n as f32);
    let mut alpha = vec![0.0f32; n * n];
    for tri in mesh.indices.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|k| mesh.vertices[tri[k] as usize]);
        let area = (b.pos - a.pos).x * (c.pos - a.pos).y - (b.pos - a.pos).y * (c.pos - a.pos).x;
        if area.abs() < 1e-9 {
            continue;
        }
        let min = a.pos.min(b.pos).min(c.pos);
        let max = a.pos.max(b.pos).max(c.pos);
        for y in (min.y.floor().max(0.0) as usize)..(max.y.ceil().min(n as f32) as usize) {
            for x in (min.x.floor().max(0.0) as usize)..(max.x.ceil().min(n as f32) as usize) {
                let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((b.pos - p).x * (c.pos - p).y - (b.pos - p).y * (c.pos - p).x) / area;
                let w1 = ((c.pos - p).x * (a.pos - p).y - (c.pos - p).y * (a.pos - p).x) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                    let value = (w0 * f32::from(a.color.a())
                        + w1 * f32::from(b.color.a())
                        + w2 * f32::from(c.color.a()))
                        / 255.0;
                    let cell = &mut alpha[y * n + x];
                    *cell = cell.max(value);
                }
            }
        }
    }
    alpha.into_iter().map(|a| a >= 0.5).collect()
}

#[test]
fn every_icon_is_centred_and_not_too_wide() {
    for icon in icons::all() {
        let b = bounds(icon);
        assert!(
            (b.center().x - 16.0).abs() <= 1.0 && (b.center().y - 16.0).abs() <= 1.0,
            "{icon:?} is centred at {:?}, not (16, 16)",
            b.center()
        );
        assert!(
            b.width() <= 30.0 && b.height() <= 30.0,
            "{icon:?} is {:.1} x {:.1}, over 30",
            b.width(),
            b.height()
        );
    }
}

#[test]
fn tool_icons_have_enough_ink_at_toolbar_sizes() {
    for tool in icons::TOOLS {
        for (n, want) in [(32usize, 0.06f32), (20, 0.05)] {
            let covered = coverage(Icon::Tool(tool), n)
                .iter()
                .filter(|on| **on)
                .count();
            let fraction = covered as f32 / (n * n) as f32;
            assert!(
                fraction >= want,
                "{tool:?} covers {:.1}% at {n} points (want {:.0}%)",
                fraction * 100.0,
                want * 100.0
            );
        }
    }
}

#[test]
fn redrawn_icons_stand_apart_from_all_others() {
    let redrawn = [
        Tool::PushPull,
        Tool::Rotate,
        Tool::Scale,
        Tool::Tape,
        Tool::Erase,
        Tool::Paint,
        Tool::Pan,
        Tool::Arc,
        Tool::Arc3,
    ];
    let all = icons::all();
    for tool in redrawn {
        let mine = coverage(Icon::Tool(tool), 32);
        for other in &all {
            if *other == Icon::Tool(tool) {
                continue;
            }
            let differ = mine
                .iter()
                .zip(coverage(*other, 32))
                .filter(|(a, b)| **a != *b)
                .count();
            assert!(
                differ >= 80,
                "{tool:?} and {other:?} differ in only {differ} of 1024 pixels"
            );
        }
    }
}
