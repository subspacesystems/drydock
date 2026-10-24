// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Texture handles in the editor (`docs/specs/m4-app-texture-handles.md`).

use glam::{DMat4, DVec2, DVec3};
use modeler::editor::*;
use modeler::input::{EditKey, Modifiers, Tool};
use modeler::texture_handles::{Grip, TextureHandles};
use modeler_kernel::model::{
    BodyId, DefinitionInfo, Instance, InstanceId, Material, Model, ModelEdit, Texture, UndoStack,
};
use modeler_kernel::paint::{Side, create_material, paint};
use modeler_kernel::select::Entity;
use modeler_kernel::texture::{TextureMap, side_map};
use modeler_kernel::topology::FaceId;
use modeler_kernel::units::Unit;
use modeler_view::{Bounds, Camera, Projection, StandardView};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};
const SHIFT: Modifiers = Modifiers {
    shift: true,
    ctrl: false,
    alt: false,
};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn close2(a: DVec2, b: DVec2) -> bool {
    (a - b).length() < 1e-6
}

fn close3(a: DVec3, b: DVec3) -> bool {
    (a - b).length() < 1e-6
}

fn top(min: f64, max: f64) -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(StandardView::Top);
    camera.projection = Projection::Parallel;
    camera.zoom_extents(
        &Bounds {
            min: DVec3::splat(min),
            max: DVec3::splat(max),
        },
        800.0 / 600.0,
    );
    Viewport {
        camera,
        size_px: (800.0, 600.0),
    }
}

fn square(model: &mut Model, body: BodyId, log: &mut ModelEdit) -> FaceId {
    let pts = [
        v(0.0, 0.0, 0.0),
        v(3.0, 0.0, 0.0),
        v(3.0, 3.0, 0.0),
        v(0.0, 3.0, 0.0),
    ];
    model
        .edit_geometry(body, log, |ctx, edit| {
            for i in 0..4 {
                ctx.add_edge(pts[i], pts[(i + 1) % 4], edit).unwrap();
            }
        })
        .unwrap();
    model
        .body(body)
        .unwrap()
        .geometry()
        .faces()
        .next()
        .unwrap()
        .0
}

fn texture_front(model: &mut Model, body: BodyId, face: FaceId) {
    let mut undo = UndoStack::new();
    let mut material = Material::new("Brick", [1.0, 1.0, 1.0, 1.0]);
    let mut texture = Texture::from("brick.png");
    texture.size = DVec2::ONE;
    material.texture = Some(texture);
    let id = create_material(model, material, &mut undo, "New").unwrap();
    paint(
        model,
        body,
        &[Entity::Face(face)],
        Side::Front,
        Some(id),
        &mut undo,
        "Paint",
    )
    .unwrap();
}

/// A 3 m square at the root (z = 0) with a 1 m texture on its front.
fn setup() -> (Model, Editor, FaceId, Viewport) {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let face = square(&mut model, BodyId::Root, &mut log);
    texture_front(&mut model, BodyId::Root, face);
    let mut editor = Editor::new();
    editor.unit = Unit::Meter;
    editor.set_tool(Tool::Select);
    (model, editor, face, top(-2.0, 5.0))
}

fn map(model: &Model, body: BodyId, face: FaceId) -> TextureMap {
    side_map(model, body, face, Side::Front).unwrap()
}

fn px(view: &Viewport, p: DVec3) -> DVec2 {
    view.project(p).unwrap()
}

fn handles(editor: &Editor, model: &Model) -> TextureHandles {
    editor.texture_handles(model).expect("handles showing")
}

fn started(model: &Model, editor: &mut Editor, face: FaceId) -> TextureHandles {
    assert!(editor.start_texture_handles(model, face, Side::Front));
    handles(editor, model)
}

fn middle(corners: &[DVec3; 4]) -> DVec3 {
    corners.iter().sum::<DVec3>() / 4.0
}

#[test]
fn start_shows_the_image_copy_under_the_face_center() {
    let (model, mut editor, face, _) = setup();
    let h = started(&model, &mut editor, face);
    assert_eq!(h.face, face);
    assert_eq!(h.side, Side::Front);
    assert_eq!(h.hover, None);
    assert_eq!(h.held, None);
    assert_eq!(editor.message(), None);
    // The face center (1.5, 1.5) lies in the copy covering x and y in [1, 2].
    let m = map(&model, BodyId::Root, face);
    let a = m.apply(h.corners[0]);
    assert!(close2(m.apply(h.corners[1]), a + DVec2::new(1.0, 0.0)));
    assert!(close2(m.apply(h.corners[2]), a + DVec2::new(1.0, 1.0)));
    assert!(close2(m.apply(h.corners[3]), a + DVec2::new(0.0, 1.0)));
    assert!(
        close3(middle(&h.corners), v(1.5, 1.5, 0.0)),
        "{:?}",
        h.corners
    );
    assert!(h.corners.iter().all(|c| c.z.abs() < 1e-9));
}

