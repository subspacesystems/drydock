// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m12-menu-close-outside: open pop-up menus close on a click or Escape in the main window and
//! when Drydock loses focus (`docs/specs/m12-menu-close-outside.md`). Pure rules, kittest, and a
//! gated real-window check on X11.

use drydock::chrome::{
    FOCUS_GRACE, MenuState, RootClose, RootInput, close_menus_from_root, menu_bar, open_menu,
    request_menu, request_menu_step, root_close, root_close_repaint_after,
};
use drydock::popup::PopupMode;
use eframe::egui::{self, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::cell::Cell;
use std::rc::Rc;

const TITLES: [Rect; 2] = [
    Rect::from_min_max(pos2(0.0, 0.0), pos2(40.0, 20.0)),
    Rect::from_min_max(pos2(40.0, 0.0), pos2(80.0, 20.0)),
];

fn input(menu: bool, submenu: bool) -> RootInput<'static> {
    RootInput {
        menu_open: menu,
        submenu_open: submenu,
        press: None,
        escape: false,
        titles: &TITLES,
        unfocused_for: None,
        popup_has_focus: false,
    }
}

// ---- pure ---------------------------------------------------------------------------

#[test]
fn nothing_open_nothing_closes() {
    let mut i = input(false, false);
    i.press = Some(pos2(200.0, 200.0));
    i.escape = true;
    i.unfocused_for = Some(5.0);
    assert_eq!(root_close(&i), RootClose::Nothing);
}

#[test]
fn quiet_frames_leave_the_menu_open() {
    assert_eq!(root_close(&input(true, false)), RootClose::Nothing);
    assert_eq!(root_close(&input(true, true)), RootClose::Nothing);
}

#[test]
fn a_press_away_from_the_titles_closes_everything() {
    for submenu in [false, true] {
        let mut i = input(true, submenu);
        i.press = Some(pos2(200.0, 200.0));
        assert_eq!(root_close(&i), RootClose::All);
    }
    // Just below the bar is outside too.
    let mut i = input(true, false);
    i.press = Some(pos2(10.0, 25.0));
    assert_eq!(root_close(&i), RootClose::All);
}

#[test]
fn a_press_on_a_title_is_left_to_the_title() {
    for p in [pos2(10.0, 10.0), pos2(60.0, 10.0)] {
        let mut i = input(true, false);
        i.press = Some(p);
        assert_eq!(root_close(&i), RootClose::Nothing);
        i.submenu_open = true;
        assert_eq!(root_close(&i), RootClose::Nothing);
    }
}

#[test]
fn escape_closes_the_submenu_first_then_the_menu() {
    let mut i = input(true, true);
    i.escape = true;
    assert_eq!(root_close(&i), RootClose::Submenu);
    i.submenu_open = false;
    assert_eq!(root_close(&i), RootClose::All);
}

#[test]
fn losing_focus_closes_everything_after_the_grace_time() {
    let mut i = input(true, true);
    i.unfocused_for = Some(FOCUS_GRACE + 0.01);
    assert_eq!(root_close(&i), RootClose::All);
    i.unfocused_for = Some(FOCUS_GRACE * 0.5);
    assert_eq!(
        root_close(&i),
        RootClose::Nothing,
        "focus may be on its way to the pop-up"
    );
    i.unfocused_for = None;
    assert_eq!(root_close(&i), RootClose::Nothing);
}

#[test]
fn focus_on_a_popup_window_keeps_the_menus_open() {
    let mut i = input(true, true);
    i.unfocused_for = Some(10.0);
    i.popup_has_focus = true;
    assert_eq!(root_close(&i), RootClose::Nothing);
}

#[test]
fn repaint_is_scheduled_for_the_rest_of_the_grace_time() {
    let mut i = input(true, false);
    i.unfocused_for = Some(0.1);
    let after = root_close_repaint_after(&i).expect("a repaint is due");
    assert!((after - (FOCUS_GRACE - 0.1)).abs() < 1e-9, "{after}");
    i.unfocused_for = Some(0.0);
    let after = root_close_repaint_after(&i).expect("a repaint is due");
    assert!((after - FOCUS_GRACE).abs() < 1e-9);
}

