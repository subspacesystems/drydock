// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `docs/specs/m6-toolbar-layout.md`: which toolbars exist, what
//! they hold, where they sit, and the layout file.

use drydock::chrome::TOOL_GROUPS;
use drydock::icons::TOOLS;
use drydock::input::Tool;
use drydock::toolbars::{
    self, Edge, GROUP_NAMES, HEADER, NEW_TOOLBAR_POS, Placement, ToolbarLayout,
};
use proptest::prelude::*;
use std::ffi::OsString;

const EDGES: [Edge; 4] = [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right];

fn tools(layout: &ToolbarLayout, index: usize) -> Vec<Tool> {
    layout.toolbars()[index].tools.clone()
}

#[test]
fn default_layout_has_one_toolbar_per_group_along_the_top() {
    let layout = ToolbarLayout::default();
    assert_eq!(layout.toolbars().len(), 5);
    for (index, toolbar) in layout.toolbars().iter().enumerate() {
        assert_eq!(toolbar.name, GROUP_NAMES[index]);
        assert_eq!(toolbar.tools, TOOL_GROUPS[index].to_vec());
        assert!(toolbar.visible);
        assert!(matches!(
            toolbar.placement,
            Placement::Docked {
                edge: Edge::Top,
                ..
            }
        ));
    }
    assert_eq!(layout.docked(Edge::Top), vec![0, 1, 2, 3, 4]);
    for edge in [Edge::Bottom, Edge::Left, Edge::Right] {
        assert!(layout.docked(edge).is_empty());
    }
    assert!(layout.floating().is_empty());
}

#[test]
fn hiding_takes_a_toolbar_off_its_edge_until_shown_again() {
    let mut layout = ToolbarLayout::default();
    layout.set_visible(1, false);
    assert!(!layout.toolbars()[1].visible);
    assert_eq!(layout.docked(Edge::Top), vec![0, 2, 3, 4]);
    layout.set_visible(1, true);
    assert_eq!(layout.docked(Edge::Top), vec![0, 1, 2, 3, 4]);
    let before = layout.clone();
    layout.set_visible(17, false);
    assert_eq!(layout, before, "out of range does nothing");
}

#[test]
fn docking_moves_a_toolbar_to_an_edge_at_a_position() {
    let mut layout = ToolbarLayout::default();
    layout.dock(4, Edge::Left, 0);
    assert_eq!(layout.docked(Edge::Left), vec![4]);
    assert_eq!(layout.docked(Edge::Top), vec![0, 1, 2, 3]);
    layout.dock(2, Edge::Left, 0);
    assert_eq!(layout.docked(Edge::Left), vec![2, 4]);
    layout.dock(0, Edge::Left, 99);
    assert_eq!(
        layout.docked(Edge::Left),
        vec![2, 4, 0],
        "past the end appends"
    );
    layout.dock(0, Edge::Left, 0);
    assert_eq!(layout.docked(Edge::Left), vec![0, 2, 4]);
    assert_eq!(layout.docked(Edge::Top), vec![1, 3]);
}

#[test]
fn docking_on_the_same_edge_reorders() {
    let mut layout = ToolbarLayout::default();
    layout.dock(4, Edge::Top, 0);
    assert_eq!(layout.docked(Edge::Top), vec![4, 0, 1, 2, 3]);
    layout.dock(4, Edge::Top, 2);
    assert_eq!(layout.docked(Edge::Top), vec![0, 1, 4, 2, 3]);
    layout.dock(0, Edge::Top, 4);
    assert_eq!(layout.docked(Edge::Top), vec![1, 4, 2, 3, 0]);
}

#[test]
fn a_hidden_toolbar_keeps_its_place_among_the_others() {
    let mut layout = ToolbarLayout::default();
    layout.set_visible(2, false);
    layout.dock(4, Edge::Top, 0);
    assert_eq!(layout.docked(Edge::Top), vec![4, 0, 1, 3]);
    layout.set_visible(2, true);
    assert_eq!(layout.docked(Edge::Top), vec![4, 0, 1, 2, 3]);
}

#[test]
fn docking_or_floating_shows_a_hidden_toolbar() {
    let mut layout = ToolbarLayout::default();
    layout.set_visible(3, false);
    layout.dock(3, Edge::Right, 0);
    assert!(layout.toolbars()[3].visible);
    assert_eq!(layout.docked(Edge::Right), vec![3]);
    layout.set_visible(1, false);
    layout.float(1, 300.0, 200.0);
    assert!(layout.toolbars()[1].visible);
    assert_eq!(
        layout.toolbars()[1].placement,
        Placement::Floating { x: 300.0, y: 200.0 }
    );
    assert_eq!(layout.floating(), vec![1]);
    assert_eq!(layout.docked(Edge::Top), vec![0, 2, 4]);
}

