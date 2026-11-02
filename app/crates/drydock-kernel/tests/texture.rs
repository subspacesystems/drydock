// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Texture maps, default placement, fitting, setting maps and the model's texture checks
//! (`docs/specs/m4-texture-model.md`).

use drydock_kernel::geom::polygon_normal;
use drydock_kernel::model::*;
use drydock_kernel::paint::*;
use drydock_kernel::select::Entity;
use drydock_kernel::texture::*;
use drydock_kernel::tools::*;
use drydock_kernel::topology::FaceId;
use drydock_kernel::units::Unit;
use glam::{DAffine2, DMat4, DQuat, DVec2, DVec3, DVec4};
use proptest::prelude::*;

mod transform_common;
use transform_common::*;

fn d4(x: f64, y: f64, z: f64, w: f64) -> DVec4 {
    DVec4::new(x, y, z, w)
}

fn close2(a: DVec2, b: DVec2, tol: f64) -> bool {
    (a - b).length() <= tol
}

fn close4(a: DVec4, b: DVec4, tol: f64) -> bool {
    (a - b).abs().max_element() <= tol
}

fn assert_map(m: TextureMap, u: DVec4, v: DVec4) {
    assert!(
        close4(m.u, u, 1e-12) && close4(m.v, v, 1e-12),
        "{m:?} != u {u:?}, v {v:?}"
    );
}

/// A map with every entry set, for storing and comparing.
fn sample_map(k: f64) -> TextureMap {
    TextureMap {
        u: d4(0.5 * k, -0.25, 0.125, 3.0),
        v: d4(0.0, 2.0, -1.0 + k, -0.75),
    }
}

fn face_normal(model: &Model, face: FaceId) -> DVec3 {
    let ctx = model.root().geometry();
    polygon_normal(&ctx.loop_points(&ctx.face(face).unwrap().outer))
}

// -- default placement -------------------------------------------------------------------

#[test]
fn default_for_up_and_down() {
    assert_map(
        TextureMap::default_for(DVec3::Z),
        d4(1.0, 0.0, 0.0, 0.0),
        d4(0.0, 1.0, 0.0, 0.0),
    );
    assert_map(
        TextureMap::default_for(-DVec3::Z),
        d4(-1.0, 0.0, 0.0, 0.0),
        d4(0.0, 1.0, 0.0, 0.0),
    );
}

#[test]
fn default_for_walls_stands_upright() {
    assert_map(
        TextureMap::default_for(DVec3::X),
        d4(0.0, 1.0, 0.0, 0.0),
        d4(0.0, 0.0, 1.0, 0.0),
    );
    assert_map(
        TextureMap::default_for(-DVec3::Y),
        d4(1.0, 0.0, 0.0, 0.0),
        d4(0.0, 0.0, 1.0, 0.0),
    );
    assert_map(
        TextureMap::default_for(DVec3::Y),
        d4(-1.0, 0.0, 0.0, 0.0),
        d4(0.0, 0.0, 1.0, 0.0),
    );
    assert_map(
        TextureMap::default_for(-DVec3::X),
        d4(0.0, -1.0, 0.0, 0.0),
        d4(0.0, 0.0, 1.0, 0.0),
    );
}

#[test]
fn default_for_ignores_length_and_tilts_up_the_slope() {
    assert_map(
        TextureMap::default_for(v(0.0, 0.0, 5.0)),
        d4(1.0, 0.0, 0.0, 0.0),
        d4(0.0, 1.0, 0.0, 0.0),
    );
    // a roof facing -y, 45 degrees: up the slope is (0, 1, 1)/sqrt 2, right is +x
    let r = std::f64::consts::FRAC_1_SQRT_2;
    assert_map(
        TextureMap::default_for(v(0.0, -3.0, 3.0)),
        d4(1.0, 0.0, 0.0, 0.0),
        d4(0.0, r, r, 0.0),
    );
}

#[test]
fn default_for_zero_or_bad_normal_is_the_up_default() {
    for n in [
        DVec3::ZERO,
        v(f64::NAN, 0.0, 1.0),
        v(0.0, f64::INFINITY, 0.0),
    ] {
        assert_map(
            TextureMap::default_for(n),
            d4(1.0, 0.0, 0.0, 0.0),
            d4(0.0, 1.0, 0.0, 0.0),
        );
    }
}

