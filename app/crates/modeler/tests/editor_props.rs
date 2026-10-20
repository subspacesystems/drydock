// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Property tests for the editor (`docs/specs/m2-app-editor.md`).

use glam::{DVec2, DVec3};
use modeler::editor::*;
use modeler::input::{EditKey, Modifiers, Tool};
use modeler_kernel::model::Model;
use modeler_kernel::select::Entity;
use modeler_kernel::units::Unit;
use modeler_view::{Bounds, Camera, Projection, StandardView};
use proptest::prelude::*;

fn camera(yaw: f64, pitch: f64, distance: f64, parallel: bool) -> Camera {
    Camera {
        target: DVec3::new(1.0, -2.0, 0.5),
        yaw,
        pitch,
        distance,
        projection: if parallel {
            Projection::Parallel
        } else {
            Projection::Perspective
        },
        ..Camera::default()
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn points_on_the_ray_project_to_the_cursor(
        yaw in -3.1f64..3.1,
        pitch in -1.5f64..1.5,
        distance in 0.5f64..200.0,
        parallel in any::<bool>(),
        w in 50.0f64..2000.0,
        h in 50.0f64..2000.0,
        cx in 0.0f64..1.0,
        cy in 0.0f64..1.0,
        s in 0.2f64..3.0,
    ) {
        let view = Viewport { camera: camera(yaw, pitch, distance, parallel), size_px: (w, h) };
        let cursor = DVec2::new(cx * w, cy * h);
        let (origin, dir) = view.ray(cursor);
        prop_assert!((dir.length() - 1.0).abs() < 1e-9);
        // A point near the target's depth.
        let t = (view.camera.eye() - origin).length() + s * distance;
        let back = view.project(origin + dir * t).expect("in front of the camera");
        prop_assert!(back.distance(cursor) < 1e-6 * (w + h), "{:?} -> {:?}", cursor, back);
    }
}

#[derive(Clone, Debug)]
enum Op {
    Move(f64, f64, bool),
    Click(f64, f64, u8, u32),
    Box(f64, f64, f64, f64, u8),
    Text(String),
    Key(u8),
    Tool(u8),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0.0f64..800.0, 0.0f64..600.0, any::<bool>()).prop_map(|(x, y, s)| Op::Move(x, y, s)),
        4 => (0.0f64..800.0, 0.0f64..600.0, 0u8..8, 1u32..4)
            .prop_map(|(x, y, m, n)| Op::Click(x, y, m, n)),
        1 => (0.0f64..800.0, 0.0f64..600.0, 0.0f64..800.0, 0.0f64..600.0, 0u8..8)
            .prop_map(|(a, b, c, d, m)| Op::Box(a, b, c, d, m)),
        1 => prop::sample::select(vec!["1", "2.5", "-3", "4,5", "1m", "x", "12\"", "'"])
            .prop_map(|s| Op::Text(s.to_string())),
        2 => (0u8..10).prop_map(Op::Key),
        1 => (0u8..5).prop_map(Op::Tool),
    ]
}

fn mods(m: u8) -> Modifiers {
    Modifiers {
        shift: m & 1 != 0,
        ctrl: m & 2 != 0,
        alt: m & 4 != 0,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn random_input_keeps_the_model_and_selection_valid(
        iso in any::<bool>(),
        parallel in any::<bool>(),
        ops in prop::collection::vec(op(), 1..60),
    ) {
        let mut cam = Camera::default();
        cam.set_standard_view(if iso { StandardView::Iso } else { StandardView::Top });
        cam.projection = if parallel { Projection::Parallel } else { Projection::Perspective };
        cam.zoom_extents(
            &Bounds { min: DVec3::splat(-5.0), max: DVec3::splat(5.0) },
            800.0 / 600.0,
        );
        let view = Viewport { camera: cam, size_px: (800.0, 600.0) };
        let mut model = Model::new();
        let mut editor = Editor::new();
        editor.unit = Unit::Meter;
        editor.set_tool(Tool::Line);
        for op in ops {
            let undo_before = editor.undo_stack().undo_count();
            match op {
                Op::Move(x, y, s) => {
                    editor.pointer_move(&mut model, &view, DVec2::new(x, y), mods(u8::from(s)))
                }
                Op::Click(x, y, m, n) => {
                    let changed = editor.click(&mut model, &view, DVec2::new(x, y), mods(m), n);
                    prop_assert_eq!(changed, editor.undo_stack().undo_count() == undo_before + 1);
                }
                Op::Box(a, b, c, d, m) => editor.box_select(
                    &model,
                    &view,
                    DVec2::new(a, b),
                    DVec2::new(c, d),
                    mods(m),
                ),
                Op::Text(s) => {
                    editor.text(&s);
                }
                Op::Key(k) => {
                    let key = match k {
                        0 => EditKey::Cancel,
                        1 | 2 => EditKey::Enter,
                        3 => EditKey::Backspace,
                        4 => EditKey::Undo,
                        5 => EditKey::Redo,
                        n => EditKey::Lock(usize::from(n - 6)),
                    };
                    editor.key(key, &mut model);
                }
                Op::Tool(t) => editor.set_tool(match t {
                    0 | 1 => Tool::Line,
                    2 | 3 => Tool::Select,
                    _ => Tool::Orbit,
                }),
            }
            prop_assert!(model.validate().is_empty(), "{:?}", model.validate());
            let ctx = model.root().geometry();
            prop_assert!(ctx.validate().is_empty(), "{:?}", ctx.validate());
            let sel = editor.selection();
            prop_assert!(sel.windows(2).all(|w| w[0] < w[1]), "sorted, unique: {:?}", sel);
            for e in sel {
                let exists = match e {
                    Entity::Edge(id) => ctx.edge(*id).is_some(),
                    Entity::Face(id) => ctx.face(*id).is_some(),
                    Entity::Instance(id) => model.root().instance(*id).is_some(),
                };
                prop_assert!(exists, "stale {:?}", e);
            }
            for (a, b) in editor.preview() {
                prop_assert!(a.is_finite() && b.is_finite());
            }
        }
    }
}