#[test]
fn add_tool_inserts_or_moves_within_a_toolbar() {
    let mut layout = ToolbarLayout::default();
    use Tool::*;
    assert_eq!(tools(&layout, 1), vec![Line, Rectangle, Circle, Arc, Arc3]);
    layout.add_tool(1, Move, 0);
    assert_eq!(
        tools(&layout, 1),
        vec![Move, Line, Rectangle, Circle, Arc, Arc3]
    );
    layout.add_tool(1, Line, 99);
    assert_eq!(
        tools(&layout, 1),
        vec![Move, Rectangle, Circle, Arc, Arc3, Line]
    );
    layout.add_tool(1, Circle, 0);
    assert_eq!(
        tools(&layout, 1),
        vec![Circle, Move, Rectangle, Arc, Arc3, Line]
    );
    assert_eq!(
        tools(&layout, 2),
        vec![PushPull, Offset, FollowMe, Move, Rotate, Scale],
        "a tool can sit on two toolbars"
    );
    let before = layout.clone();
    layout.add_tool(9, Zoom, 0);
    assert_eq!(layout, before);
}

#[test]
fn remove_and_move_tools() {
    let mut layout = ToolbarLayout::default();
    use Tool::*;
    layout.remove_tool(1, 0);
    assert_eq!(tools(&layout, 1), vec![Rectangle, Circle, Arc, Arc3]);
    layout.remove_tool(1, 99);
    assert_eq!(tools(&layout, 1), vec![Rectangle, Circle, Arc, Arc3]);
    layout.move_tool(1, 0, 2);
    assert_eq!(tools(&layout, 1), vec![Circle, Arc, Rectangle, Arc3]);
    layout.move_tool(1, 0, 99);
    assert_eq!(tools(&layout, 1), vec![Arc, Rectangle, Arc3, Circle]);
    layout.move_tool(1, 7, 0);
    assert_eq!(tools(&layout, 1), vec![Arc, Rectangle, Arc3, Circle]);
    layout.remove_tool(0, 0);
    assert!(tools(&layout, 0).is_empty(), "a toolbar may be empty");
}

#[test]
fn add_toolbar_appends_an_empty_floating_toolbar_with_a_free_name() {
    let mut layout = ToolbarLayout::default();
    let index = layout.add_toolbar("  Mine ");
    assert_eq!(index, 5);
    let toolbar = &layout.toolbars()[5];
    assert_eq!(toolbar.name, "Mine");
    assert!(toolbar.tools.is_empty());
    assert!(toolbar.visible);
    assert_eq!(
        toolbar.placement,
        Placement::Floating {
            x: NEW_TOOLBAR_POS.0,
            y: NEW_TOOLBAR_POS.1
        }
    );
    assert_eq!(layout.floating(), vec![5]);
    let names = |layout: &ToolbarLayout| -> Vec<String> {
        layout.toolbars().iter().map(|t| t.name.clone()).collect()
    };
    layout.add_toolbar("Mine");
    layout.add_toolbar("");
    layout.add_toolbar("   ");
    layout.add_toolbar("Drawing");
    layout.add_toolbar("two\nlines");
    assert_eq!(
        names(&layout)[5..],
        [
            "Mine",
            "Mine 2",
            "Toolbar",
            "Toolbar 2",
            "Drawing 2",
            "two lines"
        ]
    );
}

#[test]
fn remove_toolbar_drops_it_and_shifts_the_rest() {
    let mut layout = ToolbarLayout::default();
    layout.remove_toolbar(0);
    assert_eq!(layout.toolbars().len(), 4);
    assert_eq!(layout.toolbars()[0].name, "Drawing");
    assert_eq!(layout.docked(Edge::Top), vec![0, 1, 2, 3]);
    let before = layout.clone();
    layout.remove_toolbar(4);
    assert_eq!(layout, before);
}

#[test]
fn reset_restores_the_default() {
    let mut layout = ToolbarLayout::default();
    layout.dock(1, Edge::Bottom, 0);
    layout.add_toolbar("Mine");
    layout.remove_tool(0, 0);
    layout.reset();
    assert_eq!(layout, ToolbarLayout::default());
}

#[test]
fn tool_keys() {
    let keys = [
        "select",
        "line",
        "rectangle",
        "circle",
        "arc",
        "arc3",
        "pushpull",
        "offset",
        "followme",
        "move",
        "rotate",
        "scale",
        "tape",
        "erase",
        "paint",
        "orbit",
        "pan",
        "zoom",
    ];
    for (tool, key) in TOOLS.iter().zip(keys) {
        assert_eq!(toolbars::tool_key(*tool), key);
        assert_eq!(toolbars::tool_from_key(key), Some(*tool));
    }
    assert_eq!(toolbars::tool_from_key("bogus"), None);
    assert_eq!(toolbars::tool_from_key("Line"), None, "keys are lowercase");
}

