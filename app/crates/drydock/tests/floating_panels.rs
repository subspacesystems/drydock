// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m11-floating-panels: a floating dock panel is its own OS window. Unit and kittest parts run
//! everywhere; the real-window part needs Xvfb, xdotool, xprop and MODELER_GUI_TESTS=1 and
//! skips itself otherwise. See docs/specs/m11-floating-panels.md.

use std::cell::RefCell;
use std::rc::Rc;

use drydock::dock::{PANELS, PanelEvent, PanelId};
use drydock::floating::{
    FloatingOutcome, floating_panel_window, floating_viewport_id, layout_position,
    raise_floating_panel, screen_position,
};
use eframe::egui::{self, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

#[test]
fn every_panel_has_its_own_viewport_id() {
    let mut ids: Vec<_> = PANELS.iter().map(|id| floating_viewport_id(*id)).collect();
    assert_eq!(
        floating_viewport_id(PanelId::Materials),
        ids[PANELS
            .iter()
            .position(|p| *p == PanelId::Materials)
            .unwrap()]
    );
    ids.push(egui::ViewportId::ROOT);
    let count = ids.len();
    ids.sort_by_key(|id| format!("{id:?}"));
    ids.dedup();
    assert_eq!(ids.len(), count, "ids are distinct and not ROOT");
}

#[test]
fn layout_and_screen_positions_are_relative_to_the_root_inner_corner() {
    let root = Rect::from_min_size(pos2(100.0, 60.0), vec2(1280.0, 800.0));
    assert_eq!(screen_position(root, pos2(30.0, 40.0)), pos2(130.0, 100.0));
    assert_eq!(layout_position(root, pos2(130.0, 100.0)), pos2(30.0, 40.0));
    let p = pos2(512.5, 77.25);
    assert_eq!(layout_position(root, screen_position(root, p)), p);
}

struct Seen {
    outcomes: Vec<FloatingOutcome>,
    content_drawn: bool,
}

fn harness(id: PanelId, rect: Rect) -> (Harness<'static>, Rc<RefCell<Seen>>) {
    let seen = Rc::new(RefCell::new(Seen {
        outcomes: Vec::new(),
        content_drawn: false,
    }));
    let state = seen.clone();
    let h = Harness::builder()
        .with_size(vec2(900.0, 700.0))
        .build_ui(move |ui| {
            let outcome = floating_panel_window(ui.ctx(), id, rect, |ui| {
                ui.label("panel body text");
                state.borrow_mut().content_drawn = true;
            });
            state.borrow_mut().outcomes.push(outcome);
        });
    (h, seen)
}

fn last(seen: &Rc<RefCell<Seen>>) -> FloatingOutcome {
    seen.borrow().outcomes.last().cloned().expect("a frame ran")
}

#[test]
fn embedded_panel_shows_header_dock_button_and_content() {
    let (mut h, seen) = harness(
        PanelId::Materials,
        Rect::from_min_size(pos2(60.0, 50.0), vec2(320.0, 260.0)),
    );
    assert!(h.ctx.embed_viewports());
    h.run();
    h.get_by_label("Move Materials panel");
    h.get_by_label("Close Materials");
    h.get_by_label("Dock panel");
    h.get_by_label("panel body text");
    assert!(seen.borrow().content_drawn);
    assert!(!last(&seen).dock);
    assert!(last(&seen).events.is_empty());
    // The header sits above the Dock panel row, which sits above the content.
    let header = h.get_by_label("Move Materials panel").rect();
    let dock = h.get_by_label("Dock panel").rect();
    let body = h.get_by_label("panel body text").rect();
    assert!(header.bottom() <= dock.top() + 0.5, "{header:?} {dock:?}");
    assert!(dock.bottom() <= body.top() + 0.5, "{dock:?} {body:?}");
}

#[test]
fn embedded_panel_reports_its_rectangle_in_layout_coordinates() {
    let rect = Rect::from_min_size(pos2(60.0, 50.0), vec2(320.0, 260.0));
    let (mut h, seen) = harness(PanelId::Shadows, rect);
    h.run();
    let got = last(&seen).rect.expect("embedded windows know their place");
    assert!((got.min - rect.min).length() < 0.5, "{got:?} vs {rect:?}");
    assert!(
        (got.size() - rect.size()).length() < 0.5,
        "{got:?} vs {rect:?}"
    );
}

#[test]
fn the_dock_button_reports_dock() {
    let (mut h, seen) = harness(
        PanelId::Tags,
        Rect::from_min_size(pos2(60.0, 50.0), vec2(320.0, 260.0)),
    );
    h.run();
    h.get_by_label("Dock panel").click();
    h.run();
    assert!(
        seen.borrow().outcomes.iter().any(|o| o.dock),
        "dock was reported"
    );
}

#[test]
fn the_header_close_button_reports_close() {
    let (mut h, seen) = harness(
        PanelId::Components,
        Rect::from_min_size(pos2(60.0, 50.0), vec2(320.0, 260.0)),
    );
    h.run();
    h.get_by_label("Close Components").click();
    h.run();
    assert!(
        seen.borrow()
            .outcomes
            .iter()
            .any(|o| o.events.contains(&PanelEvent::Close(PanelId::Components))),
        "close was reported"
    );
}

#[test]
fn raising_a_panel_does_not_fail_embedded_or_when_it_is_not_shown() {
    let (mut h, _seen) = harness(
        PanelId::Materials,
        Rect::from_min_size(pos2(60.0, 50.0), vec2(320.0, 260.0)),
    );
    h.run();
    raise_floating_panel(&h.ctx, PanelId::Materials);
    raise_floating_panel(&h.ctx, PanelId::Tags);
    h.run();
    h.get_by_label("panel body text");
}

// ---- real window -------------------------------------------------------------------

use std::path::Path;
use std::process::{Child, Command, Output};
use std::time::{Duration, Instant};

use drydock::dock::{DockLayout, Place, Side};

fn gui_tests() -> bool {
    std::env::var_os("MODELER_GUI_TESTS").is_some()
}

fn tool(cmd: &str, args: &[&str]) -> Output {
    Command::new(cmd)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("{cmd}: {e}"))
}

