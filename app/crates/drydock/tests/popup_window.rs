// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m12-popup-window: the pop-up helper. The pure and kittest parts run everywhere; the
//! real-window part needs Xvfb, xdotool, xprop and DRYDOCK_GUI_TESTS=1 and skips itself
//! otherwise. See docs/specs/m12-popup-window.md.

use std::cell::RefCell;
use std::rc::Rc;

use drydock::popup::{
    PopupMode, PopupWindow, fit_size, inline_position, popup_mode, popup_supported_on,
    popup_viewport_id, popup_window,
};
use eframe::egui::{self, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

// ---- pure parts --------------------------------------------------------------------

#[test]
fn viewport_id_follows_the_title() {
    assert_eq!(popup_viewport_id("File"), popup_viewport_id("File"));
    assert_ne!(popup_viewport_id("File"), popup_viewport_id("Edit"));
    assert_ne!(popup_viewport_id("File"), egui::ViewportId::ROOT);
}

#[test]
fn platform_support() {
    assert!(popup_supported_on("windows", false));
    assert!(popup_supported_on("linux", false), "X11");
    assert!(!popup_supported_on("linux", true), "Wayland falls back");
    assert!(!popup_supported_on("macos", false));
}

#[test]
fn mode_is_a_window_only_with_real_viewports_on_a_supporting_platform() {
    assert_eq!(popup_mode(false, true), PopupMode::Window);
    assert_eq!(popup_mode(true, true), PopupMode::Inline, "UI tests");
    assert_eq!(popup_mode(false, false), PopupMode::Inline, "fallback");
    assert_eq!(popup_mode(true, false), PopupMode::Inline);
}

#[test]
fn fit_size_rounds_up_and_clamps() {
    assert_eq!(fit_size(vec2(100.2, 49.0), None), vec2(101.0, 49.0));
    assert_eq!(fit_size(vec2(0.0, 0.2), None), vec2(1.0, 1.0));
    assert_eq!(
        fit_size(vec2(300.0, 2000.0), Some(vec2(1920.0, 1080.0))),
        vec2(300.0, 1080.0)
    );
    assert_eq!(
        fit_size(vec2(5000.0, 10.0), Some(vec2(1920.0, 1080.0))),
        vec2(1920.0, 10.0)
    );
}

#[test]
fn inline_position_uses_the_roots_corner_and_stays_inside() {
    let content = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let size = vec2(100.0, 80.0);
    // Screen position, root's inner corner known: converted.
    let root = Rect::from_min_size(pos2(200.0, 100.0), vec2(800.0, 600.0));
    assert_eq!(
        inline_position(Some(root), pos2(250.0, 160.0), size, content),
        pos2(50.0, 60.0)
    );
    // Unknown root: used as it is.
    assert_eq!(
        inline_position(None, pos2(250.0, 160.0), size, content),
        pos2(250.0, 160.0)
    );
    // Past the right and bottom edges: moved left and up until it fits.
    assert_eq!(
        inline_position(None, pos2(790.0, 590.0), size, content),
        pos2(700.0, 520.0)
    );
    // Before the top-left corner: moved to it.
    assert_eq!(
        inline_position(Some(root), pos2(100.0, 50.0), size, content),
        pos2(0.0, 0.0)
    );
    // Larger than the area: the top-left corner wins.
    assert_eq!(
        inline_position(None, pos2(10.0, 10.0), vec2(900.0, 700.0), content),
        pos2(0.0, 0.0)
    );
}

// ---- inline mode (kittest embeds viewports) ------------------------------------------

fn harness(open: Rc<RefCell<bool>>, position: egui::Pos2) -> Harness<'static> {
    Harness::new_ui(move |ui| {
        let _ = ui.button("outside");
        let mut o = *open.borrow();
        let popup = PopupWindow {
            title: "Test Popup",
            position,
        };
        let outcome = popup_window(ui.ctx(), &popup, &mut o, |ui| {
            ui.label("popup item");
        });
        assert_eq!(outcome.mode, PopupMode::Inline);
        *open.borrow_mut() = o;
    })
}

#[test]
fn inline_popup_shows_its_content_at_the_position() {
    let open = Rc::new(RefCell::new(true));
    let mut h = harness(open.clone(), pos2(300.0, 200.0));
    assert!(h.ctx.embed_viewports(), "kittest embeds viewports");
    h.run();
    let rect = h.get_by_label("popup item").rect();
    let (dx, dy) = (rect.min.x - 300.0, rect.min.y - 200.0);
    assert!(
        (0.0..=16.0).contains(&dx) && (0.0..=16.0).contains(&dy),
        "content sits just inside the frame at the position: {rect:?}"
    );
    assert!(*open.borrow());
}

#[test]
fn inline_popup_has_no_title_bar() {
    let open = Rc::new(RefCell::new(true));
    let mut h = harness(open, pos2(300.0, 200.0));
    h.run();
    assert!(h.query_by_label("Test Popup").is_none());
}

#[test]
fn inline_popup_near_the_edge_is_moved_inside() {
    let open = Rc::new(RefCell::new(true));
    let mut h = harness(open, pos2(5000.0, 5000.0));
    h.run();
    h.run();
    let rect = h.get_by_label("popup item").rect();
    let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    assert!(screen.contains_rect(rect), "{rect:?} not inside {screen:?}");
}