const EXAMPLE: &str = "modeler toolbars 1
toolbar 1 top 0 Select
tools select
toolbar 0 left 0 Drawing
tools line circle
toolbar 1 float 10.5 -3 My tools
tools zoom pan
";

#[test]
fn the_file_format() {
    let layout = ToolbarLayout::parse(EXAMPLE);
    let t = layout.toolbars();
    assert_eq!(t.len(), 3);
    assert_eq!(t[0].name, "Select");
    assert!(t[0].visible);
    assert_eq!(
        t[0].placement,
        Placement::Docked {
            edge: Edge::Top,
            slot: 0
        }
    );
    assert_eq!(t[0].tools, vec![Tool::Select]);
    assert_eq!(t[1].name, "Drawing");
    assert!(!t[1].visible);
    assert_eq!(
        t[1].placement,
        Placement::Docked {
            edge: Edge::Left,
            slot: 0
        }
    );
    assert_eq!(t[1].tools, vec![Tool::Line, Tool::Circle]);
    assert_eq!(t[2].name, "My tools");
    assert_eq!(t[2].placement, Placement::Floating { x: 10.5, y: -3.0 });
    assert_eq!(t[2].tools, vec![Tool::Zoom, Tool::Pan]);
    assert_eq!(layout.to_text(), EXAMPLE);
}

#[test]
fn the_default_layout_survives_the_file() {
    let layout = ToolbarLayout::default();
    let text = layout.to_text();
    assert_eq!(text.lines().next(), Some(HEADER));
    assert_eq!(ToolbarLayout::parse(&text), layout);
}

#[test]
fn text_that_is_not_a_layout_gives_the_default() {
    for text in [
        "",
        "garbage\n",
        "modeler toolbars 2\ntoolbar 1 top 0 X\n",
        "toolbar 1 top 0 X\ntools line\n",
    ] {
        assert_eq!(
            ToolbarLayout::parse(text),
            ToolbarLayout::default(),
            "{text:?}"
        );
    }
}

#[test]
fn a_header_alone_is_an_empty_layout() {
    assert!(
        ToolbarLayout::parse("modeler toolbars 1\n")
            .toolbars()
            .is_empty()
    );
}

#[test]
fn bad_lines_are_skipped() {
    let text = "modeler toolbars 1\r
tools line\r
\r
toolbar 1 middle 0 Bad edge\r
tools line\r
toolbar 1 top x Bad slot\r
tools line\r
toolbar 1 float NaN 3 Bad float\r
tools line\r
toolbar 2 top 0 Bad visible\r
tools line\r
toolbar 1 top 0\r
tools line\r
toolbar 1 right 5 Kept\r
tools line bogus line zoom\r
something else\r
toolbar 1 right 2 Also kept\r
";
    let layout = ToolbarLayout::parse(text);
    let t = layout.toolbars();
    assert_eq!(t.len(), 2, "{t:?}");
    assert_eq!(t[0].name, "Kept");
    assert_eq!(t[0].tools, vec![Tool::Line, Tool::Zoom]);
    assert_eq!(t[1].name, "Also kept");
    assert!(t[1].tools.is_empty());
    assert_eq!(layout.docked(Edge::Right), vec![1, 0], "ordered by slot");
    assert_eq!(
        t[1].placement,
        Placement::Docked {
            edge: Edge::Right,
            slot: 0
        },
        "slots are renumbered from 0"
    );
}

#[test]
fn load_and_save() {
    let dir = std::env::temp_dir().join(format!("modeler-toolbars-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file = dir.join("nested").join("toolbars");
    assert_eq!(ToolbarLayout::load(&file), ToolbarLayout::default());
    let mut layout = ToolbarLayout::default();
    layout.dock(0, Edge::Bottom, 0);
    layout.add_toolbar("Mine");
    layout.add_tool(5, Tool::Paint, 0);
    layout.save(&file).expect("save");
    assert_eq!(ToolbarLayout::load(&file), layout);
    let _ = std::fs::remove_dir_all(&dir);

    let var = |name: &str| (name == "DRYDOCK_CONFIG_DIR").then(|| OsString::from("/tmp/cfg"));
    assert_eq!(
        toolbars::toolbars_file(var),
        Some(std::path::PathBuf::from("/tmp/cfg/toolbars"))
    );
}

#[derive(Clone, Debug)]
enum Op {
    Visible(usize, bool),
    Dock(usize, usize, usize),
    Float(usize, f32, f32),
    AddTool(usize, usize, usize),
    RemoveTool(usize, usize),
    MoveTool(usize, usize, usize),
    AddToolbar(String),
    RemoveToolbar(usize),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..8usize, any::<bool>()).prop_map(|(i, v)| Op::Visible(i, v)),
        (0..8usize, 0..4usize, 0..8usize).prop_map(|(i, e, p)| Op::Dock(i, e, p)),
        (0..8usize, -1000.0f32..1000.0, -1000.0f32..1000.0)
            .prop_map(|(i, x, y)| Op::Float(i, x, y)),
        (0..8usize, 0..16usize, 0..8usize).prop_map(|(i, t, a)| Op::AddTool(i, t, a)),
        (0..8usize, 0..8usize).prop_map(|(i, a)| Op::RemoveTool(i, a)),
        (0..8usize, 0..8usize, 0..8usize).prop_map(|(i, f, t)| Op::MoveTool(i, f, t)),
        "[a-z ]{0,6}".prop_map(Op::AddToolbar),
        (0..8usize).prop_map(Op::RemoveToolbar),
    ]
}

