// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m7-print-setup: bed presets and outline, the fits-the-bed check, the settings file, the
//! overhang angle in the view state, Drop to Bed and the Tools menu item
//! (`docs/specs/m7-print-setup.md`).

mod print_setup_common;

use std::cell::RefCell;
use std::ffi::OsString;
use std::rc::Rc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use glam::{DMat4, DVec3};
use modeler::chrome::{MenuState, menu_bar};
use modeler::editor::Editor;
use modeler::input::{Action, EditKey};
use modeler::print_setup::*;
use modeler::viewstate::{Effect, ViewState};
use modeler_dae::DEFAULT_OVERHANG_DEGREES;
use modeler_kernel::model::{InstanceId, Model};
use modeler_kernel::select::Entity;
use print_setup_common::*;

fn bed(x: f64, y: f64, z: Option<f64>) -> Bed {
    Bed {
        x_mm: x,
        y_mm: y,
        z_mm: z,
    }
}

// ---- presets and parsing ----

#[test]
fn the_default_bed_is_220_square_with_no_height_limit() {
    assert_eq!(Bed::default(), bed(220.0, 220.0, None));
}

#[test]
fn there_are_presets_named_by_their_size() {
    for want in [(180.0, 180.0), (220.0, 220.0), (256.0, 256.0)] {
        assert!(
            BED_PRESETS.iter().any(|p| (p.x_mm, p.y_mm) == want),
            "{want:?}"
        );
    }
    for p in &BED_PRESETS {
        assert_eq!(p.name, format!("{} x {} mm", p.x_mm, p.y_mm));
        assert!((MIN_BED_MM..=MAX_BED_MM).contains(&p.x_mm));
        assert!((MIN_BED_MM..=MAX_BED_MM).contains(&p.y_mm));
    }
}

#[test]
fn a_preset_keeps_the_height_limit() {
    let p = BED_PRESETS.iter().find(|p| p.x_mm == 256.0).unwrap();
    assert_eq!(
        with_preset(bed(220.0, 220.0, Some(250.0)), p),
        bed(256.0, 256.0, Some(250.0))
    );
    assert_eq!(with_preset(Bed::default(), p), bed(256.0, 256.0, None));
}

#[test]
fn bed_sides_parse_as_plain_millimeters() {
    for (text, want) in [
        ("220", 220.0),
        ("  220.5 ", 220.5),
        ("220 mm", 220.0),
        ("220mm", 220.0),
        ("10", 10.0),
        ("2000", 2000.0),
    ] {
        assert_eq!(parse_side_mm(text), Some(want), "{text:?}");
    }
    for text in [
        "", "abc", "9.9", "2000.1", "-5", "0", "220,5", "1e3", "nan", "inf", "220 cm", "2 20",
    ] {
        assert_eq!(parse_side_mm(text), None, "{text:?}");
    }
}

#[test]
fn a_blank_height_means_no_limit() {
    assert_eq!(parse_height_mm(""), Some(None));
    assert_eq!(parse_height_mm("   "), Some(None));
    assert_eq!(parse_height_mm("250"), Some(Some(250.0)));
    assert_eq!(parse_height_mm("250 mm"), Some(Some(250.0)));
    assert_eq!(parse_height_mm("x"), None);
    assert_eq!(parse_height_mm("5"), None);
}

#[test]
fn millimeters_print_without_trailing_zeros() {
    for (mm, want) in [
        (220.0, "220"),
        (0.25, "0.25"),
        (12.346, "12.35"),
        (12.5, "12.5"),
        (0.0, "0"),
        (100.004, "100"),
    ] {
        assert_eq!(format_mm(mm), want, "{mm}");
    }
}

// ---- the outline ----

#[test]
fn the_outline_is_the_rectangle_at_z_zero_in_meters() {
    let lines = bed_outline(&bed(200.0, 100.0, Some(250.0)), 1.0);
    let c = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    let want = [
        (c(-0.1, -0.05), c(0.1, -0.05)),
        (c(0.1, -0.05), c(0.1, 0.05)),
        (c(0.1, 0.05), c(-0.1, 0.05)),
        (c(-0.1, 0.05), c(-0.1, -0.05)),
    ];
    assert_eq!(lines.len(), 4, "the height limit is not drawn");
    for (line, (a, b)) in lines.iter().zip(want) {
        assert!(
            line.a.distance(a) < 1e-12 && line.b.distance(b) < 1e-12,
            "{line:?}"
        );
        assert_eq!(line.color, BED_COLOR);
        assert_eq!(line.dash_px, None);
    }
}

#[test]
fn the_outline_width_follows_the_pixel_density() {
    let width = |ppp: f32| bed_outline(&Bed::default(), ppp)[0].width_px;
    assert_eq!(width(1.0), BED_WIDTH_PT);
    assert_eq!(width(2.0), BED_WIDTH_PT * 2.0);
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(width(bad), BED_WIDTH_PT, "{bad}");
    }
}

