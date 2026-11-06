// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-brand-numbers: the Measurements box and the Entity Info values draw their numbers
//! in the monospace family (Roboto Mono once `m10-brand-fonts` is in), always with units;
//! labels stay proportional; the Measurements box has corners 2 left, 8 right.
//! See docs/specs/m10-brand-numbers.md.

use drydock::chrome::{measurements_corners, status_measurement};
use drydock::entity_info::{EntityInfoPanel, entity_info_panel};
use drydock_kernel::model::*;
use drydock_kernel::select::Entity;
use drydock_kernel::units::{LengthFormat, Unit, format_with};
use eframe::egui::{self, CornerRadius, FontFamily, Shape};
use glam::DVec3;

/// Every text drawn in `shape` as (text, family of its first section).
fn collect(shape: &Shape, out: &mut Vec<(String, FontFamily)>, rects: &mut Vec<CornerRadius>) {
    match shape {
        Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, out, rects)),
        Shape::Text(t) => {
            let family = t.galley.job.sections[0].format.font_id.family.clone();
            out.push((t.galley.text().to_string(), family));
        }
        Shape::Rect(r) => rects.push(r.corner_radius),
        _ => {}
    }
}

/// Run one frame of `add` in a central panel; return the texts with their families and
/// the corner radii of every rectangle drawn.
fn draw(add: impl FnMut(&mut egui::Ui)) -> (Vec<(String, FontFamily)>, Vec<CornerRadius>) {
    let ctx = egui::Context::default();
    let mut add = add;
    let mut output = None;
    // Two passes so layout settles.
    for _ in 0..2 {
        let mut full = ctx.run_ui(egui::RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| add(ui));
        });
        full.textures_delta.clear();
        output = Some(full);
    }
    let (mut texts, mut rects) = (Vec::new(), Vec::new());
    for clipped in &output.unwrap().shapes {
        collect(&clipped.shape, &mut texts, &mut rects);
    }
    (texts, rects)
}

fn family_of(texts: &[(String, FontFamily)], text: &str) -> FontFamily {
    texts
        .iter()
        .find(|(t, _)| t == text)
        .unwrap_or_else(|| panic!("no text {text:?} in {texts:?}"))
        .1
        .clone()
}

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn square(model: &mut Model) {
    let pts = [
        v(0.0, 0.0, 0.0),
        v(3.0, 0.0, 0.0),
        v(3.0, 4.0, 0.0),
        v(0.0, 4.0, 0.0),
    ];
    let mut log = ModelEdit::new();
    model
        .edit_geometry(BodyId::Root, &mut log, |ctx, edit| {
            for i in 0..4 {
                ctx.add_edge(pts[i], pts[(i + 1) % 4], edit).unwrap();
            }
        })
        .unwrap();
}

fn selection(model: &Model, faces: bool) -> Vec<Entity> {
    let g = model.body(BodyId::Root).unwrap().geometry();
    if faces {
        g.faces().map(|(id, _)| Entity::Face(id)).collect()
    } else {
        g.edges().map(|(id, _)| Entity::Edge(id)).collect()
    }
}

fn panel_texts(unit: Unit, faces: bool) -> Vec<(String, FontFamily)> {
    let mut model = Model::new();
    square(&mut model);
    model
        .set_length_format(
            LengthFormat::Decimal { unit, decimals: 2 },
            &mut ModelEdit::new(),
        )
        .unwrap();
    let sel = selection(&model, faces);
    let mut panel = EntityInfoPanel::default();
    draw(|ui| {
        entity_info_panel(ui, &mut panel, &model, BodyId::Root, &sel);
    })
    .0
}

// ---------------------------------------------------------------------------------------
// The Measurements box

#[test]
fn measurements_corners_are_2_left_and_8_right() {
    assert_eq!(
        measurements_corners(),
        CornerRadius {
            nw: 2,
            sw: 2,
            ne: 8,
            se: 8
        }
    );
}

#[test]
fn the_box_is_drawn_with_those_corners() {
    let (_, rects) = draw(|ui| status_measurement(ui, Some(("Length", "1.5m")), ""));
    assert!(
        rects.contains(&measurements_corners()),
        "no rectangle with the box corners among {rects:?}"
    );
}

#[test]
fn nothing_to_show_draws_no_box() {
    let (texts, rects) = draw(|ui| status_measurement(ui, None, ""));
    assert!(texts.is_empty());
    assert!(!rects.contains(&measurements_corners()));
}

#[test]
fn value_and_typed_text_are_monospace_and_the_label_is_not() {
    let (texts, _) = draw(|ui| status_measurement(ui, Some(("Length", "1.5m")), "2.25m"));
    assert_eq!(family_of(&texts, "Length"), FontFamily::Proportional);
    assert_eq!(family_of(&texts, "1.5m"), FontFamily::Monospace);
    assert_eq!(family_of(&texts, "2.25m"), FontFamily::Monospace);
}

#[test]
fn every_measured_length_carries_its_unit() {
    let units = [
        (Unit::Millimeter, "mm"),
        (Unit::Centimeter, "cm"),
        (Unit::Meter, "m"),
        (Unit::Inch, "\""),
        (Unit::Foot, "'"),
    ];
    for (unit, suffix) in units {
        let text = format_with(1.2345, LengthFormat::Decimal { unit, decimals: 2 });
        assert!(text.ends_with(suffix), "{text:?} lacks {suffix:?}");
    }
}

// ---------------------------------------------------------------------------------------
// Entity Info

#[test]
fn edge_values_are_monospace_with_units_and_labels_are_not() {
    let texts = panel_texts(Unit::Meter, false);
    for label in ["Length", "Width", "Depth", "Height"] {
        assert_eq!(
            family_of(&texts, label),
            FontFamily::Proportional,
            "{label}"
        );
    }
    for value in ["14m", "3m", "4m", "0m"] {
        assert_eq!(family_of(&texts, value), FontFamily::Monospace, "{value}");
    }
}

#[test]
fn area_is_monospace_with_a_squared_unit() {
    let texts = panel_texts(Unit::Meter, true);
    assert_eq!(family_of(&texts, "Area"), FontFamily::Proportional);
    assert_eq!(family_of(&texts, "12m²"), FontFamily::Monospace);
    let texts = panel_texts(Unit::Millimeter, true);
    assert_eq!(family_of(&texts, "12000000mm²"), FontFamily::Monospace);
}

#[test]
fn words_stay_proportional() {
    let texts = panel_texts(Unit::Meter, true);
    // "Default" is a material name, not a number.
    assert_eq!(family_of(&texts, "Default"), FontFamily::Proportional);
    assert_eq!(family_of(&texts, "Front"), FontFamily::Proportional);
}

#[test]
fn dimensions_follow_the_model_unit() {
    let texts = panel_texts(Unit::Centimeter, false);
    assert_eq!(family_of(&texts, "300cm"), FontFamily::Monospace);
    assert_eq!(family_of(&texts, "400cm"), FontFamily::Monospace);
}