#[test]
fn start_on_a_side_without_texture_is_refused() {
    let (model, mut editor, face, _) = setup();
    assert!(!editor.start_texture_handles(&model, face, Side::Back));
    assert_eq!(editor.message(), Some("That side has no texture"));
    assert!(editor.texture_handles(&model).is_none());
}

#[test]
fn hover_reports_the_grip_under_the_cursor() {
    let (mut model, mut editor, face, view) = setup();
    let h = started(&model, &mut editor, face);
    let cases = [
        (
            px(&view, h.corners[1]) + DVec2::new(3.0, -2.0),
            Some(Grip::Turn),
        ),
        (
            px(&view, h.corners[2]) + DVec2::new(-2.0, 3.0),
            Some(Grip::Scale(2)),
        ),
        (px(&view, middle(&h.corners)), Some(Grip::Slide)),
        (px(&view, v(0.2, 2.8, 0.0)), None),
        (px(&view, v(-1.5, -1.5, 0.0)), None),
    ];
    for (cursor, grip) in cases {
        editor.pointer_move(&mut model, &view, cursor, NONE);
        assert_eq!(handles(&editor, &model).hover, grip, "at {cursor}");
        assert!(editor.hint().is_none());
    }
}

#[test]
fn slide_keeps_the_grabbed_texture_point_under_the_cursor() {
    let (mut model, mut editor, face, view) = setup();
    let h = started(&model, &mut editor, face);
    let m0 = map(&model, BodyId::Root, face);
    let p1 = middle(&h.corners) + v(0.1, 0.2, 0.0);
    let p2 = p1 + v(0.35, -0.6, 0.0);
    assert!(!editor.click(&mut model, &view, px(&view, p1), NONE, 1));
    assert_eq!(handles(&editor, &model).held, Some(Grip::Slide));

    // The outline follows the cursor; the model waits for the click.
    let undo_before = editor.undo_stack().undo_count();
    editor.pointer_move(&mut model, &view, px(&view, p2), NONE);
    let preview = handles(&editor, &model);
    for (moved, start) in preview.corners.iter().zip(h.corners) {
        assert!(close3(*moved, start + (p2 - p1)), "{:?}", preview.corners);
    }
    assert_eq!(editor.undo_stack().undo_count(), undo_before);
    assert_eq!(map(&model, BodyId::Root, face), m0);

    assert!(editor.click(&mut model, &view, px(&view, p2), NONE, 1));
    let m1 = map(&model, BodyId::Root, face);
    assert!(
        close2(m1.apply(p2), m0.apply(p1)),
        "{} {}",
        m1.apply(p2),
        m0.apply(p1)
    );
    assert_eq!(editor.undo_stack().undo_count(), undo_before + 1);
    assert_eq!(editor.undo_stack().undo_label(), Some("Position Texture"));
    let after = handles(&editor, &model);
    assert_eq!(after.held, None);
}

#[test]
fn turn_points_the_bottom_edge_at_the_cursor_about_the_pin() {
    let (mut model, mut editor, face, view) = setup();
    let h = started(&model, &mut editor, face);
    let m0 = map(&model, BodyId::Root, face);
    let a = m0.apply(h.corners[0]);
    assert!(!editor.click(&mut model, &view, px(&view, h.corners[1]), NONE, 1));
    assert_eq!(handles(&editor, &model).held, Some(Grip::Turn));
    let p =
        h.corners[0] + 0.3 * (h.corners[1] - h.corners[0]) + 0.8 * (h.corners[3] - h.corners[0]);
    assert!(editor.click(&mut model, &view, px(&view, p), NONE, 1));
    let m1 = map(&model, BodyId::Root, face);
    assert!(close2(m1.apply(h.corners[0]), a), "the pin moved");
    let d = m1.apply(p) - a;
    assert!(d.y.abs() < 1e-6 && d.x > 0.0, "{d}");
    assert!((d.length() - (0.3f64.hypot(0.8))).abs() < 1e-6, "{d}");
}