// ---- the fits-the-bed check ----

fn world(sizes: &[([f64; 3], DVec3)]) -> (Model, Vec<InstanceId>) {
    let mut model = Model::new();
    let ids = sizes
        .iter()
        .map(|&(size, at)| box_mm(&mut model, size, at))
        .collect();
    (model, ids)
}

#[test]
fn a_small_part_fits() {
    let (model, _) = world(&[([50.0, 40.0, 30.0], DVec3::ZERO)]);
    let r = fit_report(&model, None, &Bed::default());
    assert!(r.fits);
    assert_eq!(r.skipped, 0);
    let size = r.size_mm.unwrap();
    for (got, want) in size.iter().zip([50.0, 40.0, 30.0]) {
        assert!((got - want).abs() < 1e-6, "{size:?}");
    }
    assert_eq!(r.message, "Fits the bed: 50 x 40 x 30 mm on 220 x 220 mm");
}

#[test]
fn a_part_exactly_as_big_as_the_bed_fits() {
    let (model, _) = world(&[([220.0, 220.0, 10.0], DVec3::ZERO)]);
    assert!(fit_report(&model, None, &Bed::default()).fits);
}

#[test]
fn where_the_part_stands_does_not_matter() {
    let (model, _) = world(&[([50.0, 50.0, 50.0], DVec3::new(5.0, -7.0, 3.0))]);
    assert!(fit_report(&model, None, &Bed::default()).fits);
}

#[test]
fn too_big_names_the_axes_that_are_over() {
    let (model, _) = world(&[([300.0, 100.0, 60.0], DVec3::ZERO)]);
    let r = fit_report(&model, None, &bed(220.0, 220.0, Some(50.0)));
    assert!(!r.fits);
    assert_eq!(
        r.message,
        "Too big for the bed: 300 x 100 x 60 mm on 220 x 220 x 50 mm (X over by 80 mm, Z over by 10 mm)"
    );
    let r = fit_report(&model, None, &bed(220.0, 90.0, None));
    assert_eq!(
        r.message,
        "Too big for the bed: 300 x 100 x 60 mm on 220 x 90 mm (X over by 80 mm, Y over by 10 mm)"
    );
}

#[test]
fn the_height_is_only_checked_when_there_is_a_limit() {
    let (model, _) = world(&[([10.0, 10.0, 900.0], DVec3::ZERO)]);
    assert!(fit_report(&model, None, &Bed::default()).fits);
    assert!(!fit_report(&model, None, &bed(220.0, 220.0, Some(300.0))).fits);
}

#[test]
fn the_box_runs_over_every_part() {
    let (model, _) = world(&[
        ([100.0, 100.0, 10.0], DVec3::ZERO),
        ([100.0, 100.0, 10.0], DVec3::new(0.15, 0.0, 0.0)),
    ]);
    let r = fit_report(&model, None, &Bed::default());
    assert!(!r.fits, "250 mm wide");
    assert!(r.message.contains("250 x 100 x 10 mm"), "{}", r.message);
}

#[test]
fn only_the_named_instances_are_checked() {
    let (model, ids) = world(&[
        ([100.0, 100.0, 10.0], DVec3::ZERO),
        ([100.0, 100.0, 10.0], DVec3::new(0.15, 0.0, 0.0)),
    ]);
    let r = fit_report(&model, Some(&ids[..1]), &Bed::default());
    assert!(r.fits);
    assert_eq!(r.message, "Fits the bed: 100 x 100 x 10 mm on 220 x 220 mm");
}

#[test]
fn nothing_solid_is_not_a_fit() {
    let model = Model::new();
    let r = fit_report(&model, None, &Bed::default());
    assert!(!r.fits);
    assert_eq!(r.size_mm, None);
    assert_eq!(r.skipped, 0);
    assert_eq!(r.message, "Nothing to check: no solid groups or components");
}

#[test]
fn parts_that_are_not_solids_are_left_out_and_counted() {
    let mut model = Model::new();
    box_mm(&mut model, [50.0, 50.0, 50.0], DVec3::ZERO);
    add_box(&mut model, DVec3::splat(0.5), &[1], DMat4::IDENTITY);
    let r = fit_report(&model, None, &Bed::default());
    assert!(r.fits, "the open box is not part of the box");
    assert_eq!(r.skipped, 1);
    assert_eq!(
        r.message,
        "Fits the bed: 50 x 50 x 50 mm on 220 x 220 mm (1 not solid, left out)"
    );

    let mut only_open = Model::new();
    add_box(&mut only_open, DVec3::splat(0.05), &[1], DMat4::IDENTITY);
    let r = fit_report(&only_open, None, &Bed::default());
    assert!(!r.fits);
    assert_eq!(
        r.message,
        "Nothing to check: no solid groups or components (1 not solid, left out)"
    );
}