fn check(layout: &ToolbarLayout) -> Result<(), TestCaseError> {
    let t = layout.toolbars();
    for toolbar in t {
        for (k, tool) in toolbar.tools.iter().enumerate() {
            prop_assert!(
                !toolbar.tools[k + 1..].contains(tool),
                "duplicate tool in {toolbar:?}"
            );
        }
    }
    let mut seen = Vec::new();
    for edge in EDGES {
        let docked = layout.docked(edge);
        let expected: Vec<usize> = (0..t.len())
            .filter(|i| {
                t[*i].visible
                    && matches!(t[*i].placement, Placement::Docked { edge: e, .. } if e == edge)
            })
            .collect();
        let mut sorted = docked.clone();
        sorted.sort();
        prop_assert_eq!(&sorted, &expected, "docked({:?})", edge);
        let mut slots: Vec<u32> = t
            .iter()
            .filter_map(|toolbar| match toolbar.placement {
                Placement::Docked { edge: e, slot } if e == edge => Some(slot),
                _ => None,
            })
            .collect();
        slots.sort();
        prop_assert_eq!(
            slots.clone(),
            (0..slots.len() as u32).collect::<Vec<_>>(),
            "slots on {:?}",
            edge
        );
        seen.extend(docked);
    }
    let floating = layout.floating();
    let expected: Vec<usize> = (0..t.len())
        .filter(|i| t[*i].visible && matches!(t[*i].placement, Placement::Floating { .. }))
        .collect();
    prop_assert_eq!(&floating, &expected);
    seen.extend(floating);
    seen.sort();
    let visible: Vec<usize> = (0..t.len()).filter(|i| t[*i].visible).collect();
    prop_assert_eq!(
        seen,
        visible,
        "every shown toolbar is on one edge or floating"
    );
    prop_assert_eq!(
        &ToolbarLayout::parse(&layout.to_text()),
        layout,
        "file round trip"
    );
    Ok(())
}

proptest! {
    #[test]
    fn edits_keep_the_layout_consistent(ops in proptest::collection::vec(op(), 0..40)) {
        let mut layout = ToolbarLayout::default();
        check(&layout)?;
        for op in ops {
            match op {
                Op::Visible(i, v) => layout.set_visible(i, v),
                Op::Dock(i, e, p) => {
                    layout.dock(i, EDGES[e], p);
                    if i < layout.toolbars().len() {
                        let docked = layout.docked(EDGES[e]);
                        prop_assert_eq!(docked[p.min(docked.len() - 1)], i);
                    }
                }
                Op::Float(i, x, y) => {
                    layout.float(i, x, y);
                    if i < layout.toolbars().len() {
                        prop_assert_eq!(layout.toolbars()[i].placement, Placement::Floating { x, y });
                    }
                }
                Op::AddTool(i, t, a) => {
                    layout.add_tool(i, TOOLS[t], a);
                    if i < layout.toolbars().len() {
                        let tools = &layout.toolbars()[i].tools;
                        prop_assert_eq!(tools[a.min(tools.len() - 1)], TOOLS[t]);
                    }
                }
                Op::RemoveTool(i, a) => layout.remove_tool(i, a),
                Op::MoveTool(i, f, t) => layout.move_tool(i, f, t),
                Op::AddToolbar(name) => {
                    let index = layout.add_toolbar(&name);
                    prop_assert_eq!(index, layout.toolbars().len() - 1);
                    let names: Vec<&str> = layout.toolbars().iter().map(|t| t.name.as_str()).collect();
                    prop_assert!(!names[..index].contains(&names[index]), "name {} is taken", names[index]);
                }
                Op::RemoveToolbar(i) => layout.remove_toolbar(i),
            }
            check(&layout)?;
        }
    }

    #[test]
    fn parse_never_panics(text in "(modeler toolbars 1\n)?(toolbar|tools|[0-9a-z .\\-]|\n){0,80}") {
        let layout = ToolbarLayout::parse(&text);
        check(&layout)?;
    }
}