#[test]
fn nearly_horizontal_uses_the_y_axis() {
    // |h| is about 1e-8, below the 1e-6 cutoff
    let m = TextureMap::default_for(v(1e-8, 0.0, 1.0));
    assert!(close4(m.v, d4(0.0, 1.0, 0.0, 0.0), 1e-7), "{m:?}");
    assert!(close4(m.u, d4(1.0, 0.0, 0.0, 0.0), 1e-7), "{m:?}");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn default_axes_are_a_right_handed_frame(
        x in -1.0f64..1.0, y in -1.0f64..1.0, z in -1.0f64..1.0,
    ) {
        let n = v(x, y, z);
        prop_assume!(n.length() > 1e-3);
        let n = n.normalize();
        let m = TextureMap::default_for(n);
        let (u, w) = (m.u.truncate(), m.v.truncate());
        prop_assert!((u.length() - 1.0).abs() < 1e-9);
        prop_assert!((w.length() - 1.0).abs() < 1e-9);
        prop_assert!(u.dot(w).abs() < 1e-9);
        prop_assert!(u.dot(n).abs() < 1e-9);
        prop_assert!(w.dot(n).abs() < 1e-9);
        prop_assert!((u.cross(w) - n).length() < 1e-9);
        prop_assert_eq!(m.u.w, 0.0);
        prop_assert_eq!(m.v.w, 0.0);
        // up the slope: v never points down
        prop_assert!(w.z >= -1e-12);
    }
}

// -- apply, uv, then, transformed --------------------------------------------------------

#[test]
fn apply_and_uv() {
    let m = TextureMap {
        u: d4(1.0, 2.0, 3.0, 4.0),
        v: d4(-1.0, 0.5, 0.0, 0.25),
    };
    let p = v(1.0, 1.0, 1.0);
    assert_eq!(m.apply(p), DVec2::new(10.0, -0.25));
    let tex = Texture::new("brick.png", DVec2::new(2.0, 0.5));
    assert_eq!(m.uv(&tex, p), DVec2::new(5.0, -0.5));
    assert_eq!(Texture::from("a.png").size, DVec2::ONE);
    assert_eq!(Texture::from("a.png").path, "a.png");
}

#[test]
fn then_moves_turns_and_scales_the_image() {
    let m = TextureMap::default_for(DVec3::Z);
    let p = v(2.0, 1.0, 7.0);
    let shift = DAffine2::from_translation(DVec2::new(0.5, -1.0));
    assert!(close2(m.then(shift).apply(p), DVec2::new(2.5, 0.0), 1e-12));
    let turn = DAffine2::from_angle(std::f64::consts::FRAC_PI_2);
    assert!(close2(m.then(turn).apply(p), DVec2::new(-1.0, 2.0), 1e-12));
    let scale = DAffine2::from_scale(DVec2::new(2.0, 3.0));
    assert!(close2(m.then(scale).apply(p), DVec2::new(4.0, 3.0), 1e-12));
}

#[test]
fn transformed_rejects_singular_and_non_finite() {
    let m = TextureMap::default_for(DVec3::Z);
    assert_eq!(m.transformed(DMat4::from_scale(v(1.0, 0.0, 1.0))), None);
    assert_eq!(m.transformed(DMat4::from_scale(v(1e-5, 1e-5, 1e-5))), None);
    let mut bad = DMat4::IDENTITY;
    bad.w_axis.x = f64::NAN;
    assert_eq!(m.transformed(bad), None);
    assert_eq!(m.transformed(DMat4::IDENTITY), Some(m));
}

#[test]
fn transformed_follows_a_move() {
    let m = TextureMap::default_for(-DVec3::Y);
    let moved = m
        .transformed(DMat4::from_translation(v(3.0, 0.0, 1.0)))
        .unwrap();
    // the point that was at the origin is now at (3, 0, 1) and keeps (0, 0)
    assert!(close2(moved.apply(v(3.0, 0.0, 1.0)), DVec2::ZERO, 1e-12));
    assert!(close2(moved.apply(v(4.0, 0.0, 2.0)), DVec2::ONE, 1e-12));
}

fn affine_map() -> impl Strategy<Value = TextureMap> {
    proptest::array::uniform8(-3.0f64..3.0).prop_map(|a| TextureMap {
        u: d4(a[0], a[1], a[2], a[3]),
        v: d4(a[4], a[5], a[6], a[7]),
    })
}

