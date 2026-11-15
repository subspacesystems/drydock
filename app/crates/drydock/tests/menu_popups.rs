// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m12-menu-popups: the top-level menus open as pop-ups (`docs/specs/m12-menu-popups.md`).
//! Pure placement rules, kittest (viewports embedded, so the pop-ups are inline areas), and a
//! gated real-window check on X11.

use drydock::chrome::{
    MenuState, menu_bar, menu_popup_position, menu_popup_title, open_menu, request_menu,
};
use drydock::input::Action;
use eframe::egui::{self, Rect, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::cell::RefCell;
use std::rc::Rc;

type Seen = Rc<RefCell<Vec<Action>>>;

const MENUS: [&str; 5] = ["File", "Edit", "View", "Tools", "Help"];

fn harness(seen: &Seen) -> Harness<'static> {
    let seen = seen.clone();
    let state = MenuState::default();
    let mut h = Harness::builder()
        .with_size(egui::vec2(700.0, 500.0))
        .build_ui(move |ui| {
            if let Some(action) = menu_bar(ui, &state) {
                seen.borrow_mut().push(action);
            }
        });
    h.ctx.set_fonts(drydock::look::font_definitions());
    h.run();
    h
}

fn current(h: &Harness<'_>) -> Option<String> {
    open_menu(&h.ctx)
}

/// One row label that only the menu `label` has.
fn marker(label: &str) -> &'static str {
    match label {
        "File" => "New",
        "Edit" => "Undo",
        "View" => "Switch to Parallel Projection",
        "Tools" => "Select",
        _ => "About Drydock...",
    }
}

fn shows(h: &Harness<'_>, label: &str) -> bool {
    h.query_by_label(marker(label)).is_some()
}

// ---- pure ---------------------------------------------------------------------------

#[test]
fn the_popup_title_is_distinct_per_menu_and_never_empty() {
    let titles: Vec<String> = MENUS.iter().map(|m| menu_popup_title(m)).collect();
    for (i, a) in titles.iter().enumerate() {
        assert!(!a.is_empty());
        for b in &titles[i + 1..] {
            assert_ne!(a, b);
        }
    }
    assert_eq!(menu_popup_title("File"), menu_popup_title("File"));
}

#[test]
fn the_popup_goes_under_the_title_in_screen_points() {
    let title = Rect::from_min_max(pos2(10.0, 4.0), pos2(52.0, 24.0));
    assert_eq!(menu_popup_position(None, title), pos2(10.0, 24.0));
    let root = Rect::from_min_size(pos2(300.0, 120.0), egui::vec2(800.0, 600.0));
    assert_eq!(menu_popup_position(Some(root), title), pos2(310.0, 144.0));
}

// ---- kittest ------------------------------------------------------------------------

#[test]
fn nothing_is_open_before_a_click() {
    let h = harness(&Seen::default());
    assert_eq!(current(&h), None);
    for m in MENUS {
        assert!(!shows(&h, m), "{m} is closed");
    }
}

#[test]
fn clicking_a_title_opens_its_menu_and_clicking_it_again_closes_it() {
    for m in MENUS {
        let mut h = harness(&Seen::default());
        h.get_by_label(m).click();
        h.run();
        h.run();
        assert_eq!(current(&h).as_deref(), Some(m));
        assert!(shows(&h, m), "{m} lists its rows");
        h.get_by_label(m).click();
        h.run();
        h.run();
        assert_eq!(current(&h), None, "{m} closes on a second click");
        assert!(!shows(&h, m));
    }
}

#[test]
fn an_open_menu_sits_below_its_title() {
    let mut h = harness(&Seen::default());
    let title = h.get_by_label("Edit").rect();
    h.get_by_label("Edit").click();
    h.run();
    h.run();
    let row = h.get_by_label(marker("Edit")).rect();
    assert!(row.top() >= title.bottom() - 1.0, "{row:?} under {title:?}");
    assert!(row.left() >= title.left() - 1.0);
    assert!(row.left() < title.left() + 40.0);
}

