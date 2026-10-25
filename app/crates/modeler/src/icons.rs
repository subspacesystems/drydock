// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The app's own icons, drawn as vector shapes so they scale to any size and take the
//! theme's text color. No image files: every icon is a few lines, circles and paths.
//!
//! Behaviour: `docs/specs/m6-icons.md`.

use crate::input::Tool;
use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, vec2};

/// Side of an icon button in points (the icon is drawn inside, with a margin).
pub const BUTTON_SIZE: f32 = 28.0;

/// Something the UI shows as an icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    /// A tool's toolbar button.
    Tool(Tool),
    /// Materials panel: show the list.
    List,
    /// Materials panel: show large tiles.
    Tiles,
    /// Materials panel: new material.
    New,
    /// Materials panel: edit the current material.
    Edit,
    /// Materials panel: purge unused materials.
    Purge,
}

/// Every tool, in `chrome::TOOL_BUTTONS` order.
pub const TOOLS: [Tool; 16] = [
    Tool::Select,
    Tool::Line,
    Tool::Rectangle,
    Tool::Circle,
    Tool::Arc,
    Tool::Arc3,
    Tool::PushPull,
    Tool::Move,
    Tool::Rotate,
    Tool::Scale,
    Tool::Tape,
    Tool::Erase,
    Tool::Paint,
    Tool::Orbit,
    Tool::Pan,
    Tool::Zoom,
];

/// Every icon: the tools in `TOOLS` order, then List, Tiles, New, Edit, Purge.
pub fn all() -> Vec<Icon> {
    let mut icons: Vec<Icon> = TOOLS.iter().map(|tool| Icon::Tool(*tool)).collect();
    icons.extend([Icon::List, Icon::Tiles, Icon::New, Icon::Edit, Icon::Purge]);
    icons
}

/// Small helper for drawing in normalized icon coordinates: `(-1, -1)` is the top-left
/// of the square, `(1, 1)` the bottom-right. Screen y grows downwards.
struct Draw {
    center: Pos2,
    half: f32,
    stroke: Stroke,
    color: Color32,
    fill: Color32,
    out: Vec<Shape>,
}

impl Draw {
    fn at(&self, x: f32, y: f32) -> Pos2 {
        self.center + vec2(x, y) * self.half
    }

    fn seg(&mut self, a: (f32, f32), b: (f32, f32)) {
        self.out.push(Shape::line_segment(
            [self.at(a.0, a.1), self.at(b.0, b.1)],
            self.stroke,
        ));
    }

    fn line(&mut self, points: &[(f32, f32)]) {
        let points = points.iter().map(|(x, y)| self.at(*x, *y)).collect();
        self.out.push(Shape::line(points, self.stroke));
    }

    fn closed(&mut self, points: &[(f32, f32)]) {
        let points = points.iter().map(|(x, y)| self.at(*x, *y)).collect();
        self.out.push(Shape::closed_line(points, self.stroke));
    }

    fn poly(&mut self, points: &[(f32, f32)]) {
        let points = points.iter().map(|(x, y)| self.at(*x, *y)).collect();
        self.out
            .push(Shape::convex_polygon(points, self.color, Stroke::NONE));
    }

    fn circle(&mut self, x: f32, y: f32, r: f32) {
        self.out.push(Shape::circle_stroke(
            self.at(x, y),
            r * self.half,
            self.stroke,
        ));
    }

    fn dot(&mut self, x: f32, y: f32, r: f32) {
        self.out.push(Shape::circle_filled(
            self.at(x, y),
            r * self.half,
            self.color,
        ));
    }

    fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.closed(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)]);
    }

    fn rect_fill(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let rect = Rect::from_min_max(self.at(x0, y0), self.at(x1, y1));
        self.out.push(Shape::rect_filled(rect, 0.0, self.fill));
    }

    fn quad(&mut self, a: (f32, f32), control: (f32, f32), b: (f32, f32)) {
        let points = [a, control, b].map(|(x, y)| self.at(x, y));
        self.out.push(Shape::QuadraticBezier(
            egui::epaint::QuadraticBezierShape::from_points_stroke(
                points,
                false,
                Color32::TRANSPARENT,
                self.stroke,
            ),
        ));
    }

    /// A filled arrowhead at `tip`, pointing away from `from`.
    fn arrow(&mut self, from: (f32, f32), tip: (f32, f32), size: f32) {
        let (dx, dy) = (tip.0 - from.0, tip.1 - from.1);
        let len = (dx * dx + dy * dy).sqrt();
        if len <= f32::EPSILON {
            return;
        }
        let (ux, uy) = (dx / len, dy / len);
        let (px, py) = (-uy, ux);
        let base = (tip.0 - ux * size, tip.1 - uy * size);
        let left = (base.0 + px * size * 0.5, base.1 + py * size * 0.5);
        let right = (base.0 - px * size * 0.5, base.1 - py * size * 0.5);
        self.poly(&[tip, left, right]);
    }
}