fn point() -> impl Strategy<Value = DVec3> {
    (-10.0f64..10.0, -10.0f64..10.0, -10.0f64..10.0).prop_map(|(x, y, z)| v(x, y, z))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn transformed_keeps_what_each_point_had(
        m in affine_map(),
        p in point(),
        t in point(),
        axis in point(),
        angle in -3.0f64..3.0,
        s in (0.2f64..3.0, 0.2f64..3.0, 0.2f64..3.0),
        mirror in any::<bool>(),
    ) {
        prop_assume!(axis.length() > 1e-3);
        let mut scale = v(s.0, s.1, s.2);
        if mirror {
            scale.x = -scale.x;
        }
        let xf = DMat4::from_scale_rotation_translation(
            scale,
            DQuat::from_axis_angle(axis.normalize(), angle),
            t,
        );
        let moved = m.transformed(xf).unwrap();
        let got = moved.apply(xf.transform_point3(p));
        prop_assert!(close2(got, m.apply(p), 1e-8 * (1.0 + m.apply(p).length())), "{got:?}");
    }

    #[test]
    fn then_is_the_affine_after_the_map(
        m in affine_map(),
        p in point(),
        angle in -3.0f64..3.0,
        sx in 0.1f64..4.0, sy in 0.1f64..4.0,
        tx in -5.0f64..5.0, ty in -5.0f64..5.0,
    ) {
        let a = DAffine2::from_scale_angle_translation(DVec2::new(sx, sy), angle, DVec2::new(tx, ty));
        let got = m.then(a).apply(p);
        let want = a.transform_point2(m.apply(p));
        prop_assert!(close2(got, want, 1e-9 * (1.0 + want.length())), "{got:?} != {want:?}");
    }
}

// -- fit ---------------------------------------------------------------------------------

#[test]
fn fit_reproduces_an_exact_planar_map() {
    // a tilted quad and a known map with no component along its normal
    let pts = [
        v(1.0, 0.0, 0.0),
        v(3.0, 0.0, 1.0),
        v(3.0, 2.0, 1.0),
        v(1.0, 2.0, 0.0),
    ];
    let n = polygon_normal(&pts);
    let truth = TextureMap::default_for(n).then(DAffine2::from_scale_angle_translation(
        DVec2::new(0.5, 2.0),
        0.3,
        DVec2::new(1.25, -4.0),
    ));
    let st: Vec<DVec2> = pts.iter().map(|&p| truth.apply(p)).collect();
    let fit = TextureMap::fit(&pts, &st).unwrap();
    for (p, want) in pts.iter().zip(&st) {
        assert!(close2(fit.apply(*p), *want, 1e-9), "{p:?}");
    }
    assert!(fit.u.truncate().dot(n).abs() < 1e-9);
    assert!(fit.v.truncate().dot(n).abs() < 1e-9);
    // off the plane along the normal changes nothing
    let off = pts[2] + n * 5.0;
    assert!(close2(fit.apply(off), st[2], 1e-9));
    assert!(close4(fit.u, truth.u, 1e-9) && close4(fit.v, truth.v, 1e-9));
}

#[test]
fn fit_a_triangle() {
    let pts = [v(0.0, 0.0, 2.0), v(4.0, 0.0, 2.0), v(0.0, 2.0, 2.0)];
    let st = [
        DVec2::new(0.0, 0.0),
        DVec2::new(1.0, 0.0),
        DVec2::new(0.0, 1.0),
    ];
    let fit = TextureMap::fit(&pts, &st).unwrap();
    assert!(close4(fit.u, d4(0.25, 0.0, 0.0, 0.0), 1e-12), "{fit:?}");
    assert!(close4(fit.v, d4(0.0, 0.5, 0.0, 0.0), 1e-12), "{fit:?}");
}