fn text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Ids of the X windows whose name matches the regular expression `name`.
fn windows(name: &str) -> Vec<String> {
    text(&tool("xdotool", &["search", "--name", name]))
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// (x, y) of an X window's top-left corner on the screen.
fn corner(window: &str) -> (i32, i32) {
    let out = text(&tool("xdotool", &["getwindowgeometry", "--shell", window]));
    let get = |key: &str| {
        out.lines()
            .find_map(|l| l.strip_prefix(key))
            .and_then(|v| v.trim().parse::<i32>().ok())
            .unwrap_or_else(|| panic!("no {key} in {out}"))
    };
    (get("X="), get("Y="))
}

fn wait_for(what: &str, mut ready: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(40);
    while Instant::now() < end {
        if ready() {
            return;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    panic!("timed out waiting for {what}");
}

struct Kill(Child);
impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_with_floating_materials(config: &Path) -> Kill {
    let mut layout = DockLayout::default();
    layout.float(
        PanelId::Materials,
        Rect::from_min_size(pos2(200.0, 120.0), vec2(320.0, 380.0)),
    );
    layout.save(&config.join("panels")).expect("layout saved");
    Kill(
        Command::new(env!("CARGO_BIN_EXE_drydock"))
            .env("DRYDOCK_CONFIG_DIR", config)
            .spawn()
            .expect("drydock starts"),
    )
}

fn temp_config(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("drydock-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn floating_panel_is_an_x_window_that_follows_moves_and_docks_from_its_button() {
    if !gui_tests() {
        eprintln!("skipped: MODELER_GUI_TESTS is unset");
        return;
    }
    let config = temp_config("floating-panel");
    let _app = start_with_floating_materials(&config);
    wait_for("the Materials window", || {
        !windows("^Materials$").is_empty()
    });
    let panel = windows("^Materials$")[0].clone();
    let main = windows("^Drydock$");
    assert_eq!(main.len(), 1, "one main window: {main:?}");
    assert_ne!(main[0], panel);

    // It belongs to the main window.
    let props = text(&tool("xprop", &["-id", &panel, "WM_TRANSIENT_FOR"]));
    let parent = props
        .split("window id # ")
        .nth(1)
        .unwrap_or_else(|| panic!("no WM_TRANSIENT_FOR: {props}"))
        .trim()
        .to_owned();
    assert_eq!(
        u64::from_str_radix(parent.trim_start_matches("0x"), 16).unwrap(),
        main[0].parse::<u64>().unwrap()
    );

    // Moving the window is saved, relative to the main window's corner.
    let (mx, my) = corner(&main[0]);
    tool(
        "xdotool",
        &[
            "windowmove",
            &panel,
            &(mx + 340).to_string(),
            &(my + 90).to_string(),
        ],
    );
    wait_for(
        "the moved position in the saved layout",
        || match DockLayout::load(&config.join("panels"))
            .panel(PanelId::Materials)
            .place
        {
            Place::Floating { x, y, .. } => (x - 340.0).abs() <= 3.0 && (y - 90.0).abs() <= 3.0,
            Place::Docked { .. } => false,
        },
    );

    // The Dock panel button (first widget, top-left of the window) docks it again.
    tool(
        "xdotool",
        &["mousemove", "--window", &panel, "40", "18", "click", "1"],
    );
    wait_for("the Materials window to go", || {
        windows("^Materials$").is_empty()
    });
    assert_eq!(windows("^Drydock$").len(), 1, "the main window stays");
    let saved = DockLayout::load(&config.join("panels"));
    assert!(saved.is_open(PanelId::Materials));
    assert!(matches!(
        saved.panel(PanelId::Materials).place,
        Place::Docked {
            side: Side::Right,
            ..
        }
    ));
    let _ = std::fs::remove_dir_all(&config);
}
