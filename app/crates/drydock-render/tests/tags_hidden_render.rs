// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-tags-hidden: a box on a hidden tag leaves no pixels in the headless render
//! (`docs/specs/m10-tags-hidden.md`). Lavapipe like `render.rs`.

use drydock_dae::kernel_scene;
use drydock_kernel::model::*;
use drydock_render::{Style, render_headless};
use drydock_view::{Camera, Projection, StandardView};
use glam::{DMat4, DVec3};

const W: u32 = 64;
const H: u32 = 64;

/// A 2 m cube group at the origin as one root instance.
fn cube_model() -> (Model, InstanceId) {
    let mut m = Model::new();
    let d = m.add_definition(
        DefinitionInfo {
            name: "Cube".into(),
            group: true,
        },
        &mut ModelEdit::new(),
    );
    let c = |i: usize| {
        DVec3::new(
            (i & 1) as f64 * 2.0 - 1.0,
            ((i >> 1) & 1) as f64 * 2.0 - 1.0,
            ((i >> 2) & 1) as f64 * 2.0 - 1.0,
        )
    };
    m.edit_geometry(BodyId::Definition(d), &mut ModelEdit::new(), |ctx, edit| {
        for a in 0..8usize {
            for bit in [1usize, 2, 4] {
                if a & bit == 0 {
                    ctx.add_edge(c(a), c(a | bit), edit).unwrap();
                }
            }
        }
    })
    .unwrap();
    let i = m
        .add_instance(
            BodyId::Root,
            Instance {
                definition: d,
                transform: DMat4::IDENTITY,
                name: "Cube".into(),
                material: None,
            },
            &mut ModelEdit::new(),
        )
        .unwrap();
    (m, i)
}

fn camera() -> Camera {
    let mut c = Camera {
        target: DVec3::ZERO,
        distance: 8.0,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: 35f64.to_radians(),
        projection: Projection::Perspective,
    };
    c.set_standard_view(StandardView::Front);
    c
}

/// Distinct colors in the render and the color of the corner pixel (background).
fn render(model: &Model) -> (usize, [u8; 4]) {
    let scene = kernel_scene(model);
    let (img, _) = render_headless(&scene, &camera(), W, H, &Style::default())
        .expect("headless device (lavapipe) available");
    let background = img.get_pixel(0, 0).0;
    let differing = img.pixels().filter(|p| p.0 != background).count();
    (differing, background)
}

#[test]
fn a_hidden_box_leaves_no_pixels() {
    let (mut m, i) = cube_model();
    let tag = m.add_tag("Hidden", &mut ModelEdit::new()).unwrap();
    m.assign_tag(i, Some(tag), &mut ModelEdit::new()).unwrap();

    let (shown, _) = render(&m);
    assert!(
        shown > 100,
        "the box is drawn when its tag is visible: {shown}"
    );

    m.set_tag_visible(tag, false, &mut ModelEdit::new())
        .unwrap();
    let (hidden, _) = render(&m);
    assert_eq!(hidden, 0, "a hidden box draws no pixel");

    m.set_tag_visible(tag, true, &mut ModelEdit::new()).unwrap();
    let (again, _) = render(&m);
    assert_eq!(again, shown);
}