#[test]
fn fit_refuses_bad_input() {
    let tri = [v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0)];
    let st3 = [DVec2::ZERO, DVec2::X, DVec2::Y];
    assert_eq!(TextureMap::fit(&tri, &st3[..2]), None, "length mismatch");
    assert_eq!(TextureMap::fit(&tri[..2], &st3[..2]), None, "two points");
    assert_eq!(TextureMap::fit(&[], &[]), None);
    let line = [v(0.0, 0.0, 0.0), v(1.0, 1.0, 1.0), v(2.0, 2.0, 2.0)];
    assert_eq!(TextureMap::fit(&line, &st3), None, "collinear");
    let nan = [DVec2::ZERO, DVec2::new(f64::NAN, 0.0), DVec2::Y];
    assert_eq!(TextureMap::fit(&tri, &nan), None);
    let inf = [
        v(0.0, 0.0, 0.0),
        v(f64::INFINITY, 0.0, 0.0),
        v(0.0, 1.0, 0.0),
    ];
    assert_eq!(TextureMap::fit(&inf, &st3), None);
}

fn residual(m: &TextureMap, pts: &[DVec3], st: &[DVec2]) -> f64 {
    pts.iter()
        .zip(st)
        .map(|(&p, &q)| (m.apply(p) - q).length_squared())
        .sum()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// On noisy data the fit is a least-squares solution: no in-plane map does better.
    #[test]
    fn fit_is_least_squares(
        noise in proptest::collection::vec((-0.3f64..0.3, -0.3f64..0.3), 5),
        other in affine_map(),
        tilt in -1.2f64..1.2,
        spin in -3.0f64..3.0,
        shift in point(),
    ) {
        let place = DMat4::from_translation(shift)
            * DMat4::from_rotation_z(spin)
            * DMat4::from_rotation_x(tilt);
        let flat = [
            v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0), v(3.0, 1.0, 0.0), v(2.0, 2.5, 0.0), v(0.0, 2.0, 0.0),
        ];
        let pts: Vec<DVec3> = flat.iter().map(|&p| place.transform_point3(p)).collect();
        let st: Vec<DVec2> = flat
            .iter()
            .zip(&noise)
            .map(|(p, (a, b))| DVec2::new(p.x * 0.7 + a, p.y * 1.3 - 2.0 + b))
            .collect();
        let fit = TextureMap::fit(&pts, &st).unwrap();
        let best = residual(&fit, &pts, &st);
        prop_assert!(best <= residual(&other, &pts, &st) + 1e-9);
        // nudging any entry of the fit can't help either
        for k in 0..8 {
            for delta in [1e-3, -1e-3] {
                let mut nudged = fit;
                if k < 4 { nudged.u[k] += delta; } else { nudged.v[k - 4] += delta; }
                prop_assert!(best <= residual(&nudged, &pts, &st) + 1e-12);
            }
        }
    }
}

// -- side_map ----------------------------------------------------------------------------

#[test]
fn side_map_defaults_follow_each_side() {
    let mut model = Model::new();
    let faces = cube(&mut model);
    for &f in &faces {
        let n = face_normal(&model, f);
        assert_eq!(
            side_map(&model, BodyId::Root, f, Side::Front),
            Some(TextureMap::default_for(n))
        );
        assert_eq!(
            side_map(&model, BodyId::Root, f, Side::Back),
            Some(TextureMap::default_for(-n))
        );
    }
    // the cube's top (normal +z): u = x, v = y on the front
    assert_map(
        side_map(&model, BodyId::Root, faces[1], Side::Front).unwrap(),
        d4(1.0, 0.0, 0.0, 0.0),
        d4(0.0, 1.0, 0.0, 0.0),
    );
}

#[test]
fn side_map_returns_the_stored_map_and_none_when_missing() {
    let mut model = Model::new();
    let faces = cube(&mut model);
    let mut log = ModelEdit::new();
    let stored = FaceMaterials {
        back_map: Some(sample_map(1.0)),
        ..Default::default()
    };
    model
        .set_face_materials(BodyId::Root, faces[2], stored, &mut log)
        .unwrap();
    assert_eq!(
        side_map(&model, BodyId::Root, faces[2], Side::Back),
        Some(sample_map(1.0))
    );
    let n = face_normal(&model, faces[2]);
    assert_eq!(
        side_map(&model, BodyId::Root, faces[2], Side::Front),
        Some(TextureMap::default_for(n))
    );
    assert_eq!(
        side_map(&model, BodyId::Root, FaceId(999), Side::Front),
        None
    );
    assert_eq!(
        side_map(
            &model,
            BodyId::Definition(DefinitionId(42)),
            faces[2],
            Side::Front
        ),
        None
    );
}

// -- set_texture_maps --------------------------------------------------------------------

