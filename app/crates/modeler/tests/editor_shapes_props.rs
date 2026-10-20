// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Property tests for the shape tools in the editor (`docs/specs/m2-app-shapes.md`).

use glam::{DVec2, DVec3};
use modeler::editor::*;
use modeler::input::{EditKey, Modifiers, Tool};
use modeler_kernel::model::{BodyId, Model, UndoStack};
use modeler_kernel::tools::{Target, draw_segments};
use modeler_kernel::units::Unit;
use modeler_view::{Bounds, Camera, Projection, StandardView};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn plane_for_normal_is_orthonormal(x in -1.0f64..1.0, y in -1.0f64..1.0, z in -1.0f64..1.0) {
        let n = DVec3::new(x, y, z);
        prop_assume!(n.length() > 1e-3);
        let n = n.normalize();
        let p = plane_for_normal(n);
        prop_assert!((p.u.length() - 1.0).abs() < 1e-9);
        prop_assert!((p.v.length() - 1.0).abs() < 1e-9);
        prop_assert!(p.u.dot(p.v).abs() < 1e-9);
        prop_assert!(p.u.cross(p.v).distance(n) < 1e-9, "{:?} {:?}", p, n);
        if n.z.abs() < 0.99 {
            prop_assert!(p.u.z.abs() < 1e-9, "u not horizontal: {:?}", p.u);
        }
    }
}

#[derive(Clone, Debug)]
enum Op {
    Move(f64, f64),
    Click(f64, f64, u32),
    Text(String),
    Key(u8),
    Tool(u8),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => (0.0f64..800.0, 0.0f64..600.0).prop_map(|(x, y)| Op::Move(x, y)),
        4 => (0.0f64..800.0, 0.0f64..600.0, 1u32..3).prop_map(|(x, y, n)| Op::Click(x, y, n)),
        1 => prop::sample::select(vec!["1", "2.5", "-3", "3,2", "8s", "1s", "400s", "90", "-45", "0", "x"])
            .prop_map(|s| Op::Text(s.to_string())),
        2 => (0u8..10).prop_map(Op::Key),
        1 => (0u8..8).prop_map(Op::Tool),
    ]
}

fn wall(model: &mut Model) {
    let pts = [
        DVec3::new(0.0, 0.0, 0.0),
        DVec3::new(2.0, 0.0, 0.0),
        DVec3::new(2.0, 0.0, 2.0),
        DVec3::new(0.0, 0.0, 2.0),
    ];
    let segs: Vec<_> = (0..4).map(|i| (pts[i], pts[(i + 1) % 4])).collect();
    let mut undo = UndoStack::new();
    let mut t = Target {
        model,
        body: BodyId::Root,
        undo: &mut undo,
        unit: Unit::Meter,
    };
    draw_segments(&mut t, &segs, "Wall").expect("wall");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn random_input_keeps_the_model_valid(
        view_kind in 0u8..3,
        parallel in any::<bool>(),
        first in 0u8..3,
        ops in prop::collection::vec(op(), 1..60),
    ) {
        let mut cam = Camera::default();
        cam.set_standard_view(match view_kind {
            0 => StandardView::Top,
            1 => StandardView::Front,
            _ => StandardView::Iso,
        });
        cam.projection = if parallel { Projection::Parallel } else { Projection::Perspective };
        cam.zoom_extents(
            &Bounds { min: DVec3::splat(-5.0), max: DVec3::splat(5.0) },
            800.0 / 600.0,
        );
        let view = Viewport { camera: cam, size_px: (800.0, 600.0) };
        let mut model = Model::new();
        wall(&mut model);
        let mut editor = Editor::new();
        editor.unit = Unit::Meter;
        editor.set_tool([Tool::Rectangle, Tool::Circle, Tool::Arc][usize::from(first)]);
        for op in ops {
            let undo_before = editor.undo_stack().undo_count();
            match op {
                Op::Move(x, y) => {
                    editor.pointer_move(&mut model, &view, DVec2::new(x, y), Modifiers::default())
                }
                Op::Click(x, y, n) => {
                    let changed =
                        editor.click(&mut model, &view, DVec2::new(x, y), Modifiers::default(), n);
                    prop_assert_eq!(changed, editor.undo_stack().undo_count() == undo_before + 1);
                }
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
                    let changed = editor.key(key, &mut model);
                    if key == EditKey::Enter {
                        prop_assert_eq!(changed, editor.undo_stack().undo_count() == undo_before + 1);
                    }
                }
                Op::Tool(t) => editor.set_tool(match t {
                    0 | 1 => Tool::Rectangle,
                    2 | 3 => Tool::Circle,
                    4 | 5 => Tool::Arc,
                    6 => Tool::Line,
                    _ => Tool::Select,
                }),
            }
            prop_assert!(model.validate().is_empty(), "{:?}", model.validate());
            let ctx = model.root().geometry();
            prop_assert!(ctx.validate().is_empty(), "{:?}", ctx.validate());
            for (a, b) in editor.preview() {
                prop_assert!(a.is_finite() && b.is_finite());
            }
            if let Some(p) = editor.draw_plane() {
                prop_assert!(p.u.is_finite() && p.v.is_finite());
                prop_assert!((p.normal().length() - 1.0).abs() < 1e-6, "{:?}", p);
            }
            if let Some(h) = editor.hint() {
                prop_assert!(h.point.is_finite());
            }
        }
    }
}
