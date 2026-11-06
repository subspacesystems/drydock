// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m10-brand-about.md`: the About window shows the brand
//! lockup for the current theme, the version and the product line.

use drydock::chrome::{
    LOCKUP_WIDTH, PRODUCT_LINE, about_lines, about_text, about_window, install_lockups, lockup_svg,
    lockup_uri,
};
use drydock::look::{self, Theme};
use eframe::egui::{self, Color32, Rect, Shape, TextureId, load::SizeHint, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

const THEMES: [Theme; 2] = [Theme::Light, Theme::Dark];

fn repo_file(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../design/brand/logo")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn dark_lockup_is_the_brand_file() {
    assert_eq!(
        lockup_svg(true),
        &repo_file("drydock-lockup-h-dark.svg")[..]
    );
}

#[test]
fn light_lockup_is_the_committed_file_and_the_colour_swap_of_the_dark_one() {
    let light = lockup_svg(false);
    assert_eq!(light, &repo_file("drydock-lockup-h-light.svg")[..]);
    let dark = String::from_utf8(repo_file("drydock-lockup-h-dark.svg")).unwrap();
    let swapped = dark
        .replace("#EEF7FF", "#091126")
        .replace("#1FD5FF", "#091126");
    assert_eq!(std::str::from_utf8(light).unwrap(), swapped);
    assert!(swapped.contains("#FF8A3D"), "the orange stays");
    assert!(!swapped.contains("#EEF7FF") && !swapped.contains("#1FD5FF"));
}

#[test]
fn uris_differ_and_end_in_svg() {
    assert_ne!(lockup_uri(true), lockup_uri(false));
    assert!(lockup_uri(true).ends_with(".svg") && lockup_uri(false).ends_with(".svg"));
}

#[test]
fn install_registers_both_lockups_and_can_run_twice() {
    let ctx = egui::Context::default();
    install_lockups(&ctx);
    install_lockups(&ctx);
    for dark in [false, true] {
        match ctx.try_load_bytes(lockup_uri(dark)) {
            Ok(egui::load::BytesPoll::Ready { bytes, .. }) => {
                assert_eq!(&bytes[..], lockup_svg(dark), "dark={dark}");
            }
            Err(e) => panic!("dark={dark}: {e}"),
            Ok(_) => panic!("dark={dark}: bytes not ready"),
        }
    }
}

#[test]
fn about_lines_have_version_then_product_line() {
    let lines = about_lines();
    assert_eq!(lines[0], about_text());
    assert_eq!(lines[1], PRODUCT_LINE);
    assert_eq!(PRODUCT_LINE, "A Subspace Systems product.");
}

fn flatten(shape: &Shape, out: &mut Vec<Shape>) {
    if let Shape::Vec(v) = shape {
        for s in v {
            flatten(s, out);
        }
    } else {
        out.push(shape.clone());
    }
}

/// Textured quads other than the font atlas: (rect, tint, texture).
fn quads(h: &Harness<'_>) -> Vec<(Rect, Color32, TextureId)> {
    let mut all = Vec::new();
    for clipped in &h.output().shapes {
        flatten(&clipped.shape, &mut all);
    }
    let mut out = Vec::new();
    for shape in &all {
        match shape {
            Shape::Mesh(mesh) if mesh.texture_id != TextureId::Managed(0) => {
                if let Some(first) = mesh.vertices.first() {
                    let mut rect = Rect::NOTHING;
                    for v in &mesh.vertices {
                        rect.extend_with(v.pos);
                    }
                    out.push((rect, first.color, mesh.texture_id));
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

fn open_about(theme: Theme, ppp: f32) -> Harness<'static> {
    let mut h = Harness::builder()
        .with_size(vec2(700.0, 700.0))
        .with_pixels_per_point(ppp)
        .build_ui(|ui| {
            let mut open = true;
            about_window(ui.ctx(), &mut open);
        });
    let t = match theme {
        Theme::Light => egui::Theme::Light,
        Theme::Dark => egui::Theme::Dark,
    };
    h.ctx.set_style_of(t, look::style(theme));
    h.ctx.set_theme(egui::ThemePreference::from(t));
    h.run();
    h.run();
    h.run();
    h
}

/// The texture ids the loader holds for the lockup of the given theme at the real pixel
/// width: for the size hints a plain exact-size or width-only draw produces.
fn textures_of(h: &Harness<'_>, dark: bool, ppp: f32) -> Vec<TextureId> {
    let w = (LOCKUP_WIDTH * ppp).round() as u32;
    let ph = (w as f32 * 748.0 / 3410.0).round() as u32;
    let hints = [
        SizeHint::Width(w),
        SizeHint::Size {
            width: w,
            height: ph,
            maintain_aspect_ratio: true,
        },
        SizeHint::Size {
            width: w,
            height: ph,
            maintain_aspect_ratio: false,
        },
    ];
    hints
        .into_iter()
        .filter_map(|hint| {
            match h
                .ctx
                .try_load_texture(lockup_uri(dark), Default::default(), hint)
            {
                Ok(egui::load::TexturePoll::Ready { texture }) => Some(texture.id),
                _ => None,
            }
        })
        .collect()
}

fn close(a: u8, b: u8) -> bool {
    a.abs_diff(b) <= 6
}

/// Opaque pixels of the texture the lockup was drawn with, as (near light #EEF7FF count,
/// near navy #091126 count, near orange #FF8A3D count).
fn colour_counts(h: &Harness<'_>, dark: bool, ppp: f32) -> (usize, usize, usize) {
    let w = (LOCKUP_WIDTH * ppp).round() as u32;
    let hint = SizeHint::Width(w);
    let Ok(egui::load::ImagePoll::Ready { image }) = h.ctx.try_load_image(lockup_uri(dark), hint)
    else {
        panic!("lockup not loaded at the drawn size");
    };
    let (mut light, mut navy, mut orange) = (0, 0, 0);
    for p in &image.pixels {
        if p.a() < 250 {
            continue;
        }
        let (r, g, b) = (p.r(), p.g(), p.b());
        if close(r, 0xEE) && close(g, 0xF7) && close(b, 0xFF) {
            light += 1;
        }
        if close(r, 0x09) && close(g, 0x11) && close(b, 0x26) {
            navy += 1;
        }
        if close(r, 0xFF) && close(g, 0x8A) && close(b, 0x3D) {
            orange += 1;
        }
    }
    (light, navy, orange)
}

#[test]
fn about_shows_the_lockup_version_and_product_line_in_both_themes_at_1x_and_2x() {
    for theme in THEMES {
        for ppp in [1.0f32, 2.0] {
            let h = open_about(theme, ppp);
            let dark = theme == Theme::Dark;
            let ctx = format!("{theme:?} {ppp}x");

            assert!(h.query_by_label("About Drydock").is_some(), "{ctx}: title");
            assert!(h.query_by_label(&about_text()).is_some(), "{ctx}: version");
            assert!(
                h.query_by_label(PRODUCT_LINE).is_some(),
                "{ctx}: product line"
            );
            assert!(h.query_by_label("Close").is_some(), "{ctx}: close");

            let lockups: Vec<_> = quads(&h)
                .into_iter()
                .filter(|(r, _, _)| (r.width() - LOCKUP_WIDTH).abs() < 0.6)
                .collect();
            assert_eq!(lockups.len(), 1, "{ctx}: one lockup image");
            let (rect, tint, texture) = lockups[0];
            let want_h = LOCKUP_WIDTH * 748.0 / 3410.0;
            assert!(
                (rect.height() - want_h).abs() < 1.0,
                "{ctx}: {:?}",
                rect.size()
            );
            assert_eq!(tint, Color32::WHITE, "{ctx}: colours are baked in, no tint");

            assert!(
                textures_of(&h, dark, ppp).contains(&texture),
                "{ctx}: the drawn texture is the theme's lockup"
            );
            assert!(
                !textures_of(&h, !dark, ppp).contains(&texture),
                "{ctx}: not the other theme's lockup"
            );

            // Crisp: the texture has the real pixel width.
            let size = h.ctx.tex_manager().read().meta(texture).map(|m| m.size);
            let w = (LOCKUP_WIDTH * ppp).round() as usize;
            let size = size.unwrap_or_else(|| panic!("{ctx}: no texture meta"));
            assert_eq!(size[0], w, "{ctx}: texture width");
            let want_ph = (w as f32 * 748.0 / 3410.0).round() as usize;
            assert!(
                size[1].abs_diff(want_ph) <= 1,
                "{ctx}: texture height {size:?}"
            );

            // The theme's lockup: light ink on dark, navy ink on light; orange in both.
            let (light, navy, orange) = colour_counts(&h, dark, ppp);
            assert!(orange > 50, "{ctx}: orange mark");
            if dark {
                assert!(
                    light > 500 && navy == 0,
                    "{ctx}: dark lockup {light} {navy}"
                );
            } else {
                assert!(
                    navy > 500 && light == 0,
                    "{ctx}: light lockup {light} {navy}"
                );
            }
        }
    }
}

#[test]
fn the_lockup_is_inside_the_window_above_the_version() {
    let h = open_about(Theme::Dark, 1.0);
    let lockup = quads(&h)
        .into_iter()
        .find(|(r, _, _)| (r.width() - LOCKUP_WIDTH).abs() < 0.6)
        .expect("lockup")
        .0;
    let version = h.get_by_label(&about_text()).rect();
    assert!(
        lockup.bottom() <= version.top() + 1.0,
        "{lockup:?} vs {version:?}"
    );
}

#[test]
fn switching_theme_while_open_switches_the_lockup() {
    let mut h = open_about(Theme::Dark, 1.0);
    h.ctx
        .set_style_of(egui::Theme::Light, look::style(Theme::Light));
    h.ctx.set_theme(egui::ThemePreference::Light);
    h.run();
    h.run();
    h.run();
    let drawn: Vec<_> = quads(&h)
        .into_iter()
        .filter(|(r, _, _)| (r.width() - LOCKUP_WIDTH).abs() < 0.6)
        .collect();
    assert_eq!(drawn.len(), 1);
    assert!(textures_of(&h, false, 1.0).contains(&drawn[0].2));
    assert!(!textures_of(&h, true, 1.0).contains(&drawn[0].2));
    let (light, navy, _) = colour_counts(&h, false, 1.0);
    assert!(navy > 500 && light == 0);
}
