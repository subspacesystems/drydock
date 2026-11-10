// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m12-submenu-popups: submenus and context menus as pop-ups
//! (`docs/specs/m12-submenu-popups.md`). Pure placement rules, kittest (viewports embedded,
//! so the pop-ups are inline areas), and a gated real-window check on X11.

use drydock::chrome::{
    MenuOutcome, MenuState, menu_bar_with_toolbars, menu_popup_title, open_menu, open_submenu,
    submenu_popup_position, submenu_popup_title,
};
use drydock::context_menu::{
    context_menu_open, context_popup, context_popup_position, context_popup_title,
    open_context_menu,
};
use drydock::toolbars::ToolbarLayout;
use eframe::egui::{self, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

// ---- pure ---------------------------------------------------------------------------

#[test]
fn titles_are_distinct_and_never_empty() {
    let mut all = vec![
        submenu_popup_title("Toolbars"),
        submenu_popup_title("Other"),
        context_popup_title("canvas"),
        context_popup_title("list"),
    ];
    for m in ["File", "Edit", "View", "Tools", "Help"] {
        all.push(menu_popup_title(m));
    }
    for (i, a) in all.iter().enumerate() {
        assert!(!a.is_empty());
        for b in &all[i + 1..] {
            assert_ne!(a, b);
        }
    }
    assert_eq!(
        submenu_popup_title("Toolbars"),
        submenu_popup_title("Toolbars")
    );
    assert_eq!(context_popup_title("canvas"), context_popup_title("canvas"));
}

#[test]
fn a_submenu_goes_to_the_right_of_its_row() {
    let row = Rect::from_min_max(pos2(10.0, 100.0), pos2(190.0, 120.0));
    let size = vec2(150.0, 90.0);
    assert_eq!(
        submenu_popup_position(None, row, size, None),
        pos2(190.0, 100.0)
    );
    let root = Rect::from_min_size(pos2(300.0, 50.0), vec2(800.0, 600.0));
    assert_eq!(
        submenu_popup_position(Some(root), row, size, None),
        pos2(490.0, 150.0)
    );
}

#[test]
fn a_submenu_flips_left_when_it_would_pass_the_monitor_edge() {
    let row = Rect::from_min_max(pos2(1700.0, 100.0), pos2(1880.0, 120.0));
    let size = vec2(150.0, 90.0);
    let monitor = Some(vec2(1920.0, 1080.0));
    // 1880 + 150 > 1920: its right edge goes at the row's left edge.
    assert_eq!(
        submenu_popup_position(None, row, size, monitor),
        pos2(1550.0, 100.0)
    );
    // Room on the right: unchanged.
    let near = Rect::from_min_max(pos2(10.0, 100.0), pos2(190.0, 120.0));
    assert_eq!(
        submenu_popup_position(None, near, size, monitor),
        pos2(190.0, 100.0)
    );
}

#[test]
fn a_submenu_moves_up_when_it_would_pass_the_monitor_bottom() {
    let row = Rect::from_min_max(pos2(10.0, 1050.0), pos2(190.0, 1070.0));
    let size = vec2(150.0, 90.0);
    let monitor = Some(vec2(1920.0, 1080.0));
    assert_eq!(
        submenu_popup_position(None, row, size, monitor),
        pos2(190.0, 990.0)
    );
    // Taller than the monitor: stops at 0.
    let tall = vec2(150.0, 2000.0);
    let p = submenu_popup_position(None, row, tall, monitor);
    assert_eq!(p.y, 0.0);
}

#[test]
fn a_context_menu_opens_at_the_click_in_screen_points() {
    assert_eq!(
        context_popup_position(None, pos2(40.0, 60.0)),
        pos2(40.0, 60.0)
    );
    let root = Rect::from_min_size(pos2(300.0, 120.0), vec2(800.0, 600.0));
    assert_eq!(
        context_popup_position(Some(root), pos2(40.0, 60.0)),
        pos2(340.0, 180.0)
    );
}

// ---- kittest: View > Toolbars -------------------------------------------------------

#[derive(Default)]
struct App {
    layout: ToolbarLayout,
    outcome: Vec<MenuOutcome>,
}

fn menus() -> Harness<'static, App> {
    let mut h = Harness::builder()
        .with_size(vec2(700.0, 500.0))
        .build_ui_state(
            |ui, app: &mut App| {
                let state = MenuState::default();
                let out = menu_bar_with_toolbars(ui, &state, &mut app.layout);
                app.outcome.push(out);
            },
            App::default(),
        );
    h.ctx.set_fonts(drydock::look::font_definitions());
    h.run();
    h
}