#[test]
fn set_maps_is_one_undo_step_and_keeps_materials() {
    let mut model = Model::new();
    let faces = cube(&mut model);
    let mut log = ModelEdit::new();
    let red = model.add_material(Material::new("Red", [1.0, 0.0, 0.0, 1.0]), &mut log);
    model
        .set_face_materials(
            BodyId::Root,
            faces[0],
            FaceMaterials {
                front: Some(red),
                back_map: Some(sample_map(2.0)),
                ..Default::default()
            },
            &mut log,
        )
        .unwrap();
    let before = model.clone();
    let mut undo = UndoStack::new();
    let n = set_texture_maps(
        &mut model,
        BodyId::Root,
        &[
            (faces[0], Side::Front, Some(sample_map(1.0))),
            (faces[3], Side::Back, Some(sample_map(3.0))),
        ],
        &mut undo,
        "Position Texture",
    )
    .unwrap();
    assert_eq!(n, 2);
    assert_eq!(undo.undo_count(), 1);
    assert_eq!(undo.undo_label(), Some("Position Texture"));
    let fm = model.root().face_materials(faces[0]);
    assert_eq!(fm.front, Some(red));
    assert_eq!(fm.front_map, Some(sample_map(1.0)));
    assert_eq!(fm.back_map, Some(sample_map(2.0)));
    let fm3 = model.root().face_materials(faces[3]);
    assert_eq!(
        fm3,
        FaceMaterials {
            back_map: Some(sample_map(3.0)),
            ..Default::default()
        }
    );
    assert!(model.validate().is_empty());
    undo.undo(&mut model).unwrap();
    assert_eq!(model, before);
    undo.redo(&mut model).unwrap();
    assert_eq!(
        model.root().face_materials(faces[3]).back_map,
        Some(sample_map(3.0))
    );
}

#[test]
fn set_maps_back_to_default_drops_the_entry() {
    let mut model = Model::new();
    let faces = cube(&mut model);
    let mut undo = UndoStack::new();
    set_texture_maps(
        &mut model,
        BodyId::Root,
        &[(faces[1], Side::Front, Some(sample_map(1.0)))],
        &mut undo,
        "Position Texture",
    )
    .unwrap();
    assert_eq!(model.root().face_material_entries().count(), 1);
    let n = set_texture_maps(
        &mut model,
        BodyId::Root,
        &[(faces[1], Side::Front, None)],
        &mut undo,
        "Reset Texture",
    )
    .unwrap();
    assert_eq!(n, 1);
    assert_eq!(model.root().face_material_entries().count(), 0);
    assert_eq!(
        model.root().face_materials(faces[1]),
        FaceMaterials::default()
    );
    assert_eq!(undo.undo_count(), 2);
}

#[test]
fn set_maps_counts_only_changes_and_last_one_wins() {
    let mut model = Model::new();
    let faces = cube(&mut model);
    let mut undo = UndoStack::new();
    let n = set_texture_maps(
        &mut model,
        BodyId::Root,
        &[
            (faces[1], Side::Front, Some(sample_map(1.0))),
            (faces[1], Side::Front, Some(sample_map(5.0))),
            (faces[2], Side::Back, None),
        ],
        &mut undo,
        "Position Texture",
    )
    .unwrap();
    assert_eq!(n, 1);
    assert_eq!(
        model.root().face_materials(faces[1]).front_map,
        Some(sample_map(5.0))
    );
    // the same again changes nothing and pushes no step
    let n = set_texture_maps(
        &mut model,
        BodyId::Root,
        &[(faces[1], Side::Front, Some(sample_map(5.0)))],
        &mut undo,
        "Position Texture",
    )
    .unwrap();
    assert_eq!(n, 0);
    assert_eq!(undo.undo_count(), 1);
}