#[test]
fn no_repaint_is_needed_in_the_other_cases() {
    let mut i = input(true, true);
    i.unfocused_for = Some(FOCUS_GRACE + 0.01);
    assert_eq!(root_close_repaint_after(&i), None, "grace already over");
    i.unfocused_for = Some(FOCUS_GRACE);
    assert_eq!(root_close_repaint_after(&i), None);
    i.unfocused_for = None;
    assert_eq!(root_close_repaint_after(&i), None, "focused");
    i.unfocused_for = Some(0.1);
    i.popup_has_focus = true;
    assert_eq!(root_close_repaint_after(&i), None, "a pop-up has focus");
    let mut closed = input(false, false);
    closed.unfocused_for = Some(0.1);
    assert_eq!(root_close_repaint_after(&closed), None, "no menu open");
}

// ---- kittest -------------------------------------------------------------------------

/// A harness that opens `File` on its first frame and then, each frame, applies the
/// main-window rules in `mode`. It has no menu bar of its own, so no title rectangles.
fn rules_harness(mode: PopupMode, clicks: &Rc<Cell<u32>>) -> Harness<'static> {
    let clicks = clicks.clone();
    let mut first = true;
    let mut h = Harness::builder()
        .with_size(vec2(400.0, 300.0))
        .build_ui(move |ui| {
            if first {
                first = false;
                request_menu(ui.ctx(), "File");
            }
            if ui.button("underneath").clicked() {
                clicks.set(clicks.get() + 1);
            }
            close_menus_from_root(ui.ctx(), mode);
        });
    h.run();
    h
}

fn current(h: &Harness<'_>) -> Option<String> {
    open_menu(&h.ctx)
}

#[test]
fn window_mode_closes_the_menu_on_escape_in_the_main_window() {
    let mut h = rules_harness(PopupMode::Window, &Rc::default());
    assert_eq!(current(&h).as_deref(), Some("File"));
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(current(&h), None);
}

#[test]
fn window_mode_closes_the_menu_on_a_press_without_eating_it() {
    let clicks = Rc::new(Cell::new(0));
    let mut h = rules_harness(PopupMode::Window, &clicks);
    assert_eq!(current(&h).as_deref(), Some("File"));
    h.get_by_label("underneath").click();
    h.run();
    assert_eq!(current(&h), None);
    assert_eq!(clicks.get(), 1, "the press still reaches what is under it");
}

#[test]
fn inline_mode_leaves_it_to_the_inline_pop_up() {
    let mut h = rules_harness(PopupMode::Inline, &Rc::default());
    h.key_press(egui::Key::Escape);
    h.run();
    assert_eq!(current(&h).as_deref(), Some("File"));
}

#[test]
fn the_rules_report_whether_they_closed_something() {
    let result = Rc::new(Cell::new((false, false)));
    let seen = result.clone();
    let mut h = Harness::builder()
        .with_size(vec2(200.0, 100.0))
        .build_ui(move |ui| {
            let none = close_menus_from_root(ui.ctx(), PopupMode::Window);
            request_menu(ui.ctx(), "File");
            let some = close_menus_from_root(ui.ctx(), PopupMode::Window);
            seen.set((none, some));
        });
    h.key_press(egui::Key::Escape);
    h.run();
    // Nothing open at the first call: false. After request_menu the Escape closes it: true.
    assert_eq!(result.get(), (false, true));
}

fn bar_harness(step: &Rc<Cell<i32>>) -> Harness<'static> {
    let step = step.clone();
    let state = MenuState::default();
    let mut h = Harness::builder()
        .with_size(vec2(700.0, 500.0))
        .build_ui(move |ui| {
            let s = step.replace(0);
            if s != 0 {
                request_menu_step(ui.ctx(), s);
            }
            menu_bar(ui, &state);
        });
    h.ctx.set_fonts(drydock::look::font_definitions());
    h.run();
    h
}

#[test]
fn a_requested_step_moves_to_the_next_and_previous_menu_with_wrap() {
    let step = Rc::new(Cell::new(0));
    let mut h = bar_harness(&step);
    h.get_by_label("Edit").click();
    h.run();
    h.run();
    step.set(1);
    h.run();
    h.run();
    assert_eq!(current(&h).as_deref(), Some("View"));
    for _ in 0..2 {
        step.set(-1);
        h.run();
        h.run();
    }
    assert_eq!(current(&h).as_deref(), Some("File"));
    step.set(-1);
    h.run();
    h.run();
    assert_eq!(current(&h).as_deref(), Some("Help"));
}

