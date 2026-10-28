// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m6-eyedropper-2: a pipette icon that is also the cursor, and the Materials panel showing
//! the current material plainly (a box behind the whole tile, scrolled into view when it
//! changes). See docs/specs/m6-eyedropper-2.md. The editor and button rules are in
//! tests/eyedropper.rs.

use eframe::egui::{
    self, Color32, Pos2, Rect, Vec2,
    epaint::{Mesh, TessellationOptions, Tessellator},
    pos2,
};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use modeler::icons::{self, CURSOR_SIZE, Icon};
use modeler::materials_panel::{MaterialAction, MaterialsPanel, materials_panel};
use modeler_kernel::model::{Material, MaterialId, Model, ModelEdit};

fn mesh(icon: Icon, rect: Rect) -> Mesh {
    let mut tessellator = Tessellator::new(1.0, TessellationOptions::default(), [1, 1], vec![]);
    let mut mesh = Mesh::default();
    for shape in icons::shapes(icon, rect, Color32::WHITE) {
        tessellator.tessellate_shape(shape, &mut mesh);
    }
    mesh
}

/// Pixels of an `n` x `n` grid the icon covers at least half-way.
fn coverage(icon: Icon, n: usize) -> Vec<bool> {
    let mesh = mesh(icon, Rect::from_min_size(Pos2::ZERO, Vec2::splat(n as f32)));
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

// ---- the icon ----

#[test]
fn the_eyedropper_bulb_is_solid_not_a_ring() {
    // A magnifying glass has a hollow ring at the top right; a pipette has a solid bulb.
    let n = 32;
    let ink = coverage(Icon::Eyedropper, n);
    let (mut sx, mut sy, mut count) = (0.0, 0.0, 0.0);
    for y in 0..n / 2 {
        for x in n / 2..n {
            if ink[y * n + x] {
                sx += x as f32;
                sy += y as f32;
                count += 1.0;
            }
        }
    }
    assert!(count > 0.0, "ink in the top-right quarter");
    let (cx, cy) = ((sx / count).round() as usize, (sy / count).round() as usize);
    for y in cy - 1..=cy + 1 {
        for x in cx - 1..=cx + 1 {
            assert!(
                ink[y * n + x],
                "pixel ({x}, {y}) near the centre ({cx}, {cy}) of the top-right ink is empty"
            );
        }
    }
}

#[test]
fn the_eyedropper_runs_from_top_right_to_a_narrow_tip_at_bottom_left() {
    let n = 32;
    let ink = coverage(Icon::Eyedropper, n);
    let quarter = |x0: usize, y0: usize| {
        (y0..y0 + n / 2)
            .flat_map(|y| (x0..x0 + n / 2).map(move |x| (x, y)))
            .filter(|&(x, y)| ink[y * n + x])
            .count()
    };
    let top_right = quarter(n / 2, 0);
    let bottom_left = quarter(0, n / 2);
    let top_left = quarter(0, 0);
    let bottom_right = quarter(n / 2, n / 2);
    assert!(bottom_left > 0, "the tube reaches the bottom left");
    assert!(
        top_right > bottom_left,
        "the bulb end ({top_right}) is heavier than the tip end ({bottom_left})"
    );
    assert!(
        top_left + bottom_right < (top_right + bottom_left) / 2,
        "a diagonal shape: off-diagonal quarters {top_left} + {bottom_right}"
    );
}

#[test]
fn the_cursor_puts_the_pipette_tip_at_the_pointer() {
    for pointer in [pos2(100.0, 200.0), pos2(3.5, 7.25)] {
        let rect = icons::eyedropper_cursor_rect(pointer);
        assert!((rect.width() - CURSOR_SIZE).abs() < 0.01);
        assert!((rect.height() - CURSOR_SIZE).abs() < 0.01);
        // The tip is the ink furthest towards the bottom left.
        let tip = mesh(Icon::Eyedropper, rect)
            .vertices
            .iter()
            .map(|v| v.pos)
            .max_by(|a, b| (a.y - a.x).total_cmp(&(b.y - b.x)))
            .expect("ink");
        assert!(
            tip.distance(pointer) <= 2.5,
            "tip {tip:?} is not at the pointer {pointer:?} (rect {rect:?})"
        );
    }
}

// ---- the panel ----

const WIDTH: f32 = 160.0;
const HEIGHT: f32 = 300.0;

struct State {
    panel: MaterialsPanel,
    model: Model,
    current: Option<MaterialId>,
    actions: Vec<MaterialAction>,
}

fn model_with(count: usize) -> (Model, Vec<MaterialId>) {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let ids = (0..count)
        .map(|i| {
            let shade = (i as f32 / count as f32).min(1.0);
            model.add_material(
                Material::new(&format!("Mat{i:02}"), [shade, 0.5, 0.5, 1.0]),
                &mut log,
            )
        })
        .collect();
    (model, ids)
}

fn harness(model: Model, current: Option<MaterialId>, tiles: bool) -> Harness<'static, State> {
    // The window is the panel's column, so whatever is on screen is in view.
    let mut h = Harness::builder()
        .with_size(egui::vec2(WIDTH, HEIGHT))
        .build_ui_state(
            |ui, state: &mut State| {
                if let Some(action) =
                    materials_panel(ui, &mut state.panel, &state.model, state.current)
                {
                    state.actions.push(action);
                }
            },
            State {
                panel: {
                    let mut panel = MaterialsPanel::default();
                    panel.tiles = tiles;
                    panel
                },
                model,
                current,
                actions: Vec::new(),
            },
        );
    h.run();
    h
}

