// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m10-brand-icons.md`: the 24 brand icons from
//! `design/brand/icons/`, rasterised by egui_extras and tinted by the theme; the active
//! tool's marker.

use drydock::icons::{self, ACTIVE_BAR_HEIGHT, BUTTON_SIZE, Icon};
use drydock::input::Tool;
use drydock::look::{self, Theme};
use eframe::egui::{self, Color32, Pos2, Rect, Shape, TextureId, load::SizeHint, vec2};
use egui_kittest::Harness;

const NAMES: [&str; 24] = [
    "select",
    "line",
    "rectangle",
    "circle",
    "arc",
    "arc3",
    "pushpull",
    "offset",
    "followme",
    "move",
    "rotate",
    "scale",
    "tape",
    "erase",
    "paint",
    "orbit",
    "pan",
    "zoom",
    "list",
    "tiles",
    "new",
    "edit",
    "purge",
    "eyedropper",
];

const THEMES: [Theme; 2] = [Theme::Light, Theme::Dark];

#[test]
fn every_icon_has_the_readme_name_in_order() {
    let all = icons::all();
    assert_eq!(all.len(), 24);
    for (icon, want) in all.iter().zip(NAMES) {
        assert_eq!(icons::name(*icon), want, "{icon:?}");
    }
}

#[test]
fn svg_is_the_brand_file_byte_for_byte() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../design/brand/icons");
    for icon in icons::all() {
        let file = dir.join(format!("{}.svg", icons::name(icon)));
        let want = std::fs::read(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        assert_eq!(icons::svg(icon), &want[..], "{icon:?}");
    }
}

#[test]
fn every_svg_parses_with_a_20_by_20_view_box() {
    for icon in icons::all() {
        let tree = usvg::Tree::from_data(icons::svg(icon), &usvg::Options::default())
            .unwrap_or_else(|e| panic!("{icon:?} does not parse: {e}"));
        assert_eq!(tree.size().width(), 20.0, "{icon:?}");
        assert_eq!(tree.size().height(), 20.0, "{icon:?}");
    }
}

#[test]
fn uris_are_distinct_svg_names() {
    let mut uris: Vec<String> = icons::all().into_iter().map(icons::uri).collect();
    assert!(uris.iter().all(|u| u.ends_with(".svg")));
    uris.sort();
    uris.dedup();
    assert_eq!(uris.len(), 24);
}

#[test]
fn install_registers_every_icon_and_can_run_twice() {
    let ctx = egui::Context::default();
    icons::install(&ctx);
    icons::install(&ctx);
    for icon in icons::all() {
        let poll = ctx
            .try_load_bytes(&icons::uri(icon))
            .unwrap_or_else(|e| panic!("{icon:?}: {e}"));
        match poll {
            egui::load::BytesPoll::Ready { bytes, .. } => {
                assert_eq!(&bytes[..], icons::svg(icon), "{icon:?}");
            }
            egui::load::BytesPoll::Pending { .. } => panic!("{icon:?} pending"),
        }
    }
}

/// A textured quad the UI drew: its bounds, tint and texture.
struct Quad {
    rect: Rect,
    tint: Color32,
    texture: TextureId,
}

fn flatten<'a>(shape: &'a Shape, out: &mut Vec<&'a Shape>) {
    if let Shape::Vec(v) = shape {
        for s in v {
            flatten(s, out);
        }
    } else {
        out.push(shape);
    }
}

fn shapes(h: &Harness<'_>) -> Vec<Shape> {
    let mut all = Vec::new();
    for clipped in &h.output().shapes {
        let mut flat = Vec::new();
        flatten(&clipped.shape, &mut flat);
        all.extend(flat.into_iter().cloned());
    }
    all
}

