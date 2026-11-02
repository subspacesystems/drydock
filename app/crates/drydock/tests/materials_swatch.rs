// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-material-swatch-texture: textured materials show their image in the Materials list
//! and tiles. See docs/specs/m4-material-swatch-texture.md.

use std::path::PathBuf;

use drydock::materials_panel::{MaterialAction, MaterialsPanel, materials_panel};
use drydock::swatches::SwatchState;
use drydock_kernel::model::{Material, MaterialId, Model, ModelEdit, Texture};
use eframe::egui;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glam::DVec2;
use image::{Rgba, RgbaImage};

const ROBOTO: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::empty();
    fonts.font_data.insert(
        "Roboto".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(ROBOTO)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.insert(family, vec!["Roboto".to_owned()]);
    }
    ctx.set_fonts(fonts);
}

fn temp_dir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "modeler-swatch-panel-test-{}-{test}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_png(dir: &std::path::Path, name: &str) -> String {
    let path = dir.join(name);
    RgbaImage::from_pixel(32, 32, Rgba([200, 60, 30, 255]))
        .save(&path)
        .unwrap();
    path.to_string_lossy().into_owned()
}

struct State {
    panel: MaterialsPanel,
    model: Model,
    actions: Vec<MaterialAction>,
}

/// Materials named `name`, textured with the image at the path when one is given.
fn model_with(materials: &[(&str, Option<&str>)]) -> (Model, Vec<MaterialId>) {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let ids = materials
        .iter()
        .map(|(name, texture)| {
            let mut material = Material::new(name, [0.5, 0.4, 0.3, 1.0]);
            material.texture = texture.map(|path| Texture::new(path, DVec2::new(1.0, 1.0)));
            model.add_material(material, &mut log)
        })
        .collect();
    (model, ids)
}

fn harness(model: Model, tiles: bool, search: &str) -> Harness<'static, State> {
    let mut panel = MaterialsPanel::default();
    panel.tiles = tiles;
    panel.search = search.to_owned();
    let mut h = Harness::builder()
        .with_size(egui::vec2(600.0, 800.0))
        .build_ui_state(
            |ui, state: &mut State| {
                ui.allocate_ui(egui::vec2(160.0, 400.0), |ui| {
                    ui.set_max_width(160.0);
                    if let Some(action) = materials_panel(ui, &mut state.panel, &state.model, None)
                    {
                        state.actions.push(action);
                    }
                });
            },
            State {
                panel,
                model,
                actions: Vec::new(),
            },
        );
    install_fonts(&h.ctx);
    h.run();
    h
}

/// Draw, let every started load finish, draw again.
fn settle(h: &mut Harness<'static, State>) {
    h.run();
    h.state_mut().panel.swatches.wait();
    h.run();
}

fn flatten<'a>(shape: &'a egui::Shape, out: &mut Vec<&'a egui::Shape>) {
    match shape {
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| flatten(s, out)),
        other => out.push(other),
    }
}

/// Bounding rects of the shapes of the last frame drawn with `texture`.
fn drawn_with(h: &Harness<'static, State>, texture: egui::TextureId) -> Vec<egui::Rect> {
    let mut shapes = Vec::new();
    for clipped in &h.output().shapes {
        flatten(&clipped.shape, &mut shapes);
    }
    shapes
        .into_iter()
        .filter(|shape| shape.texture_id() == texture)
        .map(|shape| shape.visual_bounding_rect())
        .collect()
}

/// Every texture used by the last frame apart from the font atlas.
fn image_textures(h: &Harness<'static, State>) -> Vec<egui::TextureId> {
    let mut shapes = Vec::new();
    for clipped in &h.output().shapes {
        flatten(&clipped.shape, &mut shapes);
    }
    shapes
        .into_iter()
        .map(egui::Shape::texture_id)
        .filter(|id| *id != egui::TextureId::default())
        .collect()
}