fn rotate(point: (f32, f32), angle: f32) -> (f32, f32) {
    let (sin, cos) = angle.sin_cos();
    (point.0 * cos - point.1 * sin, point.0 * sin + point.1 * cos)
}

fn arc_points(cx: f32, cy: f32, r: f32, start: f32, end: f32, steps: usize) -> Vec<(f32, f32)> {
    (0..=steps)
        .map(|i| {
            let t = start + (end - start) * i as f32 / steps as f32;
            (cx + r * t.cos(), cy + r * t.sin())
        })
        .collect()
}

/// The shapes that draw `icon` inside `rect` in `color`. Only lines, paths, circles,
/// rectangles and beziers (no text, no textures); every color used is `color`, possibly
/// with a lower alpha.
pub fn shapes(icon: Icon, rect: Rect, color: Color32) -> Vec<Shape> {
    let side = rect.width().min(rect.height());
    let mut d = Draw {
        center: rect.center(),
        half: side * 0.45,
        stroke: Stroke::new((side / 14.0).max(1.0), color),
        color,
        fill: color.gamma_multiply(0.35),
        out: Vec::new(),
    };

    match icon {
        Icon::Tool(Tool::Select) => {
            d.closed(&[
                (-0.65, -0.8),
                (0.5, 0.15),
                (0.03, 0.22),
                (0.3, 0.78),
                (0.02, 0.92),
                (-0.18, 0.36),
                (-0.65, 0.62),
            ]);
        }
        Icon::Tool(Tool::Line) => {
            d.seg((-0.7, 0.7), (0.7, -0.7));
            d.dot(-0.7, 0.7, 0.14);
            d.dot(0.7, -0.7, 0.14);
        }
        Icon::Tool(Tool::Rectangle) => d.rect(-0.7, -0.5, 0.7, 0.5),
        Icon::Tool(Tool::Circle) => {
            d.circle(0.0, 0.0, 0.68);
            d.dot(0.0, 0.0, 0.11);
            d.seg((0.0, 0.0), (0.48, -0.48));
        }
        Icon::Tool(Tool::Arc) => {
            let r = 0.7;
            d.line(&arc_points(
                0.0,
                0.0,
                r,
                std::f32::consts::PI,
                std::f32::consts::TAU,
                20,
            ));
            d.dot(-r, 0.0, 0.11);
            d.dot(r, 0.0, 0.11);
            d.seg((0.0, 0.0), (0.0, -r));
        }
        Icon::Tool(Tool::Arc3) => {
            d.quad((-0.78, 0.45), (0.0, -0.78), (0.78, 0.45));
            d.dot(-0.78, 0.45, 0.12);
            d.dot(0.0, -0.165, 0.12);
            d.dot(0.78, 0.45, 0.12);
        }
        Icon::Tool(Tool::PushPull) => {
            d.closed(&[(-0.8, 0.45), (-0.1, 0.18), (0.8, 0.45), (0.1, 0.72)]);
            d.seg((0.0, 0.34), (0.0, -0.78));
            d.arrow((0.0, 0.1), (0.0, -0.8), 0.26);
        }
        Icon::Tool(Tool::Move) => {
            d.dot(0.0, 0.0, 0.15);
            d.seg((0.5, 0.0), (0.68, 0.0));
            d.arrow((0.5, 0.0), (0.86, 0.0), 0.34);
            d.seg((-0.5, 0.0), (-0.68, 0.0));
            d.arrow((-0.5, 0.0), (-0.86, 0.0), 0.34);
            d.seg((0.0, 0.5), (0.0, 0.68));
            d.arrow((0.0, 0.5), (0.0, 0.86), 0.34);
            d.seg((0.0, -0.5), (0.0, -0.68));
            d.arrow((0.0, -0.5), (0.0, -0.86), 0.34);
        }
        Icon::Tool(Tool::Rotate) => {
            let r = 0.68;
            let start = 0.1 * std::f32::consts::PI;
            let end = 1.9 * std::f32::consts::PI;
            let pts = arc_points(0.0, 0.0, r, start, end, 24);
            let tip = *pts.last().unwrap();
            let from = pts[pts.len() - 3];
            d.line(&pts);
            d.arrow(from, tip, 0.24);
            d.dot(0.0, 0.0, 0.12);
        }
        Icon::Tool(Tool::Scale) => {
            d.rect(-0.8, -0.1, 0.35, 0.8);
            d.rect_fill(-0.8, 0.35, -0.35, 0.8);
            d.seg((-0.2, 0.1), (0.72, -0.72));
            d.arrow((-0.35, 0.0), (0.82, -0.82), 0.24);
        }
        Icon::Tool(Tool::Tape) => {
            d.circle(-0.3, -0.1, 0.5);
            d.dot(-0.3, -0.1, 0.12);
            let a = (0.05, 0.1);
            let b = (0.82, 0.75);
            d.seg(a, b);
            for f in [0.25, 0.5, 0.75] {
                let x = a.0 + (b.0 - a.0) * f;
                let y = a.1 + (b.1 - a.1) * f;
                d.seg((x - 0.11, y + 0.09), (x + 0.11, y - 0.09));
            }
        }
        Icon::Tool(Tool::Erase) => {
            let a = -0.38;
            let corners = [
                rotate((-0.72, -0.34), a),
                rotate((0.55, -0.34), a),
                rotate((0.55, 0.34), a),
                rotate((-0.72, 0.34), a),
            ];
            d.closed(&corners);
            let divider = [rotate((0.15, -0.34), a), rotate((0.15, 0.34), a)];
            d.seg(divider[0], divider[1]);
        }
        Icon::Tool(Tool::Paint) => {
            let a = -0.5;
            let body = [
                rotate((-0.6, -0.45), a),
                rotate((0.45, -0.45), a),
                rotate((0.3, 0.42), a),
                rotate((-0.42, 0.42), a),
            ];
            d.closed(&body);
            let handle: Vec<(f32, f32)> = arc_points(
                -0.08,
                -0.45,
                0.53,
                std::f32::consts::PI,
                std::f32::consts::TAU,
                12,
            )
            .into_iter()
            .map(|p| rotate(p, a))
            .collect();
            d.line(&handle);
            d.poly(&[(0.73, -0.92), (0.9, -0.58), (0.56, -0.58)]);
        }
        Icon::Tool(Tool::Orbit) => {
            let mut pts = Vec::new();
            let n = 32;
            for i in 0..=n {
                let t = std::f32::consts::TAU * i as f32 / n as f32;
                let p = (0.85 * t.cos(), 0.35 * t.sin());
                pts.push(rotate(p, -0.35));
            }
            let tip = pts[2];
            let from = pts[0];
            d.closed(&pts[..n]);
            d.arrow(from, tip, 0.22);
            d.dot(0.0, 0.0, 0.14);
        }
        Icon::Tool(Tool::Pan) => {
            d.seg((-0.6, -0.05), (0.55, -0.05));
            d.seg((-0.45, -0.05), (-0.45, -0.75));
            d.seg((-0.17, -0.05), (-0.17, -0.82));
            d.seg((0.11, -0.05), (0.11, -0.8));
            d.seg((0.39, -0.05), (0.39, -0.68));
            d.seg((-0.6, 0.2), (-0.85, 0.52));
            d.line(&[
                (-0.6, -0.05),
                (-0.6, 0.4),
                (0.0, 0.75),
                (0.55, 0.45),
                (0.55, -0.05),
            ]);
        }
        Icon::Tool(Tool::Zoom) => {
            d.circle(-0.15, -0.2, 0.55);
            d.seg((0.24, 0.19), (0.8, 0.78));
        }
        Icon::List => {
            for y in [-0.5, 0.0, 0.5] {
                d.dot(-0.62, y, 0.1);
                d.seg((-0.35, y), (0.7, y));
            }
        }
        Icon::Tiles => {
            for (x0, x1) in [(-0.7, -0.12), (0.12, 0.7)] {
                for (y0, y1) in [(-0.7, -0.12), (0.12, 0.7)] {
                    d.rect(x0, y0, x1, y1);
                }
            }
        }
        Icon::New => {
            d.seg((-0.7, 0.0), (0.7, 0.0));
            d.seg((0.0, -0.7), (0.0, 0.7));
        }
        Icon::Edit => {
            let a = -std::f32::consts::FRAC_PI_4;
            let body = [
                rotate((-0.8, -0.17), a),
                rotate((0.4, -0.17), a),
                rotate((0.82, 0.0), a),
                rotate((0.4, 0.17), a),
                rotate((-0.8, 0.17), a),
            ];
            d.closed(&body);
            d.seg(rotate((0.4, -0.17), a), rotate((0.4, 0.17), a));
        }
        Icon::Purge => {
            d.line(&[(-0.2, -0.5), (-0.2, -0.72), (0.2, -0.72), (0.2, -0.5)]);
            d.seg((-0.65, -0.5), (0.65, -0.5));
            d.seg((-0.5, -0.4), (0.5, -0.4));
            d.line(&[(-0.5, -0.4), (-0.36, 0.7), (0.36, 0.7), (0.5, -0.4)]);
            d.seg((-0.15, -0.2), (-0.1, 0.5));
            d.seg((0.15, -0.2), (0.1, 0.5));
        }
    }

    d.out
}

/// Draw `icon` inside `rect`.
pub fn paint(painter: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    painter.extend(shapes(icon, rect, color));
}

/// A square `BUTTON_SIZE` button showing `icon`, drawn selected when `selected`. Screen
/// readers and tests find it by `label`; hovering shows `tooltip`.
pub fn icon_button(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    tooltip: &str,
    selected: bool,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(BUTTON_SIZE), Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label)
    });

    let visuals = ui.style().interact_selectable(&response, selected);
    let painter = ui.painter();
    if selected || response.hovered() {
        painter.rect_filled(rect, 4.0, visuals.weak_bg_fill);
    }
    let icon_rect = rect.shrink(4.0);
    painter.extend(shapes(icon, icon_rect, visuals.fg_stroke.color));

    response.on_hover_text(tooltip)
}