#[test]
fn scale_puts_the_top_right_corner_under_the_cursor() {
    let (mut model, mut editor, face, view) = setup();
    let h = started(&model, &mut editor, face);
    let m0 = map(&model, BodyId::Root, face);
    let a = m0.apply(h.corners[0]);
    assert!(!editor.click(&mut model, &view, px(&view, h.corners[2]), NONE, 1));
    assert_eq!(handles(&editor, &model).held, Some(Grip::Scale(2)));
    let p =
        h.corners[0] + 2.0 * (h.corners[1] - h.corners[0]) + 0.5 * (h.corners[3] - h.corners[0]);
    assert!(editor.click(&mut model, &view, px(&view, p), NONE, 1));
    let m1 = map(&model, BodyId::Root, face);
    assert!(close2(m1.apply(h.corners[0]), a), "the pin moved");
    assert!(close2(m1.apply(p), a + DVec2::ONE), "{}", m1.apply(p) - a);
    assert_eq!(editor.undo_stack().undo_label(), Some("Position Texture"));
}

#[test]
fn shift_scales_uniformly() {
    let (mut model, mut editor, face, view) = setup();
    let h = started(&model, &mut editor, face);
    let m0 = map(&model, BodyId::Root, face);
    let a = m0.apply(h.corners[0]);
    assert!(!editor.click(&mut model, &view, px(&view, h.corners[2]), NONE, 1));
    let p =
        h.corners[0] + 2.0 * (h.corners[1] - h.corners[0]) + 0.5 * (h.corners[3] - h.corners[0]);
    editor.pointer_move(&mut model, &view, px(&view, p), SHIFT);
    assert!(editor.click(&mut model, &view, px(&view, p), SHIFT, 1));
    // q - A = (2, 0.5), H - A = (1, 1): k = 2 / 2.5.
    let m1 = map(&model, BodyId::Root, face);
    assert!(close2(m1.apply(h.corners[1]) - a, DVec2::new(0.8, 0.0)));
    assert!(close2(m1.apply(h.corners[3]) - a, DVec2::new(0.0, 0.8)));
}

#[test]
fn scale_across_the_pin_is_refused_and_stays_held() {
    let (mut model, mut editor, face, view) = setup();
    let h = started(&model, &mut editor, face);
    let m0 = map(&model, BodyId::Root, face);
    assert!(!editor.click(&mut model, &view, px(&view, h.corners[2]), NONE, 1));
    let p = h.corners[0] - 0.5 * (h.corners[2] - h.corners[0]);
    editor.pointer_move(&mut model, &view, px(&view, p), NONE);
    let preview = handles(&editor, &model);
    for (got, want) in preview.corners.iter().zip(h.corners) {
        assert!(
            close3(*got, want),
            "an invalid op must not move the outline"
        );
    }
    let undo_before = editor.undo_stack().undo_count();
    assert!(!editor.click(&mut model, &view, px(&view, p), NONE, 1));
    assert_eq!(editor.message(), Some("Can't position that texture"));
    assert_eq!(handles(&editor, &model).held, Some(Grip::Scale(2)));
    assert_eq!(editor.undo_stack().undo_count(), undo_before);
    assert_eq!(map(&model, BodyId::Root, face), m0);
}

#[test]
fn esc_drops_the_grip_then_ends_the_handles() {
    let (mut model, mut editor, face, view) = setup();
    let h = started(&model, &mut editor, face);
    let m0 = map(&model, BodyId::Root, face);
    assert!(!editor.click(&mut model, &view, px(&view, middle(&h.corners)), NONE, 1));
    editor.pointer_move(&mut model, &view, px(&view, v(2.7, 0.4, 0.0)), NONE);
    assert!(!editor.key(EditKey::Cancel, &mut model));
    let after = handles(&editor, &model);
    assert_eq!(after.held, None);
    for (got, want) in after.corners.iter().zip(h.corners) {
        assert!(close3(*got, want));
    }
    assert_eq!(map(&model, BodyId::Root, face), m0);
    assert!(!editor.key(EditKey::Cancel, &mut model));
    assert!(editor.texture_handles(&model).is_none());
}

#[test]
fn enter_ends_the_handles() {
    let (mut model, mut editor, face, view) = setup();
    let h = started(&model, &mut editor, face);
    assert!(!editor.click(&mut model, &view, px(&view, h.corners[1]), NONE, 1));
    assert!(!editor.key(EditKey::Enter, &mut model));
    assert!(editor.texture_handles(&model).is_none());
}