#[test]
fn hovering_another_title_switches_the_open_menu() {
    let mut h = harness(&Seen::default());
    h.get_by_label("File").click();
    h.run();
    h.run();
    assert!(shows(&h, "File"));
    h.get_by_label("Tools").hover();
    h.run();
    h.run();
    assert_eq!(current(&h).as_deref(), Some("Tools"));
    assert!(shows(&h, "Tools"));
    assert!(!shows(&h, "File"), "File went away");
    h.get_by_label("Help").hover();
    h.run();
    h.run();
    assert_eq!(current(&h).as_deref(), Some("Help"));
    assert!(!shows(&h, "Tools"));
}

#[test]
fn hovering_a_title_with_nothing_open_opens_nothing() {
    let mut h = harness(&Seen::default());
    h.get_by_label("Edit").hover();
    h.run();
    h.run();
    assert_eq!(current(&h), None);
}

#[test]
fn escape_closes_the_open_menu() {
    let mut h = harness(&Seen::default());
    h.get_by_label("View").click();
    h.run();
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    h.run();
    assert_eq!(current(&h), None);
    assert!(!shows(&h, "View"));
}

#[test]
fn the_arrow_keys_move_to_the_next_and_previous_menu() {
    let mut h = harness(&Seen::default());
    h.get_by_label("Edit").click();
    h.run();
    h.run();
    h.key_press(egui::Key::ArrowRight);
    h.run();
    h.run();
    assert_eq!(current(&h).as_deref(), Some("View"));
    h.key_press(egui::Key::ArrowLeft);
    h.run();
    h.key_press(egui::Key::ArrowLeft);
    h.run();
    h.run();
    assert_eq!(current(&h).as_deref(), Some("File"));
    h.key_press(egui::Key::ArrowLeft);
    h.run();
    h.run();
    assert_eq!(current(&h).as_deref(), Some("Help"), "wraps around");
}

#[test]
fn choosing_an_item_reports_its_action_and_closes_the_menu() {
    let seen = Seen::default();
    let mut h = harness(&seen);
    h.get_by_label("File").click();
    h.run();
    h.run();
    h.get_by_label("New").click();
    h.run();
    h.run();
    assert_eq!(seen.borrow().as_slice(), &[Action::New]);
    assert_eq!(current(&h), None);
    assert!(!shows(&h, "File"));
}

#[test]
fn a_menu_can_be_requested_without_a_click() {
    let mut h = harness(&Seen::default());
    request_menu(&h.ctx, "Tools");
    h.run();
    h.run();
    assert_eq!(current(&h).as_deref(), Some("Tools"));
    assert!(shows(&h, "Tools"));
    request_menu(&h.ctx, "No such menu");
    h.run();
    h.run();
    assert_eq!(
        current(&h).as_deref(),
        Some("Tools"),
        "an unknown label changes nothing"
    );
}

