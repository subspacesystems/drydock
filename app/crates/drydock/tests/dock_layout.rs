// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-dock-layout.md`: which side panels are open, where
//! each sits, column widths, drop targets and the layout file.

use drydock::dock::{
    self, DEFAULT_WIDTH, DockLayout, EDGE_ZONE, HEADER, MAX_WIDTH, MIN_FLOAT_SIZE, MIN_WIDTH,
    PANELS, PanelId, Place, Side,
};
use eframe::egui::{Pos2, Rect, pos2, vec2};
use proptest::prelude::*;
use std::ffi::OsString;

use PanelId::{Components, EntityInfo, Materials, Shadows};

fn screen() -> Rect {
    Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0))
}

#[test]
fn default_layout() {
    let layout = DockLayout::default();
    assert_eq!(layout.docked(Side::Right), vec![EntityInfo, Materials]);
    assert!(layout.docked(Side::Left).is_empty());
    assert!(layout.floating().is_empty());
    for (slot, id) in PANELS.iter().enumerate() {
        let panel = layout.panel(*id);
        assert_eq!(panel.id, *id);
        assert_eq!(panel.open, slot < 2, "{id:?}");
        assert!(!panel.collapsed);
        assert_eq!(
            panel.place,
            Place::Docked {
                side: Side::Right,
                slot: slot as u32
            }
        );
    }
    assert_eq!(layout.width(Side::Left), DEFAULT_WIDTH);
    assert_eq!(layout.width(Side::Right), DEFAULT_WIDTH);
}

#[test]
fn opening_and_closing_keep_the_place() {
    let mut layout = DockLayout::default();
    layout.set_open(Components, true);
    assert_eq!(
        layout.docked(Side::Right),
        vec![EntityInfo, Materials, Components]
    );
    layout.set_open(EntityInfo, false);
    assert_eq!(layout.docked(Side::Right), vec![Materials, Components]);
    layout.toggle(EntityInfo);
    assert!(layout.is_open(EntityInfo));
    assert_eq!(
        layout.docked(Side::Right),
        vec![EntityInfo, Materials, Components]
    );
    layout.toggle(Components);
    assert!(!layout.is_open(Components));
}

#[test]
fn docking_moves_between_columns_and_reorders() {
    let mut layout = DockLayout::default();
    layout.dock(Materials, Side::Left, 0);
    assert_eq!(layout.docked(Side::Left), vec![Materials]);
    assert_eq!(layout.docked(Side::Right), vec![EntityInfo]);
    layout.dock(Shadows, Side::Left, 0);
    assert!(layout.is_open(Shadows), "docking opens");
    assert_eq!(layout.docked(Side::Left), vec![Shadows, Materials]);
    layout.dock(EntityInfo, Side::Left, 99);
    assert_eq!(
        layout.docked(Side::Left),
        vec![Shadows, Materials, EntityInfo]
    );
    layout.dock(EntityInfo, Side::Left, 1);
    assert_eq!(
        layout.docked(Side::Left),
        vec![Shadows, EntityInfo, Materials]
    );
    assert!(layout.docked(Side::Right).is_empty());
}

#[test]
fn a_closed_panel_keeps_its_place_among_the_others() {
    let mut layout = DockLayout::default();
    layout.set_open(Components, true);
    layout.set_open(Materials, false);
    layout.dock(Components, Side::Right, 0);
    assert_eq!(layout.docked(Side::Right), vec![Components, EntityInfo]);
    layout.set_open(Materials, true);
    assert_eq!(
        layout.docked(Side::Right),
        vec![Components, EntityInfo, Materials]
    );
}

#[test]
fn floating_opens_a_window_of_at_least_the_minimum_size() {
    let mut layout = DockLayout::default();
    layout.float(
        Shadows,
        Rect::from_min_size(pos2(300.0, 200.0), vec2(250.0, 340.0)),
    );
    assert!(layout.is_open(Shadows));
    assert_eq!(
        layout.panel(Shadows).place,
        Place::Floating {
            x: 300.0,
            y: 200.0,
            w: 250.0,
            h: 340.0
        }
    );
    assert_eq!(layout.floating(), vec![Shadows]);
    layout.float(
        Materials,
        Rect::from_min_size(pos2(10.0, 20.0), vec2(5.0, 6.0)),
    );
    assert_eq!(
        layout.panel(Materials).place,
        Place::Floating {
            x: 10.0,
            y: 20.0,
            w: MIN_FLOAT_SIZE.0,
            h: MIN_FLOAT_SIZE.1
        }
    );
    assert_eq!(layout.floating(), vec![Materials, Shadows], "PANELS order");
    assert_eq!(layout.docked(Side::Right), vec![EntityInfo]);
    let before = layout.clone();
    layout.float(
        EntityInfo,
        Rect::from_min_size(pos2(f32::NAN, 0.0), vec2(200.0, 200.0)),
    );
    assert_eq!(layout, before, "a rect that isn't finite is ignored");
}

