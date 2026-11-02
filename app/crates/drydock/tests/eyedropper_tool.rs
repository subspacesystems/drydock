// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The armed eyedropper is a tool of its own (round 5: it only sampled in the
//! Select tool). It takes the viewport's left button in every tool, Orbit, Pan and Zoom
//! included; the tool underneath shows no preview while it waits, and is active again after
//! the sample.

use drydock::editor::*;
use drydock::input::{Modifiers, Tool, left_button_tool};
use drydock_kernel::model::{BodyId, Material, MaterialId, Model, UndoStack};
use drydock_kernel::paint::{Side, create_material, paint};
use drydock_kernel::select::Entity;
use drydock_kernel::tools::{Target, draw_segments};
use drydock_kernel::units::Unit;
use drydock_view::{Bounds, Camera, Projection, StandardView};
use glam::{DVec2, DVec3};

const NONE: Modifiers = Modifiers {
    shift: false,
    ctrl: false,
    alt: false,
};

const ALL_TOOLS: [Tool; 16] = [
    Tool::Orbit,
    Tool::Pan,
    Tool::Zoom,
    Tool::Select,
    Tool::Line,
    Tool::Rectangle,
    Tool::Circle,
    Tool::Arc,
    Tool::Arc3,
    Tool::PushPull,
    Tool::Rotate,
    Tool::Move,
    Tool::Scale,
    Tool::Tape,
    Tool::Erase,
    Tool::Paint,
];

fn top_view() -> Viewport {
    let mut camera = Camera::default();
    camera.set_standard_view(StandardView::Top);
    camera.projection = Projection::Parallel;
    camera.zoom_extents(
        &Bounds {
            min: DVec3::splat(-4.0),
            max: DVec3::splat(4.0),
        },
        800.0 / 600.0,
    );
    Viewport {
        camera,
        size_px: (800.0, 600.0),
    }
}

/// A unit square at the origin painted blue on both sides; red is current.
struct World {
    model: Model,
    editor: Editor,
    view: Viewport,
    blue: MaterialId,
}

impl World {
    fn new(tool: Tool) -> World {
        let mut model = Model::new();
        let mut undo = UndoStack::new();
        let p = [
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(1.0, 1.0, 0.0),
            DVec3::new(0.0, 1.0, 0.0),
        ];
        let segments: Vec<_> = (0..4).map(|i| (p[i], p[(i + 1) % 4])).collect();
        let mut t = Target {
            model: &mut model,
            body: BodyId::Root,
            undo: &mut undo,
            unit: Unit::Meter,
        };
        draw_segments(&mut t, &segments, "Draw").expect("draw");
        let red = create_material(
            &mut model,
            Material::new("red", [1.0, 0.0, 0.0, 1.0]),
            &mut undo,
            "New",
        )
        .expect("red");
        let blue = create_material(
            &mut model,
            Material::new("blue", [0.0, 0.0, 1.0, 1.0]),
            &mut undo,
            "New",
        )
        .expect("blue");
        let face = model.root().geometry().faces().next().expect("face").0;
        for side in [Side::Front, Side::Back] {
            paint(
                &mut model,
                BodyId::Root,
                &[Entity::Face(face)],
                side,
                Some(blue),
                &mut undo,
                "Paint",
            )
            .expect("paint");
        }
        let mut editor = Editor::new();
        editor.set_tool(tool);
        editor.set_material(Some(red));
        World {
            model,
            editor,
            view: top_view(),
            blue,
        }
    }

    fn px(&self, p: DVec3) -> DVec2 {
        self.view.project(p).expect("visible")
    }
}

const ON_FACE: DVec3 = DVec3::new(0.5, 0.5, 0.0);

#[test]
fn the_armed_eyedropper_takes_the_left_button_in_every_tool() {
    for tool in ALL_TOOLS {
        assert_eq!(left_button_tool(tool, true), None, "{tool:?} while armed");
        assert_eq!(left_button_tool(tool, false), Some(tool), "{tool:?}");
    }
}

#[test]
fn a_click_samples_in_every_tool_and_the_tool_stays_active() {
    for tool in ALL_TOOLS {
        let mut w = World::new(tool);
        w.editor.arm_eyedropper();
        let c = w.px(ON_FACE);
        let before = w.model.clone();
        w.editor.pointer_move(&mut w.model, &w.view, c, NONE);
        assert!(
            !w.editor.click(&mut w.model, &w.view, c, NONE, 1),
            "{tool:?}: sampling changes nothing in the model"
        );
        assert_eq!(w.editor.material(), Some(w.blue), "{tool:?}: sampled");
        assert!(!w.editor.eyedropper_armed(), "{tool:?}: one shot");
        assert_eq!(w.editor.tool(), tool, "{tool:?}: back in the tool");
        assert_eq!(w.model, before, "{tool:?}");
        assert_eq!(w.editor.undo_stack().undo_count(), 0, "{tool:?}");
    }
}

#[test]
fn while_armed_the_tool_underneath_shows_no_inference() {
    let mut w = World::new(Tool::Line);
    let c = w.px(ON_FACE);
    w.editor.pointer_move(&mut w.model, &w.view, c, NONE);
    assert!(
        w.editor.hint().is_some(),
        "the Line tool infers on the face"
    );
    w.editor.arm_eyedropper();
    w.editor.pointer_move(&mut w.model, &w.view, c, NONE);
    assert_eq!(w.editor.hint(), None, "the eyedropper is waiting");
    w.editor.click(&mut w.model, &w.view, c, NONE, 1);
    assert_eq!(w.editor.material(), Some(w.blue));
    w.editor.pointer_move(&mut w.model, &w.view, c, NONE);
    assert!(w.editor.hint().is_some(), "the Line tool is back");
}