/// Textured quads other than the font atlas.
fn quads(shapes: &[Shape]) -> Vec<Quad> {
    let mut out = Vec::new();
    for shape in shapes {
        match shape {
            Shape::Mesh(mesh) if mesh.texture_id != TextureId::Managed(0) => {
                if let Some(first) = mesh.vertices.first() {
                    let mut rect = Rect::NOTHING;
                    for v in &mesh.vertices {
                        rect.extend_with(v.pos);
                    }
                    out.push(Quad {
                        rect,
                        tint: first.color,
                        texture: mesh.texture_id,
                    });
                }
            }
            Shape::Rect(r) if r.fill_texture_id() != TextureId::default() => {
                out.push(Quad {
                    rect: r.rect,
                    tint: r.fill,
                    texture: r.fill_texture_id(),
                });
            }
            _ => {}
        }
    }
    out
}

/// Filled rectangles of exactly `color`.
fn fills(shapes: &[Shape], color: Color32) -> Vec<Rect> {
    shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Rect(r) if r.fill == color && r.fill_texture_id() == TextureId::default() => {
                Some(r.rect)
            }
            _ => None,
        })
        .collect()
}

struct Drawn {
    h: Harness<'static>,
    button: std::rc::Rc<std::cell::Cell<Rect>>,
}

fn draw(icon: Icon, selected: bool, theme: Theme, ppp: f32) -> Drawn {
    let button = std::rc::Rc::new(std::cell::Cell::new(Rect::NOTHING));
    let seen = button.clone();
    let mut h = Harness::builder()
        .with_size(vec2(120.0, 80.0))
        .with_pixels_per_point(ppp)
        .build_ui(move |ui| {
            let r = icons::icon_button(ui, icon, "Icon", "Tip", selected);
            seen.set(r.rect);
        });
    let egui_theme = match theme {
        Theme::Light => egui::Theme::Light,
        Theme::Dark => egui::Theme::Dark,
    };
    h.ctx.set_style_of(egui_theme, look::style(theme));
    h.ctx.set_theme(egui::ThemePreference::from(egui_theme));
    h.run();
    h.run();
    Drawn { h, button }
}

#[test]
fn each_icon_rasterises_at_the_real_pixel_size_with_ink_at_1x_and_2x_in_both_themes() {
    for theme in THEMES {
        for ppp in [1.0f32, 2.0] {
            for icon in icons::all() {
                let d = draw(icon, false, theme, ppp);
                let shapes = shapes(&d.h);
                let button = d.button.get();
                let inner = button.shrink(4.0);
                let q = quads(&shapes)
                    .into_iter()
                    .find(|q| q.rect.center().distance(inner.center()) < 0.6)
                    .unwrap_or_else(|| panic!("{icon:?} {theme:?} {ppp}x: no image drawn"));
                assert!(
                    (q.rect.width() - 20.0).abs() < 0.6 && (q.rect.height() - 20.0).abs() < 0.6,
                    "{icon:?}: drawn {:?}",
                    q.rect.size()
                );
                assert_eq!(
                    q.tint,
                    look::palette(theme).ink,
                    "{icon:?} {theme:?}: tint is the theme's ink"
                );
                // The texture holds 20 x 20 points at the real pixel size.
                let want = (20.0 * ppp) as usize;
                let size = d.h.ctx.tex_manager().read().meta(q.texture).map(|m| m.size);
                assert_eq!(size, Some([want, want]), "{icon:?} {ppp}x texture size");
                // And it has ink.
                let hint = SizeHint::Size {
                    width: want as u32,
                    height: want as u32,
                    maintain_aspect_ratio: true,
                };
                let loaded = d.h.ctx.try_load_image(&icons::uri(icon), hint);
                let Ok(egui::load::ImagePoll::Ready { image }) = loaded else {
                    panic!("{icon:?} {ppp}x not ready");
                };
                let inked = image.pixels.iter().filter(|p| p.a() > 0).count();
                assert!(inked >= 20, "{icon:?} {ppp}x has {inked} inked pixels");
            }
        }
    }
}