#[test]
fn a_requested_step_with_nothing_open_does_nothing() {
    let step = Rc::new(Cell::new(0));
    let mut h = bar_harness(&step);
    step.set(1);
    h.run();
    h.run();
    assert_eq!(current(&h), None);
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

    const ROOT: &str = "menu-close-probe-root";

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
                .with_inner_size([320.0, 240.0]),
            event_loop_builder: Some(Box::new(|builder| {
                use winit::platform::x11::EventLoopBuilderExtX11;
                builder.with_x11().with_any_thread(true);
            })),
            ..Default::default()
        };
        eframe::run_native(
            "menu-close-probe",
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
        tool("xdotool", &["search", "--name", &format!("^{title}$")])
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

    /// Sets the X input focus to None, as when another application takes it away.
    fn clear_x_focus() {
        let script = "import ctypes; x=ctypes.CDLL('libX11.so.6'); \
d=x.XOpenDisplay(None); x.XSetInputFocus(ctypes.c_void_p(d), 0, 2, 0); \
x.XSync(ctypes.c_void_p(d), 0); x.XCloseDisplay(ctypes.c_void_p(d))";
        let status = Command::new("python3").args(["-c", script]).status();
        assert!(
            status.is_ok_and(|s| s.success()),
            "could not clear the X focus"
        );
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
    fn a_click_or_escape_in_the_main_window_closes_a_pop_up_menu() {
        if std::env::var_os("DRYDOCK_GUI_TESTS").is_none()
            || std::env::var_os("DISPLAY").is_none()
            || std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
        {
            eprintln!("skipped: needs DRYDOCK_GUI_TESTS=1 and an X11 display");
            return;
        }
        let request: Request = Arc::default();
        let quit = Arc::new(AtomicBool::new(false));
        let app = {
            let (request, quit) = (Arc::clone(&request), Arc::clone(&quit));
            thread::spawn(move || run_probe(request, quit))
        };
        wait_for("the root window", || find_window(ROOT).is_some());
        thread::sleep(Duration::from_secs(1));
        let root = find_window(ROOT).unwrap();
        let (rx, ry, rw, rh) = geometry(root);
        let file = drydock::chrome::menu_popup_title("File");

        // A click in the viewport area, well below the menu bar.
        *request.lock().unwrap() = Some("File");
        wait_for("the File pop-up", || find_window(&file).is_some());
        thread::sleep(Duration::from_secs(1));
        let (cx, cy) = (rx + rw / 2, ry + rh - 20);
        tool(
            "xdotool",
            &["mousemove", &cx.to_string(), &cy.to_string(), "click", "1"],
        );
        wait_for("a click in the main window to close the menu", || {
            find_window(&file).is_none()
        });

        // Escape with the main window focused.
        *request.lock().unwrap() = Some("File");
        wait_for("the File pop-up again", || find_window(&file).is_some());
        thread::sleep(Duration::from_secs(1));
        tool("xdotool", &["windowfocus", "--sync", &root.to_string()]);
        tool("xdotool", &["key", "Escape"]);
        wait_for("Escape in the main window to close the menu", || {
            find_window(&file).is_none()
        });
        assert!(find_window(ROOT).is_some());

        // Losing focus entirely (no window has it) closes the menu by itself, with no input.
        *request.lock().unwrap() = Some("File");
        wait_for("the File pop-up for the focus test", || {
            find_window(&file).is_some()
        });
        thread::sleep(Duration::from_secs(1));
        clear_x_focus();
        thread::sleep(Duration::from_secs(1));
        assert!(
            find_window(&file).is_none(),
            "the menu stays open after focus is lost and nothing happens"
        );

        // Right in a focused pop-up switches to Edit and the new menu stays open.
        let edit = drydock::chrome::menu_popup_title("Edit");
        tool("xdotool", &["windowfocus", "--sync", &root.to_string()]);
        thread::sleep(Duration::from_millis(500));
        *request.lock().unwrap() = Some("File");
        wait_for("the File pop-up for the step test", || {
            find_window(&file).is_some()
        });
        thread::sleep(Duration::from_secs(1));
        let file_id = find_window(&file).unwrap();
        tool("xdotool", &["windowfocus", "--sync", &file_id.to_string()]);
        thread::sleep(Duration::from_millis(300));
        tool("xdotool", &["key", "Right"]);
        wait_for("the Edit pop-up after Right", || {
            find_window(&edit).is_some()
        });
        thread::sleep(Duration::from_secs(1));
        assert!(
            find_window(&edit).is_some(),
            "the Edit menu closed by itself after the step"
        );

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
