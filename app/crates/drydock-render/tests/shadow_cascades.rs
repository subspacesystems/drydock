// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m5-shadow-cascades: light cameras fitted to slices of the view
//! (`docs/specs/m5-shadow-cascades.md`). Pure math, no GPU.

use drydock_dae::Aabb;
use drydock_render::{SHADOW_CASCADES, SHADOW_SIZE, ShadowCascade, shadow_cascades, shadow_region};
use drydock_view::{Bounds, Camera, Projection, StandardView};
use glam::{DVec3, DVec4};
use proptest::prelude::*;

const FOV: f64 = 0.610_865_238_198_015_2; // 35 degrees

fn aabb(min: [f32; 3], max: [f32; 3]) -> Aabb {
    Aabb { min, max }
}

/// 200 m square ground at z = 0 with a 10 m tall thing on it: the size of a site model.
fn site() -> Aabb {
    aabb([-100.0, -100.0, 0.0], [100.0, 100.0, 10.0])
}

fn sun() -> DVec3 {
    DVec3::new(0.4, -0.3, 1.0).normalize()
}

fn camera(target: DVec3, distance: f64, yaw: f64, pitch: f64, projection: Projection) -> Camera {
    Camera {
        target,
        distance,
        yaw,
        pitch,
        fov_y: FOV,
        projection,
    }
}

/// Eye 3 m from a ground point, looking 30 degrees down.
fn close_up() -> Camera {
    camera(
        DVec3::new(5.0, -3.0, 0.0),
        3.0,
        0.7,
        30f64.to_radians(),
        Projection::Perspective,
    )
}

fn depth(cam: &Camera, p: DVec3) -> f64 {
    let (forward, _, _) = cam.basis();
    (p - cam.eye()).dot(forward)
}

fn corners(min: DVec3, max: DVec3) -> Vec<DVec3> {
    let mut out = Vec::new();
    for x in [min.x, max.x] {
        for y in [min.y, max.y] {
            for z in [min.z, max.z] {
                out.push(DVec3::new(x, y, z));
            }
        }
    }
    out
}

fn light_ndc(c: &ShadowCascade, p: DVec3) -> DVec3 {
    let v = c.light_view_proj * DVec4::new(p.x, p.y, p.z, 1.0);
    DVec3::new(v.x / v.w, v.y / v.w, v.z / v.w)
}

/// The cascade a fragment at view depth `t` uses (with room for rounding).
fn pick(cascades: &[ShadowCascade], t: f64) -> Option<&ShadowCascade> {
    let slack = 1e-9 * t.abs().max(1.0);
    cascades.iter().find(|c| c.far >= t - slack)
}

/// NDC margin that keeps a 3x3 filter (one texel each way) inside the map.
fn margin() -> f64 {
    2.0 / SHADOW_SIZE as f64
}

/// `p` is drawn somewhere in the frame (x and y inside the view, in front of a
/// perspective eye).
fn in_view(cam: &Camera, aspect: f64, p: DVec3) -> bool {
    match cam.project(p, aspect) {
        Some((ndc, _)) => ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0,
        None => false,
    }
}

fn check_covered(cam: &Camera, cascades: &[ShadowCascade], p: DVec3) {
    let t = depth(cam, p);
    let c = pick(cascades, t)
        .unwrap_or_else(|| panic!("no cascade for depth {t} (point {p}), cascades {cascades:?}"));
    let n = light_ndc(c, p);
    let lim = 1.0 - margin();
    assert!(
        n.x.abs() <= lim && n.y.abs() <= lim,
        "point {p} at depth {t} falls outside its cascade's map (ndc {n}); cascade {c:?}"
    );
    assert!(
        (0.0..=1.0).contains(&n.z),
        "point {p} outside its cascade's depth range (ndc z {})",
        n.z
    );
}