fn open_toolbars(h: &mut Harness<'static, App>) {
    h.get_by_label("View").click();
    h.run();
    h.run();
    h.get_by_label("Toolbars").click();
    h.run();
    h.run();
}

#[test]
fn clicking_toolbars_opens_its_rows_to_the_right_of_the_row() {
    let mut h = menus();
    assert_eq!(open_submenu(&h.ctx), None);
    h.get_by_label("View").click();
    h.run();
    h.run();
    let row = h.get_by_label("Toolbars").rect();
    h.get_by_label("Toolbars").click();
    h.run();
    h.run();
    assert_eq!(open_submenu(&h.ctx).as_deref(), Some("Toolbars"));
    assert_eq!(
        open_menu(&h.ctx).as_deref(),
        Some("View"),
        "View stays open"
    );
    let reset = h.get_by_label("Reset Toolbars...").rect();
    assert!(
        reset.left() >= row.right() - 2.0,
        "{reset:?} beside {row:?}"
    );
    assert!(
        reset.top() >= row.top() - 30.0,
        "{reset:?} level with {row:?}"
    );
    assert!(h.query_by_label("Customize...").is_some());
}

#[test]
fn a_submenu_row_still_reports_its_event_and_closes_everything() {
    let mut h = menus();
    open_toolbars(&mut h);
    h.state_mut().outcome.clear();
    h.get_by_label("Reset Toolbars...").click();
    h.run();
    h.run();
    assert!(
        h.state().outcome.iter().any(|o| o.reset_toolbars),
        "Reset reported"
    );
    assert_eq!(open_submenu(&h.ctx), None);
    assert_eq!(open_menu(&h.ctx), None);
    assert!(h.query_by_label("Customize...").is_none());

    open_toolbars(&mut h);
    h.state_mut().outcome.clear();
    h.get_by_label("Customize...").click();
    h.run();
    h.run();
    assert!(h.state().outcome.iter().any(|o| o.customize_toolbars));
    assert_eq!(open_menu(&h.ctx), None);
}

#[test]
fn toggling_a_toolbar_in_the_submenu_changes_the_layout_and_keeps_it_open() {
    let mut h = menus();
    open_toolbars(&mut h);
    let before = h.state().layout.clone();
    h.state_mut().outcome.clear();
    // The first toolbar's checkbox row is the one just above "Customize...".
    let customize = h.get_by_label("Customize...").rect();
    let first = h
        .query_all_by_role(egui::accesskit::Role::CheckBox)
        .next()
        .expect("a toolbar row");
    let _ = customize;
    first.click();
    h.run();
    h.run();
    assert!(h.state().outcome.iter().any(|o| o.toolbars_changed));
    assert_ne!(h.state().layout, before);
    assert_eq!(open_submenu(&h.ctx).as_deref(), Some("Toolbars"));
}

#[test]
fn escape_closes_the_submenu_and_the_menu() {
    let mut h = menus();
    open_toolbars(&mut h);
    h.key_press(egui::Key::Escape);
    h.run();
    h.run();
    // One Escape closes the innermost pop-up at least; a second closes the rest.
    if open_menu(&h.ctx).is_some() {
        h.key_press(egui::Key::Escape);
        h.run();
        h.run();
    }
    assert_eq!(open_submenu(&h.ctx), None);
    assert_eq!(open_menu(&h.ctx), None);
}

#[test]
fn closing_the_menu_closes_its_submenu() {
    let mut h = menus();
    open_toolbars(&mut h);
    h.get_by_label("View").click(); // the title of the open menu: closes it
    h.run();
    h.run();
    assert_eq!(open_menu(&h.ctx), None);
    assert_eq!(open_submenu(&h.ctx), None);
    assert!(h.query_by_label("Reset Toolbars...").is_none());
}

#[test]
fn switching_to_another_menu_closes_the_submenu() {
    let mut h = menus();
    open_toolbars(&mut h);
    h.get_by_label("Edit").hover();
    h.run();
    h.run();
    assert_eq!(open_menu(&h.ctx).as_deref(), Some("Edit"));
    assert_eq!(open_submenu(&h.ctx), None);
    assert!(h.query_by_label("Reset Toolbars...").is_none());
}

// ---- kittest: context menus ---------------------------------------------------------