#[test]
fn button_stays_button_size() {
    let d = draw(Icon::Tool(Tool::Line), true, Theme::Dark, 1.0);
    assert_eq!(d.button.get().size(), vec2(BUTTON_SIZE, BUTTON_SIZE));
}

#[test]
fn active_tool_has_accent_icon_accent_soft_fill_and_the_orange_bar() {
    for theme in THEMES {
        let p = look::palette(theme);
        let d = draw(Icon::Tool(Tool::PushPull), true, theme, 1.0);
        let button = d.button.get();
        let shapes = shapes(&d.h);

        let soft = fills(&shapes, p.accent_soft);
        assert!(
            soft.iter()
                .any(|r| button.contains_rect(r.expand(0.01)) && r.width() > 20.0),
            "{theme:?}: accent-soft fill behind the button"
        );
        let inner = button.shrink(4.0);
        let icon = quads(&shapes)
            .into_iter()
            .find(|q| q.rect.center().distance(inner.center()) < 0.6)
            .expect("icon drawn");
        assert_eq!(icon.tint, p.accent, "{theme:?}: active icon is accent");

        let bars = fills(&shapes, p.product);
        assert_eq!(bars.len(), 1, "{theme:?}: exactly one orange bar");
        let bar = bars[0];
        assert!((bar.height() - ACTIVE_BAR_HEIGHT).abs() < 0.01, "{bar:?}");
        assert!(
            (bar.min.x - button.min.x).abs() < 0.6,
            "{bar:?} vs {button:?}"
        );
        assert!(
            (bar.max.x - button.max.x).abs() < 0.6,
            "{bar:?} vs {button:?}"
        );
        assert!(
            (bar.max.y - button.max.y).abs() < 0.6,
            "{bar:?} vs {button:?}"
        );
    }
}

#[test]
fn inactive_tool_has_no_fill_and_no_bar() {
    for theme in THEMES {
        let p = look::palette(theme);
        let d = draw(Icon::Tool(Tool::PushPull), false, theme, 1.0);
        let shapes = shapes(&d.h);
        assert!(fills(&shapes, p.product).is_empty(), "{theme:?}: no bar");
        let button = d.button.get();
        assert!(
            fills(&shapes, p.accent_soft)
                .iter()
                .all(|r| !button.intersects(*r)),
            "{theme:?}: no accent-soft fill"
        );
    }
}

#[test]
fn orange_is_only_for_tools_a_selected_panel_icon_gets_no_bar() {
    let p = look::palette(Theme::Light);
    let d = draw(Icon::List, true, Theme::Light, 1.0);
    let shapes = shapes(&d.h);
    assert!(fills(&shapes, p.product).is_empty());
    assert!(
        !fills(&shapes, p.accent_soft).is_empty(),
        "still accent-soft"
    );
    let inner = d.button.get().shrink(4.0);
    let icon = quads(&shapes)
        .into_iter()
        .find(|q| q.rect.center().distance(inner.center()) < 0.6)
        .expect("icon drawn");
    assert_eq!(icon.tint, p.accent);
}

#[test]
fn paint_draws_the_icon_in_the_given_rect_and_color() {
    let rect = Rect::from_min_size(Pos2::new(10.0, 10.0), vec2(24.0, 24.0));
    let color = Color32::from_rgb(200, 30, 30);
    let mut h = Harness::builder()
        .with_size(vec2(80.0, 60.0))
        .build_ui(move |ui| {
            icons::paint(ui.painter(), rect, Icon::Eyedropper, color);
        });
    h.run();
    h.run();
    let shapes = shapes(&h);
    let q = quads(&shapes)
        .into_iter()
        .find(|q| q.tint == color)
        .expect("icon painted");
    assert!(q.rect.center().distance(rect.center()) < 0.6);
    assert!((q.rect.width() - 24.0).abs() < 0.6);
}