#[test]
fn collapsing_is_a_flag() {
    let mut layout = DockLayout::default();
    layout.set_collapsed(Materials, true);
    assert!(layout.panel(Materials).collapsed);
    assert_eq!(layout.docked(Side::Right), vec![EntityInfo, Materials]);
    layout.set_collapsed(Materials, false);
    assert!(!layout.panel(Materials).collapsed);
}

#[test]
fn column_widths_are_clamped() {
    let mut layout = DockLayout::default();
    layout.set_width(Side::Left, 320.0);
    assert_eq!(layout.width(Side::Left), 320.0);
    layout.set_width(Side::Left, 10.0);
    assert_eq!(layout.width(Side::Left), MIN_WIDTH);
    layout.set_width(Side::Right, 5000.0);
    assert_eq!(layout.width(Side::Right), MAX_WIDTH);
    layout.set_width(Side::Right, f32::NAN);
    assert_eq!(layout.width(Side::Right), MAX_WIDTH, "NaN is ignored");
    layout.set_width(Side::Right, f32::NEG_INFINITY);
    assert_eq!(layout.width(Side::Right), MIN_WIDTH);
}

#[test]
fn drop_side_uses_the_edge_zones_and_open_columns() {
    let mut layout = DockLayout::default();
    let s = screen();
    let at = |x: f32| pos2(x, 300.0);
    assert_eq!(layout.drop_side(s, at(990.0)), Some(Side::Right));
    assert_eq!(
        layout.drop_side(s, at(1000.0 - DEFAULT_WIDTH + 1.0)),
        Some(Side::Right),
        "over the open right column"
    );
    assert_eq!(layout.drop_side(s, at(700.0)), None);
    assert_eq!(layout.drop_side(s, at(20.0)), Some(Side::Left));
    assert_eq!(
        layout.drop_side(s, at(EDGE_ZONE + 10.0)),
        None,
        "the left column is empty"
    );
    assert_eq!(
        layout.drop_side(s, at(-30.0)),
        Some(Side::Left),
        "past the edge still counts"
    );
    assert_eq!(layout.drop_side(s, at(1040.0)), Some(Side::Right));
    layout.dock(Materials, Side::Left, 0);
    layout.set_width(Side::Left, 300.0);
    assert_eq!(layout.drop_side(s, at(250.0)), Some(Side::Left));
    layout.set_open(EntityInfo, false);
    assert_eq!(
        layout.drop_side(s, at(800.0)),
        None,
        "no open panel on the right"
    );
    assert_eq!(layout.drop_side(s, at(970.0)), Some(Side::Right));
    let shifted = s.translate(vec2(100.0, 0.0));
    assert_eq!(layout.drop_side(shifted, at(390.0)), Some(Side::Left));
    assert_eq!(layout.drop_side(shifted, at(500.0)), None);
    assert_eq!(layout.drop_side(shifted, at(1080.0)), Some(Side::Right));
    let tiny = Rect::from_min_size(Pos2::ZERO, vec2(60.0, 100.0));
    assert_eq!(
        layout.drop_side(tiny, at(20.0)),
        Some(Side::Left),
        "nearer edge wins"
    );
    assert_eq!(layout.drop_side(tiny, at(45.0)), Some(Side::Right));
}

#[test]
fn drop_position_counts_the_middles_above() {
    assert_eq!(dock::drop_position(&[], 10.0), 0);
    let middles = [100.0, 200.0, 300.0];
    assert_eq!(dock::drop_position(&middles, 50.0), 0);
    assert_eq!(dock::drop_position(&middles, 150.0), 1);
    assert_eq!(dock::drop_position(&middles, 200.0), 1);
    assert_eq!(dock::drop_position(&middles, 250.0), 2);
    assert_eq!(dock::drop_position(&middles, 350.0), 3);
}