// ---- the settings file ----

#[test]
fn default_settings() {
    let s = PrintSettings::default();
    assert_eq!(s.bed, Bed::default());
    assert!(!s.show_bed);
    assert_eq!(s.overhang_degrees, DEFAULT_OVERHANG_DEGREES);
    assert_eq!(PrintSettings::parse(""), s);
}

#[test]
fn settings_text_round_trips() {
    let s = PrintSettings {
        bed: bed(256.0, 250.5, Some(256.0)),
        show_bed: true,
        overhang_degrees: 50.0,
    };
    assert_eq!(
        s.to_text(),
        "bed 256 250.5 256\nshow-bed yes\noverhang 50\n"
    );
    assert_eq!(PrintSettings::parse(&s.to_text()), s);
    let s = PrintSettings::default();
    assert_eq!(s.to_text(), "bed 220 220\nshow-bed no\noverhang 45\n");
    assert_eq!(PrintSettings::parse(&s.to_text()), s);
}

#[test]
fn bad_lines_keep_their_defaults_and_the_last_good_line_wins() {
    let text = "bed 5 5\r\nshow-bed maybe\nfoo bar\noverhang 120\nbed 180 180 200\nbed 1 2 3 4\n\
                overhang 30\noverhang x\nshow-bed yes\n";
    let s = PrintSettings::parse(text);
    assert_eq!(s.bed, bed(180.0, 180.0, Some(200.0)));
    assert_eq!(s.overhang_degrees, 30.0);
    assert!(s.show_bed);
}

