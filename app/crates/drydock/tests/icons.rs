// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-icons.md`: the app's own vector icons and the
//! icon button.

use drydock::icons::{self, BUTTON_SIZE, Icon, TOOLS};
use drydock::input::Tool;
use eframe::egui::{
    self, Color32, Pos2, Rect, Shape, Vec2,
    epaint::{Mesh, TessellationOptions, Tessellator},
    pos2, vec2,
};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

fn square(min: Pos2, size: f32) -> Rect {
    Rect::from_min_size(min, Vec2::splat(size))
}

fn mesh(icon: Icon, rect: Rect, color: Color32) -> Mesh {
    let mut tessellator = Tessellator::new(1.0, TessellationOptions::default(), [1, 1], vec![]);
    let mut mesh = Mesh::default();
    for shape in icons::shapes(icon, rect, color) {
        tessellator.tessellate_shape(shape, &mut mesh);
    }
    mesh
}

/// Which pixels of an `n` x `n` grid over `rect` the icon covers at least half-way
/// (alpha sampled at pixel centers, highest covering triangle wins).
fn coverage(icon: Icon, n: usize) -> Vec<bool> {
    let rect = square(Pos2::ZERO, n as f32);
    let mesh = mesh(icon, rect, Color32::WHITE);
    let mut alpha = vec![0.0f32; n * n];
    for tri in mesh.indices.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|k| mesh.vertices[tri[k] as usize]);
        let area = (b.pos - a.pos).x * (c.pos - a.pos).y - (b.pos - a.pos).y * (c.pos - a.pos).x;
        if area.abs() < 1e-9 {
            continue;
        }
        let min = a.pos.min(b.pos).min(c.pos);
        let max = a.pos.max(b.pos).max(c.pos);
        let x0 = (min.x.floor().max(0.0)) as usize;
        let y0 = (min.y.floor().max(0.0)) as usize;
        let x1 = (max.x.ceil().min(n as f32)) as usize;
        let y1 = (max.y.ceil().min(n as f32)) as usize;
        for y in y0..y1 {
            for x in x0..x1 {
                let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((b.pos - p).x * (c.pos - p).y - (b.pos - p).y * (c.pos - p).x) / area;
                let w1 = ((c.pos - p).x * (a.pos - p).y - (c.pos - p).y * (a.pos - p).x) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                    let value = w0 * f32::from(a.color.a())
                        + w1 * f32::from(b.color.a())
                        + w2 * f32::from(c.color.a());
                    let cell = &mut alpha[y * n + x];
                    *cell = cell.max(value / 255.0);
                }
            }
        }
    }
    alpha.into_iter().map(|a| a >= 0.5).collect()
}

fn bounds(mesh: &Mesh) -> Rect {
    let mut rect = Rect::NOTHING;
    for vertex in &mesh.vertices {
        rect.extend_with(vertex.pos);
    }
    rect
}

fn flatten(shapes: Vec<Shape>, out: &mut Vec<Shape>) {
    for shape in shapes {
        match shape {
            Shape::Vec(inner) => flatten(inner, out),
            other => out.push(other),
        }
    }
}

#[test]
fn all_lists_every_tool_then_the_panel_icons() {
    let all = icons::all();
    let mut expected: Vec<Icon> = TOOLS.iter().map(|tool| Icon::Tool(*tool)).collect();
    expected.extend([
        Icon::List,
        Icon::Tiles,
        Icon::New,
        Icon::Edit,
        Icon::Purge,
        Icon::Eyedropper,
    ]);
    assert_eq!(all, expected);
    let tools: Vec<Tool> = drydock::chrome::TOOL_BUTTONS
        .iter()
        .map(|(tool, _)| *tool)
        .collect();
    assert_eq!(TOOLS.to_vec(), tools, "TOOLS follows the toolbar order");
}

#[test]
fn every_icon_draws_inside_its_rect() {
    for icon in icons::all() {
        let rect = square(Pos2::ZERO, 32.0);
        let mesh = mesh(icon, rect, Color32::WHITE);
        assert!(!mesh.vertices.is_empty(), "{icon:?} draws nothing");
        let outer = rect.expand(1.0);
        for vertex in &mesh.vertices {
            assert!(
                outer.contains(vertex.pos),
                "{icon:?} draws at {:?}, outside {rect:?}",
                vertex.pos
            );
        }
        let covered = coverage(icon, 32).iter().filter(|on| **on).count();
        let fraction = covered as f32 / (32.0 * 32.0);
        assert!(
            (0.03..=0.6).contains(&fraction),
            "{icon:?} covers {:.1}% of its square (want 3-60%)",
            fraction * 100.0
        );
    }
}

