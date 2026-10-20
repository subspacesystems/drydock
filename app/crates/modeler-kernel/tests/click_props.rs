// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Property tests for `modeler_kernel::click`.

use modeler_kernel::click::{ClickMods, apply};
use modeler_kernel::select::Entity;
use modeler_kernel::topology::{EdgeId, FaceId};
use proptest::prelude::*;
use std::collections::BTreeSet;

fn ent() -> impl Strategy<Value = Entity> {
    prop_oneof![
        (0u64..6).prop_map(|n| Entity::Edge(EdgeId(n))),
        (0u64..6).prop_map(|n| Entity::Face(FaceId(n)))
    ]
}

fn mods() -> impl Strategy<Value = ClickMods> {
    (any::<bool>(), any::<bool>(), any::<bool>()).prop_map(|(shift, ctrl, alt)| ClickMods {
        shift,
        ctrl,
        alt,
    })
}

proptest! {
    #[test]
    fn matches_set_model(cur in prop::collection::vec(ent(), 0..10), pick in prop::collection::vec(ent(), 0..10), m in mods()) {
        let c: BTreeSet<_> = cur.iter().copied().collect();
        let p: BTreeSet<_> = pick.iter().copied().collect();
        let expect: BTreeSet<_> = if m.alt {
            c.difference(&p).copied().collect()
        } else if m.ctrl {
            c.symmetric_difference(&p).copied().collect()
        } else if m.shift {
            c.union(&p).copied().collect()
        } else {
            p.clone()
        };
        let got = apply(&cur, &pick, m);
        prop_assert_eq!(got, expect.into_iter().collect::<Vec<_>>());
    }

    #[test]
    fn toggle_twice_restores(cur in prop::collection::vec(ent(), 0..10), pick in prop::collection::vec(ent(), 0..10)) {
        let m = ClickMods { shift: false, ctrl: true, alt: false };
        let once = apply(&cur, &pick, m);
        let twice = apply(&once, &pick, m);
        let norm = apply(&cur, &[], ClickMods { shift: true, ctrl: false, alt: false });
        prop_assert_eq!(twice, norm);
    }
}