#[test]
fn clicking_away_ends_the_handles_without_changes() {
    let (mut model, mut editor, face, view) = setup();
    started(&model, &mut editor, face);
    let undo_before = editor.undo_stack().undo_count();
    let selection = editor.selection().to_vec();
    assert!(!editor.click(&mut model, &view, px(&view, v(-1.5, 4.0, 0.0)), NONE, 1));
    assert!(editor.texture_handles(&model).is_none());
    assert_eq!(editor.undo_stack().undo_count(), undo_before);
    assert_eq!(editor.selection(), &selection[..]);
}

#[test]
fn changing_tool_ends_the_handles() {
    let (model, mut editor, face, _) = setup();
    started(&model, &mut editor, face);
    editor.set_tool(Tool::Move);
    assert!(editor.texture_handles(&model).is_none());
    started(&model, &mut editor, face);
    editor.end_texture_handles();
    assert!(editor.texture_handles(&model).is_none());
}

#[test]
fn start_keeps_the_selection_and_ends_a_paste() {
    let (mut model, mut editor, face, view) = setup();
    editor.click(&mut model, &view, px(&view, v(0.4, 2.6, 0.0)), NONE, 1);
    assert_eq!(editor.selection(), &[Entity::Face(face)]);
    assert!(editor.copy(&model));
    assert!(editor.paste(&model));
    assert!(editor.pasting());
    started(&model, &mut editor, face);
    assert!(!editor.pasting());
    assert_eq!(editor.selection(), &[Entity::Face(face)]);
}

#[test]
fn undo_restores_the_map_and_the_handles_stay() {
    let (mut model, mut editor, face, view) = setup();
    let h = started(&model, &mut editor, face);
    let p1 = middle(&h.corners);
    assert!(!editor.click(&mut model, &view, px(&view, p1), NONE, 1));
    assert!(editor.click(
        &mut model,
        &view,
        px(&view, p1 + v(0.25, 0.0, 0.0)),
        NONE,
        1
    ));
    assert!(editor.key(EditKey::Undo, &mut model));
    let after = handles(&editor, &model);
    assert_eq!(model.root().face_materials(face).front_map, None);
    for (got, want) in after.corners.iter().zip(h.corners) {
        assert!(close3(*got, want));
    }
}

#[test]
fn typing_is_ignored_while_handles_show() {
    let (model, mut editor, face, _) = setup();
    started(&model, &mut editor, face);
    assert!(!editor.text("5"));
    assert_eq!(editor.typed(), "");
}

fn instance_world() -> (Model, Editor, InstanceId, FaceId, BodyId) {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let d = model.add_definition(
        DefinitionInfo {
            name: "part".into(),
            group: false,
        },
        &mut log,
    );
    let body = BodyId::Definition(d);
    let face = square(&mut model, body, &mut log);
    texture_front(&mut model, body, face);
    let i = model
        .add_instance(
            BodyId::Root,
            Instance {
                definition: d,
                transform: DMat4::from_translation(v(6.0, 0.0, 0.0)),
                name: String::new(),
                material: None,
            },
            &mut log,
        )
        .unwrap();
    let mut editor = Editor::new();
    editor.unit = Unit::Meter;
    editor.set_tool(Tool::Select);
    assert!(editor.open_instance(&model, i));
    (model, editor, i, face, body)
}

#[test]
fn handles_work_inside_an_open_instance() {
    let (mut model, mut editor, _, face, body) = instance_world();
    let view = top(-1.0, 10.0);
    let world = |p: DVec3| p + v(6.0, 0.0, 0.0);
    let h = started(&model, &mut editor, face);
    // Corners are in the definition's coordinates.
    assert!(
        close3(middle(&h.corners), v(1.5, 1.5, 0.0)),
        "{:?}",
        h.corners
    );
    editor.pointer_move(&mut model, &view, px(&view, world(h.corners[1])), NONE);
    assert_eq!(handles(&editor, &model).hover, Some(Grip::Turn));

    let m0 = map(&model, body, face);
    let p1 = middle(&h.corners);
    let p2 = p1 + v(-0.3, 0.45, 0.0);
    assert!(!editor.click(&mut model, &view, px(&view, world(p1)), NONE, 1));
    assert!(editor.click(&mut model, &view, px(&view, world(p2)), NONE, 1));
    let m1 = map(&model, body, face);
    assert!(close2(m1.apply(p2), m0.apply(p1)));
}