#[test]
fn set_maps_errors_leave_everything_alone() {
    let mut model = Model::new();
    let faces = cube(&mut model);
    let before = model.clone();
    let mut undo = UndoStack::new();
    assert_eq!(
        set_texture_maps(&mut model, BodyId::Root, &[], &mut undo, "x"),
        Err(PaintError::Empty)
    );
    assert_eq!(
        set_texture_maps(
            &mut model,
            BodyId::Root,
            &[
                (faces[0], Side::Front, Some(sample_map(1.0))),
                (FaceId(77), Side::Front, None),
                (FaceId(78), Side::Front, None),
            ],
            &mut undo,
            "x"
        ),
        Err(PaintError::Missing(Entity::Face(FaceId(77))))
    );
    let missing = BodyId::Definition(DefinitionId(9));
    assert_eq!(
        set_texture_maps(
            &mut model,
            missing,
            &[(faces[0], Side::Front, None)],
            &mut undo,
            "x"
        ),
        Err(PaintError::Model(ModelError::MissingBody(missing)))
    );
    let mut bad = sample_map(1.0);
    bad.v.w = f64::INFINITY;
    assert_eq!(
        set_texture_maps(
            &mut model,
            BodyId::Root,
            &[
                (faces[0], Side::Front, Some(sample_map(1.0))),
                (faces[1], Side::Back, Some(bad)),
            ],
            &mut undo,
            "x"
        ),
        Err(PaintError::Model(ModelError::BadTextureMap))
    );
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 0);
}

#[test]
fn set_maps_inside_a_definition() {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    let def = model.add_definition(
        DefinitionInfo {
            name: "Panel".into(),
            group: false,
        },
        &mut log,
    );
    let body = BodyId::Definition(def);
    let face = model
        .edit_geometry(body, &mut log, |ctx, log| {
            let p = [
                v(0.0, 0.0, 0.0),
                v(1.0, 0.0, 0.0),
                v(1.0, 1.0, 0.0),
                v(0.0, 1.0, 0.0),
            ];
            let vs: Vec<_> = p.iter().map(|&q| ctx.insert_vertex(q, log)).collect();
            let mut uses = Vec::new();
            for i in 0..4 {
                let e = ctx.insert_edge(vs[i], vs[(i + 1) % 4], log).unwrap();
                uses.push(drydock_kernel::topology::EdgeUse::forward(e));
            }
            ctx.insert_face(
                drydock_kernel::topology::Face {
                    outer: drydock_kernel::topology::Loop { uses },
                    inner: vec![],
                },
                log,
            )
            .unwrap()
        })
        .unwrap();
    let mut undo = UndoStack::new();
    let n = set_texture_maps(
        &mut model,
        body,
        &[(face, Side::Back, Some(sample_map(4.0)))],
        &mut undo,
        "Position Texture",
    )
    .unwrap();
    assert_eq!(n, 1);
    assert_eq!(
        side_map(&model, body, face, Side::Back),
        Some(sample_map(4.0))
    );
    assert_eq!(model.root().face_material_entries().count(), 0);
}

// -- maps travel with face materials -----------------------------------------------------

#[test]
fn a_split_face_keeps_its_maps_on_both_halves() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    let square = [
        (v(0.0, 0.0, 0.0), v(2.0, 0.0, 0.0)),
        (v(2.0, 0.0, 0.0), v(2.0, 2.0, 0.0)),
        (v(2.0, 2.0, 0.0), v(0.0, 2.0, 0.0)),
        (v(0.0, 2.0, 0.0), v(0.0, 0.0, 0.0)),
    ];
    let mut target = Target {
        model: &mut model,
        body: BodyId::Root,
        undo: &mut undo,
        unit: Unit::Meter,
    };
    draw_segments(&mut target, &square, "Draw").unwrap();
    let face = model.root().geometry().faces().next().unwrap().0;
    set_texture_maps(
        &mut model,
        BodyId::Root,
        &[
            (face, Side::Front, Some(sample_map(1.0))),
            (face, Side::Back, Some(sample_map(2.0))),
        ],
        &mut undo,
        "Position Texture",
    )
    .unwrap();
    let mut target = Target {
        model: &mut model,
        body: BodyId::Root,
        undo: &mut undo,
        unit: Unit::Meter,
    };
    draw_segments(&mut target, &[(v(1.0, 0.0, 0.0), v(1.0, 2.0, 0.0))], "Draw").unwrap();
    let faces: Vec<FaceId> = model.root().geometry().faces().map(|(f, _)| f).collect();
    assert_eq!(faces.len(), 2);
    for f in faces {
        let fm = model.root().face_materials(f);
        assert_eq!(fm.front_map, Some(sample_map(1.0)), "{f:?}");
        assert_eq!(fm.back_map, Some(sample_map(2.0)), "{f:?}");
    }
}