const EXAMPLE: &str = "modeler panels 1
width left 200
width right 300.5
panel entity-info 1 0 right 0
panel materials 1 1 right 1
panel components 0 0 left 0
panel tags 0 0 right 3
panel shadows 1 0 float 100 50 260 360
panel texture-position 0 0 right 2
";

#[test]
fn the_file_format() {
    let layout = DockLayout::parse(EXAMPLE);
    assert_eq!(layout.width(Side::Left), 200.0);
    assert_eq!(layout.width(Side::Right), 300.5);
    assert_eq!(layout.docked(Side::Right), vec![EntityInfo, Materials]);
    assert!(layout.panel(Materials).collapsed);
    assert!(!layout.is_open(Components));
    assert_eq!(
        layout.panel(Components).place,
        Place::Docked {
            side: Side::Left,
            slot: 0
        }
    );
    assert_eq!(
        layout.panel(Shadows).place,
        Place::Floating {
            x: 100.0,
            y: 50.0,
            w: 260.0,
            h: 360.0
        }
    );
    assert_eq!(layout.floating(), vec![Shadows]);
    assert_eq!(layout.to_text(), EXAMPLE);
}

#[test]
fn the_default_layout_survives_the_file() {
    let layout = DockLayout::default();
    let text = layout.to_text();
    assert_eq!(text.lines().next(), Some(HEADER));
    assert_eq!(DockLayout::parse(&text), layout);
}

#[test]
fn text_that_is_not_a_layout_gives_the_default() {
    for text in [
        "",
        "junk\n",
        "modeler panels 2\npanel materials 0 0 right 0\n",
        "panel materials 0 0 right 0\n",
        "modeler panels 1\n",
    ] {
        assert_eq!(DockLayout::parse(text), DockLayout::default(), "{text:?}");
    }
}

#[test]
fn bad_lines_are_skipped_and_missing_panels_keep_their_default() {
    let text = "modeler panels 1\r
width left 50\r
width right NaN\r
width middle 300\r
\r
panel bogus 1 0 right 0\r
panel materials 2 0 right 0\r
panel materials 1 0 middle 0\r
panel materials 1 0 left x\r
panel materials 1 0 float 1 2 inf 4\r
panel materials 1 0 float 1 2 3\r
panel shadows 1 0 left 4\r
panel shadows 1 1 left 7\r
panel components 1 0 float 5 6 7 8\r
other\r
";
    let layout = DockLayout::parse(text);
    assert_eq!(layout.width(Side::Left), MIN_WIDTH, "clamped");
    assert_eq!(
        layout.width(Side::Right),
        DEFAULT_WIDTH,
        "bad width kept the default"
    );
    assert_eq!(
        *layout.panel(Materials),
        *DockLayout::default().panel(Materials)
    );
    assert_eq!(
        *layout.panel(EntityInfo),
        *DockLayout::default().panel(EntityInfo)
    );
    let shadows = layout.panel(Shadows);
    assert!(shadows.open && shadows.collapsed, "the later line wins");
    assert_eq!(
        shadows.place,
        Place::Docked {
            side: Side::Left,
            slot: 0
        },
        "slots are renumbered"
    );
    assert_eq!(
        layout.panel(Components).place,
        Place::Floating {
            x: 5.0,
            y: 6.0,
            w: MIN_FLOAT_SIZE.0,
            h: MIN_FLOAT_SIZE.1
        }
    );
    assert_eq!(layout.docked(Side::Right), vec![EntityInfo, Materials]);
}