#[test]
fn nothing_is_drawn_while_closed() {
    let open = Rc::new(RefCell::new(false));
    let mut h = harness(open, pos2(300.0, 200.0));
    h.run();
    assert!(h.query_by_label("popup item").is_none());
}

#[test]
fn escape_closes_the_inline_popup() {
    let open = Rc::new(RefCell::new(true));
    let mut h = harness(open.clone(), pos2(300.0, 200.0));
    h.run();
    h.key_press(egui::Key::Escape);
    h.run();
    assert!(!*open.borrow());
}

#[test]
fn a_press_outside_closes_the_inline_popup() {
    let open = Rc::new(RefCell::new(true));
    let mut h = harness(open.clone(), pos2(300.0, 200.0));
    h.run();
    h.get_by_label("outside").click();
    h.run();
    assert!(!*open.borrow());
}

#[test]
fn a_press_inside_keeps_the_inline_popup_open() {
    let open = Rc::new(RefCell::new(true));
    let mut h = harness(open.clone(), pos2(300.0, 200.0));
    h.run();
    h.get_by_label("popup item").click();
    h.run();
    assert!(*open.borrow());
}

// ---- real window -------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod real {
    use std::process::Command;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    use drydock::popup::{PopupWindow, popup_window};
    use eframe::egui;

    const ROOT: &str = "popup-probe-root";
    const POPUP: &str = "popup-probe-menu";

    struct Probe {
        open: bool,
        reopen: Arc<AtomicBool>,
        quit: Arc<AtomicBool>,
    }

    impl eframe::App for Probe {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            let ctx = ui.ctx().clone();
            ui.label("root");
            if self.reopen.swap(false, Ordering::SeqCst) {
                self.open = true;
            }
            // Root window is 320x240 at the screen's top-left; the pop-up reaches past it.
            let popup = PopupWindow {
                title: POPUP,
                position: egui::pos2(250.0, 200.0),
            };
            popup_window(&ctx, &popup, &mut self.open, |ui| {
                ui.label("popup-probe-item");
            });
            if self.quit.load(Ordering::SeqCst) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn run_probe(reopen: Arc<AtomicBool>, quit: Arc<AtomicBool>) -> Result<(), String> {
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
            "popup-probe",
            options,
            Box::new(move |_cc| {
                Ok(Box::new(Probe {
                    open: true,
                    reopen,
                    quit,
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

    /// (x, y, width, height) from `xdotool getwindowgeometry --shell`.
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
    fn popup_is_a_separate_window_at_the_position_and_closes_on_escape_and_focus_loss() {
        if std::env::var_os("DRYDOCK_GUI_TESTS").is_none()
            || std::env::var_os("DISPLAY").is_none()
            || std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
        {
            eprintln!("skipped: needs DRYDOCK_GUI_TESTS=1 and an X11 display");
            return;
        }
        let reopen = Arc::new(AtomicBool::new(false));
        let quit = Arc::new(AtomicBool::new(false));
        let app = {
            let (reopen, quit) = (Arc::clone(&reopen), Arc::clone(&quit));
            thread::spawn(move || run_probe(reopen, quit))
        };

        wait_for("the root window", || find_window(ROOT).is_some());
        wait_for("the pop-up window", || find_window(POPUP).is_some());
        let root = find_window(ROOT).unwrap();
        let popup = find_window(POPUP).unwrap();
        assert_ne!(root, popup, "a separate OS window");
        thread::sleep(Duration::from_secs(1));

        let (x, y, w, h) = geometry(popup);
        assert_eq!((x, y), (250, 200), "at the requested screen position");
        let (_, _, root_w, root_h) = geometry(root);
        assert_eq!((root_w, root_h), (320, 240));
        assert!(
            x + w > root_w || y + h > root_h,
            "reaches past the root: {w}x{h}"
        );
        assert!(
            (20..400).contains(&w) && (10..300).contains(&h),
            "sized to its content, not the default: {w}x{h}"
        );
        let kind = tool("xprop", &["-id", &popup.to_string(), "_NET_WM_WINDOW_TYPE"]);
        assert!(kind.contains("_NET_WM_WINDOW_TYPE_POPUP_MENU"), "{kind}");
        let transient = tool("xprop", &["-id", &popup.to_string(), "WM_TRANSIENT_FOR"]);
        assert!(
            transient.contains("window id #"),
            "owned by the root: {transient}"
        );

        // Escape closes it.
        tool("xdotool", &["windowfocus", "--sync", &popup.to_string()]);
        tool("xdotool", &["key", "Escape"]);
        wait_for("Escape to close the pop-up", || {
            find_window(POPUP).is_none()
        });
        assert!(find_window(ROOT).is_some(), "the root stays");

        // Losing focus closes it.
        reopen.store(true, Ordering::SeqCst);
        wait_for("the pop-up to come back", || find_window(POPUP).is_some());
        let popup = find_window(POPUP).unwrap();
        tool("xdotool", &["windowfocus", "--sync", &popup.to_string()]);
        thread::sleep(Duration::from_millis(500));
        tool("xdotool", &["windowfocus", "--sync", &root.to_string()]);
        wait_for("focus loss to close the pop-up", || {
            find_window(POPUP).is_none()
        });

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