// ---- real window --------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod real {
    use super::*;
    use std::process::Command;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    const ROOT: &str = "menu-probe-root";

    type Request = Arc<std::sync::Mutex<Option<&'static str>>>;

    struct App {
        request: Request,
        quit: Arc<AtomicBool>,
        state: MenuState<'static>,
    }

    impl eframe::App for App {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            let ctx = ui.ctx().clone();
            egui::Panel::top("menu").show(ui, |ui| {
                menu_bar(ui, &self.state);
            });
            if let Some(label) = self.request.lock().unwrap().take() {
                request_menu(&ctx, label);
            }
            if self.quit.load(Ordering::SeqCst) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn run_probe(request: Request, quit: Arc<AtomicBool>) -> Result<(), String> {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title(ROOT)
                .with_position(egui::pos2(0.0, 0.0))
                // Short: the Tools menu is taller than the window.
                .with_inner_size([320.0, 160.0]),
            event_loop_builder: Some(Box::new(|builder| {
                use winit::platform::x11::EventLoopBuilderExtX11;
                builder.with_x11().with_any_thread(true);
            })),
            ..Default::default()
        };
        eframe::run_native(
            "menu-probe",
            options,
            Box::new(move |cc| {
                cc.egui_ctx.set_fonts(drydock::look::font_definitions());
                Ok(Box::new(App {
                    request,
                    quit,
                    state: MenuState::default(),
                }))
            }),
        )
        .map_err(|err| err.to_string())
    }

    fn tool(cmd: &str, args: &[&str]) -> String {
        let out = Command::new(cmd)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("{cmd}: {e}"));
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn find_window(title: &str) -> Option<u32> {
        tool(
            "xdotool",
            &["search", "--onlyvisible", "--name", &format!("^{title}$")],
        )
        .lines()
        .next()
        .and_then(|line| line.trim().parse().ok())
    }

    fn geometry(id: u32) -> (i32, i32, i32, i32) {
        let text = tool(
            "xdotool",
            &["getwindowgeometry", "--shell", &id.to_string()],
        );
        let get = |key: &str| -> i32 {
            text.lines()
                .find_map(|l| l.strip_prefix(&format!("{key}=")))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or_else(|| panic!("no {key} in {text:?}"))
        };
        (get("X"), get("Y"), get("WIDTH"), get("HEIGHT"))
    }

    fn wait_for(what: &str, mut ready: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(40);
        while Instant::now() < end {
            if ready() {
                return;
            }
            thread::sleep(Duration::from_millis(250));
        }
        panic!("timed out waiting for {what}");
    }

    #[test]
    fn a_menu_is_a_pop_up_window_below_its_title_that_reaches_past_the_window() {
        if std::env::var_os("DRYDOCK_GUI_TESTS").is_none()
            || std::env::var_os("DISPLAY").is_none()
            || std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
        {
            eprintln!("skipped: needs DRYDOCK_GUI_TESTS=1 and an X11 display");
            return;
        }
        let request: Request = Arc::default();
        let quit = Arc::new(AtomicBool::new(false));
        let spawn = || {
            let (request, quit) = (Arc::clone(&request), Arc::clone(&quit));
            thread::spawn(move || run_probe(request, quit))
        };
        let mut app = spawn();
        let deadline = Instant::now() + Duration::from_secs(40);
        loop {
            if find_window(ROOT).is_some() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for the root window"
            );
            if app.is_finished() {
                let _ = app.join();
                app = spawn();
            }
            thread::sleep(Duration::from_millis(250));
        }
        thread::sleep(Duration::from_secs(1));
        let root = find_window(ROOT).unwrap();
        let (_, _, root_w, root_h) = geometry(root);

        let tools = menu_popup_title("Tools");
        let file = menu_popup_title("File");
        *request.lock().unwrap() = Some("Tools");
        wait_for("the Tools pop-up", || find_window(&tools).is_some());
        let popup = find_window(&tools).unwrap();
        assert_ne!(popup, root, "a separate OS window");
        thread::sleep(Duration::from_secs(1));
        let (x, y, w, h) = geometry(popup);
        assert!(y > 0 && y < 60, "just below the menu bar: y = {y}");
        assert!(x >= 0 && x < root_w, "under a title: x = {x}");
        assert!(
            y + h > root_h,
            "reaches past the short window: {w}x{h} at {x},{y}"
        );
        let kind = tool("xprop", &["-id", &popup.to_string(), "_NET_WM_WINDOW_TYPE"]);
        assert!(kind.contains("_NET_WM_WINDOW_TYPE_POPUP_MENU"), "{kind}");

        // Another menu replaces it.
        *request.lock().unwrap() = Some("File");
        wait_for("the File pop-up", || find_window(&file).is_some());
        wait_for("the Tools pop-up to go", || find_window(&tools).is_none());

        // Escape closes the open one. Focus and send again: the pop-up may still be being
        // revealed when it first shows, so one lost key must not fail the test.
        let popup = find_window(&file).unwrap();
        wait_for("Escape to close the menu", || {
            if find_window(&file).is_none() {
                return true;
            }
            tool("xdotool", &["windowfocus", "--sync", &popup.to_string()]);
            tool("xdotool", &["key", "Escape"]);
            false
        });
        assert!(find_window(ROOT).is_some());

        quit.store(true, Ordering::SeqCst);
        let end = Instant::now() + Duration::from_secs(20);
        while !app.is_finished() && Instant::now() < end {
            thread::sleep(Duration::from_millis(100));
        }
        if app.is_finished() {
            let result = app.join().expect("probe thread does not panic");
            assert!(result.is_ok(), "probe app failed: {result:?}");
        }
    }
}
