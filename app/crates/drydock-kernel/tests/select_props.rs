// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Selection invariants (`docs/specs/m2-select-box.md`).

use drydock_kernel::model::*;
use drydock_kernel::select::*;
use drydock_kernel::topology::*;
use glam::{DMat4, DVec2, DVec3};
use proptest::prelude::*;

/// Random segments on a small integer grid: mostly in the ground plane (so faces form), some
/// rising. Drawn into the root; one definition with the same kind of geometry is placed a few
/// times.
fn model() -> impl Strategy<Value = Model> {
    let seg = (
        0..6i32,
        0..6i32,
        0..6i32,
        0..6i32,
        prop::bool::weighted(0.15),
    );
    (
        prop::collection::vec(seg.clone(), 1..14),
        prop::collection::vec(seg, 0..6),
        prop::collection::vec((-8..8i32, -8..8i32), 0..3),
    )
        .prop_map(|(root_segs, def_segs, places)| {
            let mut m = Model::new();
            let mut log = ModelEdit::new();
            let draw = |m: &mut Model, body: BodyId, segs: &[(i32, i32, i32, i32, bool)]| {
                let mut log = ModelEdit::new();
                m.edit_geometry(body, &mut log, |ctx, edit| {
                    for &(x0, y0, x1, y1, up) in segs {
                        let a = DVec3::new(x0 as f64, y0 as f64, 0.0);
                        let b = DVec3::new(x1 as f64, y1 as f64, if up { 2.0 } else { 0.0 });
                        let _ = ctx.add_edge(a, b, edit);
                    }
                })
                .unwrap();
            };
            draw(&mut m, BodyId::Root, &root_segs);
            let d = m.add_definition(
                DefinitionInfo {
                    name: "d".to_string(),
                    group: false,
                },
                &mut log,
            );
            draw(&mut m, BodyId::Definition(d), &def_segs);
            for (x, y) in places {
                let instance = Instance {
                    definition: d,
                    transform: DMat4::from_translation(DVec3::new(x as f64, y as f64, 0.0)),
                    name: String::new(),
                    material: None,
                };
                m.add_instance(BodyId::Root, instance, &mut log).unwrap();
            }
            m
        })
}

/// A slanted view: screen x mixes in a little z, so rising edges don't collapse to points.
fn view(q: DVec3) -> Option<DVec2> {
    Some(DVec2::new(q.x + 0.3 * q.z, q.y + 0.2 * q.z))
}

fn corner() -> impl Strategy<Value = DVec2> {
    (-10.0..16.0f64, -10.0..16.0f64).prop_map(|(x, y)| DVec2::new(x, y))
}

fn mode() -> impl Strategy<Value = BoxMode> {
    prop_oneof![Just(BoxMode::Window), Just(BoxMode::Crossing)]
}

fn sel(m: &Model, a: DVec2, b: DVec2, mode: BoxMode) -> Vec<Entity> {
    box_select(m, BodyId::Root, &view, a, b, mode)
}

fn is_sorted_set(v: &[Entity]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}

fn all_entities(m: &Model) -> Vec<Entity> {
    let ctx = m.root().geometry();
    let mut v: Vec<Entity> = ctx.edges().map(|(id, _)| Entity::Edge(id)).collect();
    v.extend(ctx.faces().map(|(id, _)| Entity::Face(id)));
    v.sort();
    v
}

proptest! {
    #[test]
    fn window_is_within_crossing(m in model(), a in corner(), b in corner()) {
        let w = sel(&m, a, b, BoxMode::Window);
        let c = sel(&m, a, b, BoxMode::Crossing);
        prop_assert!(is_sorted_set(&w), "{w:?}");
        prop_assert!(is_sorted_set(&c), "{c:?}");
        for e in &w {
            prop_assert!(c.contains(e), "{e:?} in window, not crossing");
        }
    }

    #[test]
    fn corner_order_does_not_matter(m in model(), a in corner(), b in corner(), mode in mode()) {
        let want = sel(&m, a, b, mode);
        prop_assert_eq!(&sel(&m, b, a, mode), &want);
        prop_assert_eq!(&sel(&m, DVec2::new(a.x, b.y), DVec2::new(b.x, a.y), mode), &want);
    }

    #[test]
    fn bigger_box_selects_no_less(
        m in model(),
        a in corner(),
        b in corner(),
        grow in (0.0..4.0f64, 0.0..4.0f64, 0.0..4.0f64, 0.0..4.0f64),
        mode in mode(),
    ) {
        let lo = a.min(b);
        let hi = a.max(b);
        let small = sel(&m, lo, hi, mode);
        let big = sel(&m, lo - DVec2::new(grow.0, grow.1), hi + DVec2::new(grow.2, grow.3), mode);
        for e in &small {
            prop_assert!(big.contains(e), "{e:?} lost when the box grew");
        }
    }

    #[test]
    fn huge_window_selects_all_root_geometry(m in model()) {
        let got = sel(&m, DVec2::splat(-100.0), DVec2::splat(100.0), BoxMode::Window);
        let geometry: Vec<Entity> =
            got.iter().copied().filter(|e| !matches!(e, Entity::Instance(_))).collect();
        prop_assert_eq!(geometry, all_entities(&m));
        let placed = m.root().instances().count();
        let def_has_geometry = m.definitions().any(|(d, _)| {
            m.body(BodyId::Definition(d))
                .is_some_and(|b| b.geometry().vertex_count() > 0)
        });
        let instances = got.iter().filter(|e| matches!(e, Entity::Instance(_))).count();
        prop_assert_eq!(instances, if def_has_geometry { placed } else { 0 });
    }

    #[test]
    fn connected_is_closed_and_contains_bounded(m in model(), pick in any::<prop::sample::Index>()) {
        let ctx: &Context = m.root().geometry();
        let all = all_entities(&m);
        prop_assume!(!all.is_empty());
        let start = all[pick.index(all.len())];
        let b = bounded(ctx, start);
        let c = connected(ctx, start);
        prop_assert!(is_sorted_set(&b), "{b:?}");
        prop_assert!(is_sorted_set(&c), "{c:?}");
        prop_assert!(b.contains(&start));
        for e in &b {
            prop_assert!(c.contains(e), "{e:?} bounded but not connected");
        }
        for e in &c {
            prop_assert_eq!(&connected(ctx, *e), &c, "from {:?}", e);
        }
    }

    #[test]
    fn bounded_is_symmetric(m in model(), pick in any::<prop::sample::Index>()) {
        // An edge's double click holds a face exactly when that face's double click holds it.
        let ctx: &Context = m.root().geometry();
        let all = all_entities(&m);
        prop_assume!(!all.is_empty());
        let start = all[pick.index(all.len())];
        for e in bounded(ctx, start) {
            if e != start {
                prop_assert!(bounded(ctx, e).contains(&start), "{start:?} -> {e:?}");
            }
        }
    }
}