fn check_structure(cam: &Camera, bounds: Aabb, sun: DVec3, on_ground: bool, cs: &[ShadowCascade]) {
    assert!(
        (1..=SHADOW_CASCADES).contains(&cs.len()),
        "{} cascades",
        cs.len()
    );
    for pair in cs.windows(2) {
        assert!(
            pair[1].far > pair[0].far,
            "far not increasing: {:?}",
            cs.iter().map(|c| c.far).collect::<Vec<_>>()
        );
    }
    let (min, max) = shadow_region(bounds, sun, on_ground);
    let deepest = corners(min, max)
        .into_iter()
        .map(|p| depth(cam, p))
        .fold(f64::MIN, f64::max);
    let last = cs.last().unwrap().far;
    assert!(
        last >= deepest - 1e-6 * deepest.abs().max(1.0),
        "last cascade ends at {last}, region reaches depth {deepest}"
    );
    // Every caster is inside every cascade's depth range: a shadow can come from anywhere
    // in the scene, also from outside the view.
    let smin = DVec3::new(
        bounds.min[0] as f64,
        bounds.min[1] as f64,
        bounds.min[2] as f64,
    );
    let smax = DVec3::new(
        bounds.max[0] as f64,
        bounds.max[1] as f64,
        bounds.max[2] as f64,
    );
    for c in cs {
        assert!(c.texel.is_finite() && c.texel > 0.0, "texel {}", c.texel);
        for p in corners(smin, smax) {
            let z = light_ndc(c, p).z;
            assert!(
                (-1e-9..=1.0 + 1e-9).contains(&z),
                "scene corner {p} has light depth {z} in cascade {c:?}"
            );
        }
    }
}

#[test]
fn close_up_has_up_to_four_ordered_cascades_covering_the_region() {
    let cam = close_up();
    let cs = shadow_cascades(&cam, 1.5, site(), sun(), true);
    check_structure(&cam, site(), sun(), true, &cs);
    assert!(
        cs.len() >= 2,
        "a close-up of a big site needs several cascades, got {}",
        cs.len()
    );
}

#[test]
fn close_up_cascade_at_the_target_is_fine() {
    // Before this task one map covered the whole site: a texel was about 0.15 m, many
    // pixels wide in a close-up, so shadow edges showed stairs.
    let cam = close_up();
    let cs = shadow_cascades(&cam, 1.5, site(), sun(), true);
    let c = pick(&cs, cam.distance).expect("cascade at the target");
    assert!(
        c.texel <= 0.005,
        "texel {} m at the target 3 m away (want <= 5 mm)",
        c.texel
    );
}

#[test]
fn far_cascades_are_coarser() {
    let cam = close_up();
    let cs = shadow_cascades(&cam, 1.5, site(), sun(), false);
    for pair in cs.windows(2) {
        assert!(
            pair[1].texel >= pair[0].texel * 0.999,
            "texels {:?}",
            cs.iter().map(|c| c.texel).collect::<Vec<_>>()
        );
    }
}

#[test]
fn parallel_close_up_from_above_is_fine() {
    let mut cam = camera(DVec3::ZERO, 3.0, 0.0, 0.0, Projection::Parallel);
    cam.set_standard_view(StandardView::Top);
    let cs = shadow_cascades(&cam, 1.0, site(), sun(), true);
    check_structure(&cam, site(), sun(), true, &cs);
    let c = pick(&cs, depth(&cam, DVec3::ZERO)).expect("cascade at the ground");
    // The view is 3.8 m across; everything up to 10 m above the ground under it can show.
    assert!(c.texel <= 0.01, "texel {} m (want <= 10 mm)", c.texel);
}

#[test]
fn texel_is_the_world_size_of_one_map_texel() {
    let cam = close_up();
    let s = sun();
    let a = s.cross(DVec3::Z).normalize();
    let b = s.cross(a);
    for c in shadow_cascades(&cam, 1.5, site(), s, true) {
        let p = DVec3::new(1.0, 2.0, 0.5);
        let base = light_ndc(&c, p);
        for dir in [a, b] {
            let q = light_ndc(&c, p + dir * c.texel);
            let d = (q - base).truncate().length();
            let want = 2.0 / SHADOW_SIZE as f64;
            assert!(
                (d - want).abs() <= want * 1e-6,
                "one texel moves {d} in ndc, want {want}"
            );
        }
        // Moving along the sun changes only the depth.
        let q = light_ndc(&c, p + s * 0.5);
        assert!((q - base).truncate().length() < 1e-9);
        assert!(q.z < base.z, "toward the sun is nearer the light");
    }
}