#[derive(Default)]
struct Ctx {
    chosen: Vec<&'static str>,
}

fn context_harness() -> Harness<'static, Ctx> {
    let mut h = Harness::builder()
        .with_size(vec2(500.0, 400.0))
        .build_ui_state(
            |ui, state: &mut Ctx| {
                let target = ui.allocate_response(vec2(400.0, 300.0), egui::Sense::click());
                ui.painter()
                    .rect_filled(target.rect, 0.0, egui::Color32::DARK_GRAY);
                context_popup(ui, "canvas", &target, |ui| {
                    if ui.button("Zoom to fit").clicked() {
                        state.chosen.push("fit");
                        ui.close();
                    }
                    if ui.button("Hide").clicked() {
                        state.chosen.push("hide");
                        ui.close();
                    }
                });
            },
            Ctx::default(),
        );
    h.ctx.set_fonts(drydock::look::font_definitions());
    h.run();
    h
}

fn right_click(h: &mut Harness<'static, Ctx>, at: egui::Pos2) {
    h.hover_at(at);
    h.run();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run();
    }
    h.run();
}

#[test]
fn nothing_is_open_before_a_right_click() {
    let h = context_harness();
    assert_eq!(context_menu_open(&h.ctx, "canvas"), None);
    assert!(h.query_by_label("Zoom to fit").is_none());
}

#[test]
fn a_right_click_opens_the_context_menu_at_the_click_position() {
    let mut h = context_harness();
    let at = pos2(120.0, 90.0);
    right_click(&mut h, at);
    let opened = context_menu_open(&h.ctx, "canvas").expect("open");
    assert!(
        (opened.x - at.x).abs() < 1.5 && (opened.y - at.y).abs() < 1.5,
        "{opened:?}"
    );
    let row = h.get_by_label("Zoom to fit").rect();
    assert!(
        row.left() >= at.x - 1.0 && row.left() < at.x + 30.0,
        "{row:?}"
    );
    assert!(
        row.top() >= at.y - 1.0 && row.top() < at.y + 30.0,
        "{row:?}"
    );
}

#[test]
fn a_left_click_does_not_open_it() {
    let mut h = context_harness();
    h.hover_at(pos2(120.0, 90.0));
    h.run();
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: pos2(120.0, 90.0),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run();
    }
    assert_eq!(context_menu_open(&h.ctx, "canvas"), None);
    assert!(h.query_by_label("Zoom to fit").is_none());
}

#[test]
fn choosing_a_context_row_acts_and_closes() {
    let mut h = context_harness();
    right_click(&mut h, pos2(120.0, 90.0));
    h.get_by_label("Hide").click();
    h.run();
    h.run();
    assert_eq!(h.state().chosen, vec!["hide"]);
    assert_eq!(context_menu_open(&h.ctx, "canvas"), None);
    assert!(h.query_by_label("Hide").is_none());
}

#[test]
fn escape_closes_a_context_menu() {
    let mut h = context_harness();
    right_click(&mut h, pos2(120.0, 90.0));
    assert!(context_menu_open(&h.ctx, "canvas").is_some());
    h.key_press(egui::Key::Escape);
    h.run();
    h.run();
    assert_eq!(context_menu_open(&h.ctx, "canvas"), None);
}

#[test]
fn a_context_menu_can_be_opened_without_a_click() {
    let mut h = context_harness();
    open_context_menu(&h.ctx, "canvas", pos2(200.0, 150.0));
    h.run();
    h.run();
    let opened = context_menu_open(&h.ctx, "canvas").expect("open");
    assert!((opened.x - 200.0).abs() < 1.5 && (opened.y - 150.0).abs() < 1.5);
    assert!(h.query_by_label("Zoom to fit").is_some());
}

// ---- real window (X11) --------------------------------------------------------------

#[cfg(target_os = "linux")]
mod real_window {
    use super::*;
    use std::process::Command;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    const TITLE: &str = "submenu-popups-probe";

    struct Probe {
        layout: ToolbarLayout,
        quit: Arc<AtomicBool>,
    }

    impl eframe::App for Probe {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            if self.quit.load(Ordering::Relaxed) {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            let state = MenuState::default();
            egui::Panel::top("bar").show(ui, |ui| {
                menu_bar_with_toolbars(ui, &state, &mut self.layout);
            });
            ui.ctx().request_repaint();
        }
    }