#[test]
fn settings_are_saved_and_loaded_from_a_file() {
    let dir = std::env::temp_dir().join(format!("modeler-print-setup-{}", std::process::id()));
    let file = dir.join("config").join("print-setup");
    assert_eq!(
        PrintSettings::load(&file),
        PrintSettings::default(),
        "missing"
    );
    let s = PrintSettings {
        bed: bed(300.0, 300.0, None),
        show_bed: true,
        overhang_degrees: 60.0,
    };
    s.save(&file).unwrap();
    assert_eq!(PrintSettings::load(&file), s);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_settings_file_lives_in_the_config_folder() {
    let var = |name: &str| (name == "DRYDOCK_CONFIG_DIR").then(|| OsString::from("/cfg"));
    assert_eq!(
        settings_file(var),
        Some(std::path::PathBuf::from("/cfg/print-setup"))
    );
}

// ---- the view state ----

#[test]
fn the_view_starts_with_the_default_bed_hidden() {
    let view = ViewState::new();
    assert_eq!(view.bed, Bed::default());
    assert!(!view.bed_visible);
}

#[test]
fn a_new_overhang_angle_reuploads_only_while_overhangs_are_on() {
    let mut view = ViewState::new();
    assert_eq!(view.set_overhang_degrees(50.0), Effect::None);
    assert_eq!(view.overhang_degrees, 50.0);
    view.overhangs = true;
    assert_eq!(view.set_overhang_degrees(50.0), Effect::None, "same value");
    assert_eq!(view.set_overhang_degrees(30.0), Effect::ReuploadScene);
    assert_eq!(view.overhang_degrees, 30.0);
    for ok in [MIN_OVERHANG_DEGREES, MAX_OVERHANG_DEGREES] {
        assert_eq!(view.set_overhang_degrees(ok), Effect::ReuploadScene);
        assert_eq!(view.overhang_degrees, ok);
    }
}

#[test]
fn an_overhang_angle_out_of_range_changes_nothing() {
    let mut view = ViewState::new();
    view.overhangs = true;
    for bad in [0.0, 0.5, 89.5, 90.0, -10.0, f64::NAN, f64::INFINITY] {
        assert_eq!(view.set_overhang_degrees(bad), Effect::None, "{bad}");
        assert_eq!(view.overhang_degrees, DEFAULT_OVERHANG_DEGREES);
    }
}

#[test]
fn print_setup_opens_its_window() {
    let mut view = ViewState::new();
    assert_eq!(
        view.apply(Action::PrintSetup, (800.0, 600.0)),
        Effect::PrintSetup
    );
}

// ---- the Tools menu ----

#[test]
fn tools_has_print_setup_with_an_ellipsis() {
    let seen: Rc<RefCell<Vec<Action>>> = Rc::default();
    let sink = seen.clone();
    let state = MenuState::default();
    let mut h = Harness::new_ui(move |ui| {
        if let Some(action) = menu_bar(ui, &state) {
            sink.borrow_mut().push(action);
        }
    });
    h.run();
    h.get_by_label("Tools").click();
    h.run();
    h.get_by_label("Print Setup...").click();
    h.run();
    assert_eq!(*seen.borrow(), vec![Action::PrintSetup]);
}

// ---- Drop to Bed ----

struct World {
    model: Model,
    editor: Editor,
}

impl World {
    fn new() -> World {
        World {
            model: Model::new(),
            editor: Editor::new(),
        }
    }

    fn part(&mut self, size_mm: [f64; 3], at: DVec3) -> InstanceId {
        box_mm(&mut self.model, size_mm, at)
    }

    fn select(&mut self, ids: &[InstanceId]) {
        self.editor
            .select(ids.iter().map(|&i| Entity::Instance(i)).collect());
    }
}

#[test]
fn a_floating_part_drops_to_z_zero_and_stays_in_place_sideways() {
    let mut w = World::new();
    let id = w.part([50.0, 50.0, 50.0], DVec3::new(0.1, 0.2, 0.3));
    w.select(&[id]);
    assert!(w.editor.drop_to_bed(&mut w.model));
    assert!(lowest_z(&w.model, id).abs() < 1e-6);
    let t = w.model.root().instance(id).unwrap().transform;
    let p = t.transform_point3(DVec3::ZERO);
    assert!((p.x - 0.1).abs() < 1e-9 && (p.y - 0.2).abs() < 1e-9, "{p}");
    assert_eq!(w.editor.message(), None);
}

#[test]
fn a_part_below_the_bed_comes_up() {
    let mut w = World::new();
    let id = w.part([50.0, 50.0, 50.0], DVec3::new(0.0, 0.0, -0.2));
    w.select(&[id]);
    assert!(w.editor.drop_to_bed(&mut w.model));
    assert!(lowest_z(&w.model, id).abs() < 1e-6);
}

#[test]
fn a_turned_part_drops_by_its_lowest_corner() {
    let mut w = World::new();
    let turn = DMat4::from_translation(DVec3::new(0.0, 0.0, 0.4)) * DMat4::from_rotation_x(0.5);
    let id = add_box(&mut w.model, DVec3::splat(0.05), &[], turn);
    w.select(&[id]);
    assert!(w.editor.drop_to_bed(&mut w.model));
    assert!(lowest_z(&w.model, id).abs() < 1e-6);
}

#[test]
fn several_parts_drop_together_and_keep_their_gap() {
    let mut w = World::new();
    let low = w.part([50.0, 50.0, 50.0], DVec3::new(0.0, 0.0, 0.1));
    let high = w.part([50.0, 50.0, 50.0], DVec3::new(0.1, 0.0, 0.3));
    w.select(&[low, high]);
    let before = w.editor.undo_stack().undo_count();
    assert!(w.editor.drop_to_bed(&mut w.model));
    assert!(lowest_z(&w.model, low).abs() < 1e-6);
    assert!((lowest_z(&w.model, high) - 0.2).abs() < 1e-6);
    assert_eq!(w.editor.undo_stack().undo_count(), before + 1, "one step");
    assert_eq!(w.editor.undo_stack().undo_label(), Some("Drop to Bed"));
}

#[test]
fn drop_to_bed_undoes() {
    let mut w = World::new();
    let id = w.part([50.0, 50.0, 50.0], DVec3::new(0.0, 0.0, 0.3));
    w.select(&[id]);
    w.editor.drop_to_bed(&mut w.model);
    assert!(w.editor.key(EditKey::Undo, &mut w.model));
    assert!((lowest_z(&w.model, id) - 0.3).abs() < 1e-6);
}

#[test]
fn a_part_already_on_the_bed_changes_nothing() {
    let mut w = World::new();
    let id = w.part([50.0, 50.0, 50.0], DVec3::ZERO);
    w.select(&[id]);
    let before = w.editor.undo_stack().undo_count();
    assert!(!w.editor.drop_to_bed(&mut w.model));
    assert_eq!(w.editor.undo_stack().undo_count(), before);
    assert_eq!(w.editor.message(), None);
}

#[test]
fn drop_to_bed_needs_a_group_or_component_selected() {
    let mut w = World::new();
    let id = w.part([50.0, 50.0, 50.0], DVec3::new(0.0, 0.0, 0.3));
    let before = w.editor.undo_stack().undo_count();
    assert!(!w.editor.drop_to_bed(&mut w.model));
    assert_eq!(
        w.editor.message(),
        Some("Select groups or components to drop to the bed")
    );
    w.select(&[]);
    assert!(!w.editor.drop_to_bed(&mut w.model));
    assert_eq!(w.editor.undo_stack().undo_count(), before);
    assert!((lowest_z(&w.model, id) - 0.3).abs() < 1e-6);
}