#[test]
fn whole_scene_in_view_is_no_coarser_than_one_map_over_the_region() {
    let bounds = aabb([-6.0, -4.0, 0.0], [6.0, 4.0, 7.0]);
    for on_ground in [false, true] {
        for view in [StandardView::Iso, StandardView::Front, StandardView::Top] {
            for projection in [Projection::Perspective, Projection::Parallel] {
                let mut cam = camera(DVec3::ZERO, 10.0, 0.0, 0.0, projection);
                cam.set_standard_view(view);
                let (min, max) = shadow_region(bounds, sun(), on_ground);
                cam.zoom_extents(&Bounds { min, max }, 1.5);
                let cs = shadow_cascades(&cam, 1.5, bounds, sun(), on_ground);
                check_structure(&cam, bounds, sun(), on_ground, &cs);
                let limit = 1.1 * (max - min).length() / SHADOW_SIZE as f64;
                for c in &cs {
                    assert!(
                        c.texel <= limit,
                        "{view:?} {projection:?} ground {on_ground}: texel {} > {limit}",
                        c.texel
                    );
                }
            }
        }
    }
}

#[test]
fn panning_keeps_the_near_cascades_on_the_same_texel_grid() {
    // Moving the camera must not make shadow edges crawl: the near cascades keep their
    // size and move by whole texels.
    let cam = close_up();
    let before = shadow_cascades(&cam, 1.5, site(), sun(), true);
    let shift = DVec3::new(0.37, -0.21, 0.0);
    let mut moved = cam;
    moved.target += shift;
    let after = shadow_cascades(&moved, 1.5, site(), sun(), true);
    assert_eq!(before.len(), after.len());
    let n = before.len() - 1;
    let size = SHADOW_SIZE as f64;
    for i in 0..n {
        let (b, a) = (&before[i], &after[i]);
        assert!(
            (a.texel - b.texel).abs() <= b.texel * 1e-9,
            "cascade {i} texel changed {} -> {}",
            b.texel,
            a.texel
        );
        for p in [DVec3::ZERO, DVec3::new(3.3, -1.7, 0.2)] {
            let tb = (light_ndc(b, p).truncate() + 1.0) * 0.5 * size;
            let ta = (light_ndc(a, p).truncate() + 1.0) * 0.5 * size;
            let d = ta - tb;
            for v in [d.x, d.y] {
                assert!(
                    (v - v.round()).abs() < 1e-3,
                    "cascade {i}: point {p} moved {v} texels (not a whole number)"
                );
            }
        }
    }
}

#[test]
fn caster_behind_the_camera_still_shadows_the_view() {
    // Sun low behind the eye: what shadows the frame stands far behind the camera.
    let bounds = aabb([-80.0, -50.0, 0.0], [50.0, 50.0, 20.0]);
    let s = DVec3::new(-1.0, 0.0, 0.3).normalize();
    let cam = camera(
        DVec3::ZERO,
        4.0,
        std::f64::consts::PI,
        0.4,
        Projection::Perspective,
    );
    let cs = shadow_cascades(&cam, 1.0, bounds, s, false);
    check_structure(&cam, bounds, s, false, &cs);
    // A ground point in view and the top of a 20 m wall about 64 m behind the eye that
    // shades it: same light x/y, the wall nearer the light.
    let ground = DVec3::new(-1.0, 0.0, 0.0);
    assert!(in_view(&cam, 1.0, ground));
    let blocker = ground + s * (20.0 / s.z);
    let c = pick(&cs, depth(&cam, ground)).unwrap();
    let (g, w) = (light_ndc(c, ground), light_ndc(c, blocker));
    assert!((g.truncate() - w.truncate()).length() < 1e-9);
    assert!(
        w.z >= 0.0 && w.z < g.z,
        "blocker depth {} ground {}",
        w.z,
        g.z
    );
}