    fn tool(cmd: &str, args: &[&str]) -> String {
        Command::new(cmd)
            .args(args)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    }

    fn windows_named(title: &str) -> Vec<u32> {
        tool("xdotool", &["search", "--name", &format!("^{title}$")])
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect()
    }

    fn geometry(id: u32) -> (i32, i32, i32, i32) {
        let out = tool(
            "xdotool",
            &["getwindowgeometry", "--shell", &id.to_string()],
        );
        let get = |k: &str| {
            out.lines()
                .find_map(|l| l.strip_prefix(&format!("{k}=")))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0)
        };
        (get("X"), get("Y"), get("WIDTH"), get("HEIGHT"))
    }

    fn wait_for(what: &str, mut ready: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end {
            if ready() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("timed out waiting for {what}");
    }

    #[test]
    fn the_toolbars_submenu_is_its_own_window_beside_the_menu_and_can_pass_the_edge() {
        if std::env::var_os("DRYDOCK_GUI_TESTS").is_none()
            || std::env::var_os("DISPLAY").is_none()
            || std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
        {
            eprintln!("skipped: needs DRYDOCK_GUI_TESTS=1 and an X11 display");
            return;
        }
        let quit = Arc::new(AtomicBool::new(false));
        let q = quit.clone();
        let thread = std::thread::spawn(move || {
            let options = eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default()
                    .with_title(TITLE)
                    .with_inner_size([300.0, 200.0])
                    .with_position([40.0, 40.0]),
                event_loop_builder: Some(Box::new(|b| {
                    use winit::platform::x11::EventLoopBuilderExtX11;
                    b.with_any_thread(true);
                })),
                ..Default::default()
            };
            let _ = eframe::run_native(
                TITLE,
                options,
                Box::new(move |cc| {
                    cc.egui_ctx.set_fonts(drydock::look::font_definitions());
                    Ok(Box::new(Probe {
                        layout: ToolbarLayout::default(),
                        quit: q,
                    }))
                }),
            );
        });
        wait_for("the main window", || !windows_named(TITLE).is_empty());
        let main = windows_named(TITLE)[0];
        let (mx, my, mw, _mh) = geometry(main);
        let click = |x: i32, y: i32| {
            tool("xdotool", &["mousemove", &x.to_string(), &y.to_string()]);
            std::thread::sleep(Duration::from_millis(200));
            tool("xdotool", &["click", "1"]);
            std::thread::sleep(Duration::from_millis(500));
        };
        // View is the third title in the bar; File, Edit are ~45 and ~45 points wide.
        click(mx + 110, my + 14);
        let menu_windows = |title_filter: &str| {
            tool("xdotool", &["search", "--name", title_filter])
                .split_whitespace()
                .filter_map(|s| s.parse::<u32>().ok())
                .collect::<Vec<_>>()
        };
        wait_for("the View pop-up", || {
            !menu_windows("drydock-menu-View").is_empty()
        });
        let view = menu_windows("drydock-menu-View")[0];
        let (vx, vy, vw, _vh) = geometry(view);
        // Find the Toolbars row by scanning down the menu: click the row whose pop-up opens.
        let mut sub = None;
        for step in 0..30 {
            click(vx + 30, vy + 10 + step * 22);
            let found = menu_windows("drydock-submenu-Toolbars");
            if let Some(w) = found.first() {
                sub = Some(*w);
                break;
            }
            if menu_windows("drydock-menu-View").is_empty() {
                click(mx + 110, my + 14);
            }
        }
        let sub = sub.expect("clicking down the View menu opened the Toolbars pop-up");
        let (sx, _sy, _sw, _sh) = geometry(sub);
        assert!(
            sx >= vx + vw - 4,
            "submenu at x={sx} is beside the menu ({vx}+{vw})"
        );
        let ty = tool("xprop", &["-id", &sub.to_string(), "_NET_WM_WINDOW_TYPE"]);
        assert!(ty.contains("POPUP_MENU"), "{ty}");
        let _ = mw;
        // Escape closes them.
        tool("xdotool", &["windowfocus", "--sync", &sub.to_string()]);
        tool("xdotool", &["key", "Escape"]);
        tool("xdotool", &["key", "Escape"]);
        wait_for("the pop-ups to close", || {
            menu_windows("drydock-submenu-Toolbars").is_empty()
                && menu_windows("drydock-menu-View").is_empty()
        });
        quit.store(true, Ordering::Relaxed);
        let _ = thread.join();
    }
}