#[test]
fn a_list_row_shows_the_image_in_its_swatch() {
    let dir = temp_dir("row");
    let path = write_png(&dir, "brick.png");
    let (model, _) = model_with(&[("Brick", Some(&path))]);
    let mut h = harness(model, false, "");
    settle(&mut h);
    let ctx = h.ctx.clone();
    let id = h
        .state_mut()
        .panel
        .swatches
        .get(&ctx, &path)
        .expect("preview ready");
    let rects = drawn_with(&h, id);
    assert_eq!(rects.len(), 1, "{rects:?}");
    let swatch = rects[0];
    assert!((swatch.width() - 14.0).abs() < 0.5, "{swatch:?}");
    assert!((swatch.height() - 14.0).abs() < 0.5, "{swatch:?}");
    let label = h.get_by_label("Brick").rect();
    assert!(swatch.right() <= label.left() + 0.5, "{swatch:?} {label:?}");
    let middle = label.center().y;
    assert!(
        swatch.top() <= middle && middle <= swatch.bottom(),
        "{swatch:?} {label:?}"
    );
}

#[test]
fn a_tile_shows_the_image_above_its_name() {
    let dir = temp_dir("tile");
    let path = write_png(&dir, "oak.png");
    let (model, _) = model_with(&[("Oak", Some(&path))]);
    let mut h = harness(model, true, "");
    settle(&mut h);
    let ctx = h.ctx.clone();
    let id = h
        .state_mut()
        .panel
        .swatches
        .get(&ctx, &path)
        .expect("preview ready");
    let rects = drawn_with(&h, id);
    assert_eq!(rects.len(), 1, "{rects:?}");
    let swatch = rects[0];
    // m6-eyedropper-2: the tile is one widget (swatch and name) and the swatch may be
    // inset a little so the selection box shows around it.
    assert!((52.0..=64.5).contains(&swatch.width()), "{swatch:?}");
    assert!((32.0..=40.5).contains(&swatch.height()), "{swatch:?}");
    let tile = h.get_by_label("Oak").rect();
    assert!(tile.contains_rect(swatch), "{swatch:?} {tile:?}");
    assert!(
        swatch.bottom() <= tile.bottom() - 16.0,
        "the name goes under the image: {swatch:?} {tile:?}"
    );
}

#[test]
fn materials_without_a_readable_image_keep_the_color_swatch() {
    let dir = temp_dir("plain");
    let missing = dir.join("missing.png").to_string_lossy().into_owned();
    let (model, _) = model_with(&[("Paint", None), ("Lost", Some(&missing))]);
    let mut h = harness(model, false, "");
    settle(&mut h);
    assert!(image_textures(&h).is_empty(), "{:?}", image_textures(&h));
    assert!(matches!(
        h.state_mut().panel.swatches.state(&missing),
        SwatchState::Failed(_)
    ));
    h.get_by_label("Paint");
    h.get_by_label("Lost");
}

#[test]
fn hidden_materials_load_nothing() {
    let dir = temp_dir("hidden");
    let brick = write_png(&dir, "brick.png");
    let oak = write_png(&dir, "oak.png");
    let (model, _) = model_with(&[("Brick", Some(&brick)), ("Oak", Some(&oak))]);
    let mut h = harness(model, false, "oak");
    settle(&mut h);
    assert_eq!(
        h.state_mut().panel.swatches.state(&brick),
        SwatchState::Unknown
    );
    assert!(matches!(
        h.state_mut().panel.swatches.state(&oak),
        SwatchState::Ready { .. }
    ));
    assert_eq!(image_textures(&h).len(), 1);
}

#[test]
fn clicking_a_textured_swatch_still_chooses_the_material() {
    let dir = temp_dir("click");
    let path = write_png(&dir, "brick.png");
    let (model, ids) = model_with(&[("Brick", Some(&path))]);
    let mut h = harness(model, false, "");
    settle(&mut h);
    let ctx = h.ctx.clone();
    let id = h.state_mut().panel.swatches.get(&ctx, &path).unwrap();
    let swatch = drawn_with(&h, id)[0];
    h.get_by_label("Brick"); // still listed by name
    h.event(egui::Event::PointerMoved(swatch.center()));
    h.event(egui::Event::PointerButton {
        pos: swatch.center(),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.event(egui::Event::PointerButton {
        pos: swatch.center(),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run();
    assert_eq!(
        h.state().actions.last(),
        Some(&MaterialAction::Choose(Some(ids[0])))
    );
}