#[test]
fn no_cascades_without_sun() {
    let cam = close_up();
    for s in [DVec3::new(1.0, 0.0, 0.0), DVec3::new(0.3, 0.2, -0.5)] {
        assert!(shadow_cascades(&cam, 1.5, site(), s, true).is_empty());
    }
}

#[test]
fn no_cascades_when_the_region_is_behind_the_camera() {
    let bounds = aabb([-1.0, -1.0, 0.0], [1.0, 1.0, 1.0]);
    // Eye at x = 10 looking along +x, away from the box.
    let cam = camera(
        DVec3::new(20.0, 0.0, 0.5),
        10.0,
        std::f64::consts::PI,
        0.0,
        Projection::Perspective,
    );
    assert!(shadow_cascades(&cam, 1.0, bounds, sun(), false).is_empty());
}

#[test]
fn sun_length_does_not_matter() {
    let cam = close_up();
    let a = shadow_cascades(&cam, 1.5, site(), sun(), true);
    let b = shadow_cascades(&cam, 1.5, site(), sun() * 7.5, true);
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert!((x.far - y.far).abs() <= 1e-9 * x.far.abs().max(1.0));
        assert!((x.texel - y.texel).abs() <= 1e-9 * x.texel);
    }
}

fn arb_camera() -> impl Strategy<Value = (Camera, f64)> {
    (
        (-30.0..30.0f64, -30.0..30.0f64, -2.0..12.0f64),
        -0.5..2.5f64,
        -3.2..3.2f64,
        -1.5..1.5f64,
        prop::bool::ANY,
        0.5..2.5f64,
    )
        .prop_map(|((x, y, z), log_d, yaw, pitch, parallel, aspect)| {
            let projection = if parallel {
                Projection::Parallel
            } else {
                Projection::Perspective
            };
            (
                camera(
                    DVec3::new(x, y, z),
                    10f64.powf(log_d),
                    yaw,
                    pitch,
                    projection,
                ),
                aspect,
            )
        })
}

fn arb_bounds() -> impl Strategy<Value = Aabb> {
    (
        (-40.0..10.0f32, -40.0..10.0f32, -5.0..3.0f32),
        (0.5..60.0f32, 0.5..60.0f32, 0.0..25.0f32),
    )
        .prop_map(|((x, y, z), (w, d, h))| aabb([x, y, z], [x + w, y + d, z + h]))
}

fn arb_sun() -> impl Strategy<Value = DVec3> {
    (-3.2..3.2f64, 0.08..1.55f64)
        .prop_map(|(az, el)| DVec3::new(el.cos() * az.cos(), el.cos() * az.sin(), el.sin()))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Every point of the region that shows in the frame is inside the map of the cascade
    /// its depth picks, with room for the filter, and inside its depth range.
    #[test]
    fn visible_region_is_covered(
        (cam, aspect) in arb_camera(),
        bounds in arb_bounds(),
        s in arb_sun(),
        on_ground in prop::bool::ANY,
        samples in prop::collection::vec((0.0..1.0f64, 0.0..1.0f64, 0.0..1.0f64), 64),
    ) {
        let cs = shadow_cascades(&cam, aspect, bounds, s, on_ground);
        let (min, max) = shadow_region(bounds, s, on_ground);
        let mut points: Vec<DVec3> = samples
            .iter()
            .map(|&(u, v, w)| min + (max - min) * DVec3::new(u, v, w))
            .collect();
        points.extend(corners(min, max));
        if on_ground && min.z <= 0.0 && max.z >= 0.0 {
            // Ground points, where the ground shadow is drawn.
            points.extend(samples.iter().map(|&(u, v, _)| {
                DVec3::new(min.x + (max.x - min.x) * u, min.y + (max.y - min.y) * v, 0.0)
            }));
        }
        let visible: Vec<DVec3> = points.into_iter().filter(|&p| in_view(&cam, aspect, p)).collect();
        if visible.is_empty() {
            return Ok(());
        }
        prop_assert!(!cs.is_empty(), "region in view but no cascades");
        check_structure(&cam, bounds, s, on_ground, &cs);
        for p in visible {
            check_covered(&cam, &cs, p);
        }
    }
}