#[test]
fn icons_are_plain_shapes_in_the_given_color() {
    for icon in icons::all() {
        let mut shapes = Vec::new();
        flatten(
            icons::shapes(icon, square(Pos2::ZERO, 24.0), Color32::RED),
            &mut shapes,
        );
        for shape in &shapes {
            assert!(
                !matches!(shape, Shape::Text(_) | Shape::Mesh(_) | Shape::Callback(_)),
                "{icon:?} uses {shape:?}; icons are lines, paths, circles, rects and curves"
            );
        }
        for (color, name) in [(Color32::RED, "red"), (Color32::BLUE, "blue")] {
            let mesh = mesh(icon, square(Pos2::ZERO, 24.0), color);
            for vertex in &mesh.vertices {
                let c = vertex.color;
                let ok = match name {
                    "red" => c.g() == 0 && c.b() == 0,
                    _ => c.r() == 0 && c.g() == 0,
                };
                assert!(ok, "{icon:?} drawn in {name} has a vertex colored {c:?}");
            }
        }
    }
}

#[test]
fn no_two_icons_look_alike() {
    let all = icons::all();
    let grids: Vec<Vec<bool>> = all.iter().map(|icon| coverage(*icon, 32)).collect();
    for i in 0..all.len() {
        for j in i + 1..all.len() {
            let differ = grids[i]
                .iter()
                .zip(&grids[j])
                .filter(|(a, b)| a != b)
                .count();
            assert!(
                differ >= 40,
                "{:?} and {:?} differ in only {differ} of 1024 pixels",
                all[i],
                all[j]
            );
        }
    }
}

#[test]
fn icons_move_with_their_rect() {
    for icon in icons::all() {
        let here = mesh(icon, square(Pos2::ZERO, 24.0), Color32::WHITE);
        let there = mesh(icon, square(pos2(100.0, 50.0), 24.0), Color32::WHITE);
        let (a, b) = (bounds(&here), bounds(&there));
        let shift = b.min - a.min;
        assert!(
            (shift - vec2(100.0, 50.0)).length() < 0.05 && (b.size() - a.size()).length() < 0.05,
            "{icon:?}: bounds {a:?} at the origin, {b:?} at (100, 50)"
        );
    }
}

#[test]
fn icons_grow_with_their_rect() {
    for icon in icons::all() {
        let small = bounds(&mesh(icon, square(Pos2::ZERO, 32.0), Color32::WHITE));
        let large = bounds(&mesh(icon, square(Pos2::ZERO, 64.0), Color32::WHITE));
        let doubled = small.size() * 2.0;
        assert!(
            (large.size() - doubled).x.abs() <= 4.0 && (large.size() - doubled).y.abs() <= 4.0,
            "{icon:?}: {:?} at 32 points but {:?} at 64",
            small.size(),
            large.size()
        );
        assert!(
            (large.center() - small.center() * 2.0).length() <= 3.0,
            "{icon:?} drifts when scaled"
        );
    }
}

#[derive(Default)]
struct ButtonState {
    clicks: usize,
    rect: Option<Rect>,
    selected: bool,
}

fn button_harness(selected: bool) -> Harness<'static, ButtonState> {
    let state = ButtonState {
        selected,
        ..ButtonState::default()
    };
    Harness::builder()
        .with_size(vec2(200.0, 120.0))
        .build_ui_state(
            |ui, state: &mut ButtonState| {
                let response = icons::icon_button(
                    ui,
                    Icon::Tool(Tool::Line),
                    "Line",
                    "Line (L)",
                    state.selected,
                );
                state.rect = Some(response.rect);
                if response.clicked() {
                    state.clicks += 1;
                }
            },
            state,
        )
}

#[test]
fn icon_button_is_found_by_label_and_clicks() {
    let mut h = button_harness(false);
    h.run();
    h.get_by_label("Line").click();
    h.run();
    assert_eq!(h.state().clicks, 1);
}

#[test]
fn icon_button_is_button_size() {
    let mut h = button_harness(false);
    h.run();
    let rect = h.state().rect.expect("drawn");
    assert_eq!(rect.size(), Vec2::splat(BUTTON_SIZE));
}

#[test]
fn icon_button_reports_whether_it_is_selected() {
    use egui::accesskit::Toggled;
    for selected in [false, true] {
        let mut h = button_harness(selected);
        h.run();
        let toggled = h.get_by_label("Line").accesskit_node().toggled();
        let want = if selected {
            Toggled::True
        } else {
            Toggled::False
        };
        assert_eq!(toggled, Some(want), "selected = {selected}");
    }
}

#[test]
fn icon_button_shows_its_tooltip_on_hover() {
    let mut h = button_harness(false);
    h.run();
    h.get_by_label("Line").hover();
    for _ in 0..90 {
        h.step();
    }
    h.run();
    assert!(
        h.query_by_label("Line (L)").is_some(),
        "hovering shows the tooltip"
    );
}