/// The entry is on screen inside the panel (rows scrolled away may leave the tree).
fn visible(h: &Harness<'_, State>, label: &str) -> bool {
    h.query_by_label(label).is_some_and(|node| {
        let rect = node.rect();
        rect.top() >= -0.5 && rect.bottom() <= HEIGHT + 0.5
    })
}

#[test]
fn the_current_tile_is_one_selected_box_around_swatch_and_name() {
    use egui::accesskit::Toggled;
    let (model, ids) = model_with(2);
    let h = harness(model, Some(ids[1]), true);
    let tile = h.get_by_label("Mat01");
    assert_eq!(tile.accesskit_node().toggled(), Some(Toggled::True));
    let rect = tile.rect();
    assert!(
        rect.width() >= 64.0 - 0.5,
        "the selected box spans the tile: {rect:?}"
    );
    assert!(
        rect.height() >= 40.0 + 18.0,
        "the selected box covers the swatch and the name: {rect:?}"
    );
    assert_eq!(
        h.get_by_label("Mat00").accesskit_node().toggled(),
        Some(Toggled::False)
    );
}

#[test]
fn a_newly_current_material_is_scrolled_into_view() {
    for tiles in [false, true] {
        let (model, ids) = model_with(60);
        let mut h = harness(model, Some(ids[0]), tiles);
        assert!(visible(&h, "Mat00"));
        assert!(!visible(&h, "Mat59"), "the last one starts out of view");
        // The eyedropper (or anything else) makes the last material current.
        h.state_mut().current = Some(ids[59]);
        h.run();
        h.run();
        assert!(visible(&h, "Mat59"), "scrolled to it (tiles: {tiles})");
        // And back to the first.
        h.state_mut().current = Some(ids[0]);
        h.run();
        h.run();
        assert!(visible(&h, "Mat00"), "scrolled back (tiles: {tiles})");
    }
}

#[test]
fn the_panel_only_scrolls_when_the_current_material_changes() {
    let (model, ids) = model_with(60);
    let mut h = harness(model, Some(ids[0]), false);
    assert!(visible(&h, "Mat00"));
    // The user scrolls down; the current material stays put, so the list must not jump
    // back to it.
    h.get_by_label("Mat05").hover();
    h.step();
    for _ in 0..10 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -200.0),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
        });
        h.step();
    }
    // A tooltip may keep asking for frames, so step rather than run; the wheel scroll is
    // animated, so give it time.
    h.run_steps(30);
    assert!(!visible(&h, "Mat00"), "the user's scroll is kept");
}