// -- model checks ------------------------------------------------------------------------

fn textured(size: DVec2) -> Material {
    let mut m = Material::new("Brick", [1.0, 1.0, 1.0, 1.0]);
    m.texture = Some(Texture::new("brick.png", size));
    m
}

const BAD_SIZES: [DVec2; 5] = [
    DVec2::new(0.0, 1.0),
    DVec2::new(1.0, -2.0),
    DVec2::new(f64::NAN, 1.0),
    DVec2::new(1.0, f64::INFINITY),
    DVec2::new(-0.0, 1.0),
];

#[test]
fn model_refuses_bad_texture_sizes() {
    let mut model = Model::new();
    for size in BAD_SIZES {
        assert_eq!(
            model.apply(&ModelRecord::AddMaterial(MaterialId(0), textured(size))),
            Err(ModelError::BadTexture),
            "{size:?}"
        );
    }
    assert!(model.materials().next().is_none());
    let good = textured(DVec2::new(0.3, 2.5));
    model
        .apply(&ModelRecord::AddMaterial(MaterialId(0), good.clone()))
        .unwrap();
    for size in BAD_SIZES {
        assert_eq!(
            model.apply(&ModelRecord::SetMaterial {
                id: MaterialId(0),
                from: good.clone(),
                to: textured(size),
            }),
            Err(ModelError::BadTexture),
            "{size:?}"
        );
    }
    assert_eq!(model.material(MaterialId(0)), Some(&good));
}

#[test]
fn model_refuses_non_finite_maps() {
    let mut model = Model::new();
    let faces = cube(&mut model);
    for k in 0..8 {
        let mut bad = sample_map(1.0);
        if k < 4 {
            bad.u[k] = f64::NAN;
        } else {
            bad.v[k - 4] = f64::NEG_INFINITY;
        }
        for (front_map, back_map) in [(Some(bad), None), (None, Some(bad))] {
            assert_eq!(
                model.apply(&ModelRecord::SetFaceMaterials {
                    body: BodyId::Root,
                    face: faces[0],
                    from: FaceMaterials::default(),
                    to: FaceMaterials {
                        front_map,
                        back_map,
                        ..Default::default()
                    },
                }),
                Err(ModelError::BadTextureMap)
            );
        }
    }
    assert_eq!(model.root().face_material_entries().count(), 0);
}

#[test]
fn an_entry_with_only_a_map_is_kept() {
    let mut model = Model::new();
    let faces = cube(&mut model);
    let only_map = FaceMaterials {
        front_map: Some(sample_map(1.0)),
        ..Default::default()
    };
    model
        .apply(&ModelRecord::SetFaceMaterials {
            body: BodyId::Root,
            face: faces[0],
            from: FaceMaterials::default(),
            to: only_map,
        })
        .unwrap();
    assert_eq!(
        model.root().face_material_entries().collect::<Vec<_>>(),
        vec![(faces[0], only_map)]
    );
}

#[test]
fn create_and_edit_material_refuse_bad_textures() {
    let mut model = Model::new();
    let mut undo = UndoStack::new();
    for size in BAD_SIZES {
        assert_eq!(
            create_material(&mut model, textured(size), &mut undo, "New Material"),
            Err(PaintError::Model(ModelError::BadTexture)),
            "{size:?}"
        );
    }
    assert!(model.materials().next().is_none());
    assert_eq!(undo.undo_count(), 0);

    let id = create_material(
        &mut model,
        textured(DVec2::new(1.5, 0.75)),
        &mut undo,
        "New Material",
    )
    .unwrap();
    assert_eq!(
        model.material(id).unwrap().texture,
        Some(Texture::new("brick.png", DVec2::new(1.5, 0.75)))
    );
    let before = model.clone();
    for size in BAD_SIZES {
        assert_eq!(
            edit_material(&mut model, id, textured(size), &mut undo, "Edit Material"),
            Err(PaintError::Model(ModelError::BadTexture)),
            "{size:?}"
        );
    }
    assert_eq!(model, before);
    assert_eq!(undo.undo_count(), 1);
    // a bad color is still reported as such
    let mut bad_color = textured(DVec2::ZERO);
    bad_color.color[0] = 2.0;
    assert_eq!(
        create_material(&mut model, bad_color, &mut undo, "New Material"),
        Err(PaintError::BadColor)
    );
}
