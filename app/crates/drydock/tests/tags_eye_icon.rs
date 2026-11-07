// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-tags-eye-icon: the Tags panel's toggle draws the brand `Eye` / `EyeOff` icon, not a
//! painted circle. See docs/specs/m10-tags-eye-icon.md.

use drydock::editor::Editor;
use drydock::icons::{self, Icon};
use drydock::tags_panel::*;
use drydock_kernel::model::*;
use eframe::egui::{self, Color32, Rect, Shape, TextureId, load::SizeHint};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

struct State {
    panel: TagsPanel,
    model: Model,
    editor: Editor,
}

fn harness(visible: bool) -> Harness<'static, State> {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let id = model.add_tag("Walls", &mut log).unwrap();
    let mut editor = Editor::new();
    if !visible {
        editor.set_tag_visible(&mut model, id, false);
    }
    let mut h = Harness::builder()
        .with_size(egui::vec2(400.0, 300.0))
        .build_ui_state(
            |ui, state: &mut State| {
                if let Some(TagAction::SetVisible(id, v)) =
                    tags_panel(ui, &mut state.panel, &state.model)
                {
                    state.editor.set_tag_visible(&mut state.model, id, v);
                }
            },
            State {
                panel: TagsPanel::default(),
                model,
                editor,
            },
        );
    h.run();
    h.run();
    h
}

fn flat(shape: &Shape, out: &mut Vec<Shape>) {
    if let Shape::Vec(v) = shape {
        v.iter().for_each(|s| flat(s, out));
    } else {
        out.push(shape.clone());
    }
}

fn shapes(h: &Harness<'_, State>) -> Vec<Shape> {
    let mut out = Vec::new();
    for clipped in &h.output().shapes {
        flat(&clipped.shape, &mut out);
    }
    out
}

/// The texture egui holds for `icon` at the toggle's 20 x 20 point size (1 point = 1 pixel).
fn texture_of(h: &Harness<'_, State>, icon: Icon) -> TextureId {
    let size = SizeHint::Size {
        width: 20,
        height: 20,
        maintain_aspect_ratio: false,
    };
    match h
        .ctx
        .try_load_texture(&icons::uri(icon), egui::TextureOptions::LINEAR, size)
    {
        Ok(egui::load::TexturePoll::Ready { texture }) => texture.id,
        other => panic!("{icon:?} not loaded: {:?}", other.map(|_| ())),
    }
}

/// Textured quads (bounds, tint, texture) other than the font atlas.
fn quads(shapes: &[Shape]) -> Vec<(Rect, Color32, TextureId)> {
    let mut out = Vec::new();
    for shape in shapes {
        match shape {
            Shape::Mesh(m) if m.texture_id != TextureId::Managed(0) => {
                if let Some(first) = m.vertices.first() {
                    let mut rect = Rect::NOTHING;
                    m.vertices.iter().for_each(|v| rect.extend_with(v.pos));
                    out.push((rect, first.color, m.texture_id));
                }
            }
            Shape::Rect(r) if r.fill_texture_id() != TextureId::default() => {
                out.push((r.rect, r.fill, r.fill_texture_id()));
            }
            _ => {}
        }
    }
    out
}

fn toggle_rect(h: &Harness<'_, State>) -> Rect {
    h.get_by_label("Visible: Walls").rect()
}

#[test]
fn a_visible_tag_shows_the_eye_in_the_text_color_inside_the_toggle() {
    let h = harness(true);
    let eye = texture_of(&h, Icon::Eye);
    let want = h.ctx.style_of(h.ctx.theme()).visuals.text_color();
    let rect = toggle_rect(&h);
    assert!((rect.width() - 20.0).abs() < 0.5 && (rect.height() - 20.0).abs() < 0.5);
    let found = quads(&shapes(&h))
        .into_iter()
        .find(|(r, _, t)| *t == eye && rect.expand(1.0).contains_rect(*r))
        .expect("the Eye icon is drawn inside the toggle");
    assert_eq!(found.1, want);
}

#[test]
fn a_hidden_tag_shows_eye_off_in_the_weak_text_color() {
    let h = harness(false);
    let off = texture_of(&h, Icon::EyeOff);
    let eye = texture_of(&h, Icon::Eye);
    let want = h.ctx.style_of(h.ctx.theme()).visuals.weak_text_color();
    let rect = toggle_rect(&h);
    let quads = quads(&shapes(&h));
    let found = quads
        .iter()
        .find(|(r, _, t)| *t == off && rect.expand(1.0).contains_rect(*r))
        .expect("the EyeOff icon is drawn inside the toggle");
    assert_eq!(found.1, want);
    assert!(
        quads.iter().all(|(_, _, t)| *t != eye),
        "no Eye icon while hidden"
    );
}

#[test]
fn the_toggle_paints_no_circle_or_strike_of_its_own() {
    for visible in [true, false] {
        let h = harness(visible);
        let rect = toggle_rect(&h).expand(1.0);
        for shape in shapes(&h) {
            match shape {
                Shape::Circle(c) => assert!(!rect.contains(c.center), "circle in toggle"),
                Shape::LineSegment { points, .. } => {
                    assert!(!rect.contains(points[0]), "line in toggle")
                }
                _ => {}
            }
        }
    }
}

#[test]
fn clicking_toggles_and_swaps_the_icon() {
    let mut h = harness(true);
    let (eye, off) = (texture_of(&h, Icon::Eye), texture_of(&h, Icon::EyeOff));
    h.get_by_label("Visible: Walls").click();
    h.run();
    h.run();
    let after_hide = quads(&shapes(&h));
    assert!(after_hide.iter().any(|q| q.2 == off));
    assert!(after_hide.iter().all(|q| q.2 != eye));
    h.get_by_label("Visible: Walls").click();
    h.run();
    h.run();
    let after_show = quads(&shapes(&h));
    assert!(after_show.iter().any(|q| q.2 == eye));
    assert!(after_show.iter().all(|q| q.2 != off));
}

#[test]
fn the_icons_exist_with_their_brand_names() {
    assert_eq!(icons::name(Icon::Eye), "eye");
    assert_eq!(icons::name(Icon::EyeOff), "eye-off");
    assert!(icons::all().contains(&Icon::Eye) && icons::all().contains(&Icon::EyeOff));
}