#[test]
fn load_and_save() {
    let dir = std::env::temp_dir().join(format!("modeler-panels-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file = dir.join("nested").join("panels");
    assert_eq!(DockLayout::load(&file), DockLayout::default());
    let mut layout = DockLayout::default();
    layout.dock(Shadows, Side::Left, 0);
    layout.set_width(Side::Left, 333.0);
    layout.float(
        Components,
        Rect::from_min_size(pos2(1.0, 2.0), vec2(300.0, 400.0)),
    );
    layout.save(&file).expect("save");
    assert_eq!(DockLayout::load(&file), layout);
    let _ = std::fs::remove_dir_all(&dir);

    let var = |name: &str| (name == "DRYDOCK_CONFIG_DIR").then(|| OsString::from("/tmp/cfg"));
    assert_eq!(
        dock::panels_file(var),
        Some(std::path::PathBuf::from("/tmp/cfg/panels"))
    );
}

#[derive(Clone, Debug)]
enum Op {
    Open(usize, bool),
    Collapse(usize, bool),
    Dock(usize, bool, usize),
    Float(usize, f32, f32, f32, f32),
    Width(bool, f32),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..4usize, any::<bool>()).prop_map(|(i, v)| Op::Open(i, v)),
        (0..4usize, any::<bool>()).prop_map(|(i, v)| Op::Collapse(i, v)),
        (0..4usize, any::<bool>(), 0..6usize).prop_map(|(i, s, p)| Op::Dock(i, s, p)),
        (
            0..4usize,
            -500.0f32..2000.0,
            -500.0f32..2000.0,
            0.0f32..900.0,
            0.0f32..900.0
        )
            .prop_map(|(i, x, y, w, h)| Op::Float(i, x, y, w, h)),
        (any::<bool>(), -100.0f32..1000.0).prop_map(|(s, w)| Op::Width(s, w)),
    ]
}

fn side(left: bool) -> Side {
    if left { Side::Left } else { Side::Right }
}

fn check(layout: &DockLayout) -> Result<(), TestCaseError> {
    let mut seen = Vec::new();
    for s in [Side::Left, Side::Right] {
        let docked = layout.docked(s);
        let mut slots: Vec<u32> = PANELS
            .iter()
            .filter_map(|id| match layout.panel(*id).place {
                Place::Docked { side, slot } if side == s => Some(slot),
                _ => None,
            })
            .collect();
        slots.sort();
        prop_assert_eq!(
            slots.clone(),
            (0..slots.len() as u32).collect::<Vec<_>>(),
            "slots in {:?}",
            s
        );
        for id in &docked {
            prop_assert!(layout.is_open(*id));
            let here = matches!(layout.panel(*id).place, Place::Docked { side, .. } if side == s);
            prop_assert!(here, "{:?} is not docked in {:?}", id, s);
        }
        seen.extend(docked);
        let w = layout.width(s);
        prop_assert!((MIN_WIDTH..=MAX_WIDTH).contains(&w));
    }
    for id in layout.floating() {
        let panel = layout.panel(id);
        prop_assert!(panel.open);
        match panel.place {
            Place::Floating { w, h, .. } => {
                prop_assert!(w >= MIN_FLOAT_SIZE.0 && h >= MIN_FLOAT_SIZE.1);
            }
            _ => prop_assert!(false, "{:?} is not floating", id),
        }
        seen.push(id);
    }
    let mut open: Vec<PanelId> = PANELS
        .iter()
        .copied()
        .filter(|id| layout.is_open(*id))
        .collect();
    let key = |id: &PanelId| PANELS.iter().position(|p| p == id);
    seen.sort_by_key(key);
    open.sort_by_key(key);
    prop_assert_eq!(seen, open, "every open panel is docked once or floating");
    prop_assert_eq!(
        &DockLayout::parse(&layout.to_text()),
        layout,
        "file round trip"
    );
    Ok(())
}

proptest! {
    #[test]
    fn edits_keep_the_layout_consistent(ops in proptest::collection::vec(op(), 0..40)) {
        let mut layout = DockLayout::default();
        check(&layout)?;
        for op in ops {
            match op {
                Op::Open(i, v) => layout.set_open(PANELS[i], v),
                Op::Collapse(i, v) => layout.set_collapsed(PANELS[i], v),
                Op::Dock(i, s, p) => {
                    layout.dock(PANELS[i], side(s), p);
                    let docked = layout.docked(side(s));
                    prop_assert_eq!(docked[p.min(docked.len() - 1)], PANELS[i]);
                }
                Op::Float(i, x, y, w, h) => {
                    layout.float(PANELS[i], Rect::from_min_size(pos2(x, y), vec2(w, h)));
                    prop_assert!(layout.floating().contains(&PANELS[i]));
                }
                Op::Width(s, w) => layout.set_width(side(s), w),
            }
            check(&layout)?;
        }
    }

    #[test]
    fn parse_never_panics(text in "(modeler panels 1\n)?(panel|width|entity-info|materials|float|left|right|[0-9 .\\-]|\n){0,60}") {
        let layout = DockLayout::parse(&text);
        check(&layout)?;
    }
}
