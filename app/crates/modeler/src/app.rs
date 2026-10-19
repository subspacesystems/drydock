// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The viewer window: an egui status bar around a viewport that shows the
//! offscreen-rendered scene. Behaviour: `docs/specs/m0-app.md`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use eframe::egui;
use egui::{Color32, Event, MouseWheelUnit, PointerButton, Pos2, Rect, Sense};
use glam::DVec2;
use modeler::document::{Document, Stats, format_size};
use modeler::input::{self, Button, Modifiers, Tool};
use modeler::viewstate::{Effect, ViewState, scene_bounds};
use modeler_dae::FlatScene;
use modeler_render::{GpuScene, Renderer, Style, Viewport};
use modeler_view::Projection;

/// Scroll distance in points that counts as one wheel notch (touchpads report points).
const POINTS_PER_SCROLL_LINE: f64 = 50.0;
const ERROR_COLOR: Color32 = Color32::from_rgb(200, 60, 50);

const ROBOTO: &[u8] = include_bytes!("../assets/fonts/Roboto-Regular.ttf");

/// The app's only font. eframe is built without egui's default fonts.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::empty();
    fonts.font_data.insert(
        "Roboto".to_owned(),
        Arc::new(egui::FontData::from_static(ROBOTO)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.insert(family, vec!["Roboto".to_owned()]);
    }
    ctx.set_fonts(fonts);
}

/// GPU state that lives as long as the window.
struct Gpu {
    renderer: Renderer,
    scene: GpuScene,
    viewport: Option<Viewport>,
    texture: Option<egui::TextureId>,
}

/// The open model, as far as the status bar cares.
struct OpenModel {
    path: PathBuf,
    stats: Stats,
}

struct Loading {
    path: PathBuf,
    result: Receiver<Result<Document, String>>,
}

pub struct App {
    view: ViewState,
    style: Style,
    gpu: Option<Gpu>,
    model: Option<OpenModel>,
    loading: Option<Loading>,
    error: Option<String>,
    warnings: Vec<String>,
    dialog: Option<OpenDialog>,
    /// Viewport size in points, for camera input.
    viewport_pt: (f64, f64),
    drag_start_ndc: DVec2,
    /// The rendered image is out of date.
    dirty: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, file: Option<PathBuf>) -> App {
        install_fonts(&cc.egui_ctx);
        let gpu = cc.wgpu_render_state.as_ref().map(|state| {
            let renderer = Renderer::new(&state.device);
            let scene = renderer.upload(&state.device, &state.queue, &FlatScene::default());
            Gpu {
                renderer,
                scene,
                viewport: None,
                texture: None,
            }
        });
        let mut app = App {
            view: ViewState::new(),
            style: Style::default(),
            gpu,
            model: None,
            loading: None,
            error: None,
            warnings: Vec::new(),
            dialog: None,
            viewport_pt: (1.0, 1.0),
            drag_start_ndc: DVec2::ZERO,
            dirty: true,
        };
        if app.gpu.is_none() {
            app.error = Some("No wgpu render state: cannot draw the viewport".to_owned());
        }
        if let Some(path) = file {
            app.start_loading(&cc.egui_ctx, path);
        }
        app
    }

    /// Open `path` on a background thread; `poll_loading` picks up the result.
    fn start_loading(&mut self, ctx: &egui::Context, path: PathBuf) {
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        let thread_path = path.clone();
        std::thread::spawn(move || {
            let result = Document::open(&thread_path);
            // The receiver is gone if another file was opened meanwhile.
            let _ = sender.send(result);
            ctx.request_repaint();
        });
        self.error = None;
        self.loading = Some(Loading {
            path,
            result: receiver,
        });
    }

    fn poll_loading(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let Some(loading) = &self.loading else {
            return;
        };
        let result = match loading.result.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                Err(format!("{}: loading failed", file_name(&loading.path)))
            }
        };
        self.loading = None;
        match result {
            Ok(document) => self.show_document(ctx, frame, document),
            Err(message) => self.error = Some(message),
        }
    }

    fn show_document(&mut self, ctx: &egui::Context, frame: &eframe::Frame, document: Document) {
        if let (Some(gpu), Some(state)) = (&mut self.gpu, frame.wgpu_render_state()) {
            gpu.scene = gpu
                .renderer
                .upload(&state.device, &state.queue, &document.scene);
            self.warnings = gpu.scene.warnings.clone();
        }
        self.view
            .set_bounds(scene_bounds(&document.scene), self.viewport_pt);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(document.title()));
        self.model = Some(OpenModel {
            path: document.path,
            stats: document.stats,
        });
        self.dirty = true;
    }

    fn open_dialog(&mut self) {
        let dir = self
            .model
            .as_ref()
            .and_then(|model| model.path.parent().map(Path::to_path_buf))
            .filter(|dir| dir.is_dir())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        self.dialog = Some(OpenDialog::new(dir));
    }

    fn apply(&mut self, action: input::Action) {
        match self.view.apply(action, self.viewport_pt) {
            Effect::None => self.dirty = true,
            Effect::OpenDialog => self.open_dialog(),
            Effect::Save | Effect::SaveAsDialog => unimplemented!(),
        }
    }

    /// Keyboard shortcuts and dropped files (window-wide).
    fn global_input(&mut self, ctx: &egui::Context) {
        let (events, dropped) = ctx.input(|i| (i.events.clone(), i.raw.dropped_files.clone()));
        if let Some(path) = dropped.into_iter().find_map(|file| {
            let path = file.path();
            (!path.as_os_str().is_empty()).then(|| path.to_path_buf())
        }) {
            self.start_loading(ctx, path);
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        for event in events {
            if let Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = event
                && let Some(action) = input::key(key.name(), to_modifiers(modifiers))
            {
                self.apply(action);
            }
        }
    }

    /// Mouse input over the viewport rectangle.
    fn viewport_input(&mut self, ui: &egui::Ui, rect: Rect, response: &egui::Response) {
        let ndc = |p: Pos2| {
            DVec2::new(
                f64::from((p.x - rect.left()) / rect.width()) * 2.0 - 1.0,
                1.0 - f64::from((p.y - rect.top()) / rect.height()) * 2.0,
            )
        };
        let (mods, press_origin, events) = ui.input(|i| {
            (
                to_modifiers(i.modifiers),
                i.pointer.press_origin(),
                i.events.clone(),
            )
        });
        for (egui_button, button) in [
            (PointerButton::Primary, Button::Left),
            (PointerButton::Middle, Button::Middle),
            (PointerButton::Secondary, Button::Right),
        ] {
            if response.drag_started_by(egui_button)
                && let Some(origin) = press_origin.or(response.interact_pointer_pos())
            {
                self.drag_start_ndc = ndc(origin);
            }
            if response.dragged_by(egui_button) {
                let delta = response.drag_delta();
                let delta = DVec2::new(f64::from(delta.x), f64::from(delta.y));
                if let Some(action) =
                    input::drag(self.view.tool, button, mods, delta, self.drag_start_ndc)
                {
                    self.apply(action);
                }
            }
        }
        if let Some(cursor) = response.hover_pos() {
            for event in events {
                if let Event::MouseWheel { unit, delta, .. } = event {
                    let lines = match unit {
                        MouseWheelUnit::Line => f64::from(delta.y),
                        MouseWheelUnit::Point => f64::from(delta.y) / POINTS_PER_SCROLL_LINE,
                        MouseWheelUnit::Page => f64::from(delta.y) * 3.0,
                    };
                    if lines != 0.0 {
                        self.apply(input::scroll(lines, ndc(cursor)));
                    }
                }
            }
        }
    }

    /// Make the offscreen target match the panel, re-render if needed, and paint it.
    fn paint_viewport(&mut self, ui: &egui::Ui, frame: &eframe::Frame, rect: Rect) {
        let (Some(gpu), Some(state)) = (&mut self.gpu, frame.wgpu_render_state()) else {
            return;
        };
        let scale = ui.ctx().pixels_per_point();
        let width = (rect.width() * scale).round().max(1.0) as u32;
        let height = (rect.height() * scale).round().max(1.0) as u32;
        let resized = gpu
            .viewport
            .as_ref()
            .is_none_or(|v| (v.width, v.height) != (width, height));
        if resized {
            let viewport = gpu.renderer.viewport(&state.device, width, height);
            let mut egui_renderer = state.renderer.write();
            match gpu.texture {
                Some(id) => egui_renderer.update_egui_texture_from_wgpu_texture(
                    &state.device,
                    &viewport.color_view,
                    wgpu::FilterMode::Linear,
                    id,
                ),
                None => {
                    gpu.texture = Some(egui_renderer.register_native_texture(
                        &state.device,
                        &viewport.color_view,
                        wgpu::FilterMode::Linear,
                    ));
                }
            }
            gpu.viewport = Some(viewport);
            self.dirty = true;
        }
        if let Some(viewport) = &gpu.viewport
            && self.dirty
        {
            gpu.renderer.render(
                &state.device,
                &state.queue,
                viewport,
                &gpu.scene,
                &self.view.camera,
                &self.style,
            );
            self.dirty = false;
        }
        if let Some(texture) = gpu.texture {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            ui.painter().image(texture, rect, uv, Color32::WHITE);
        }
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            match (&self.loading, &self.model) {
                (Some(loading), _) => {
                    ui.label(format!("Loading {}...", file_name(&loading.path)));
                }
                (None, Some(model)) => {
                    ui.label(file_name(&model.path));
                }
                (None, None) => {
                    ui.label("No model - Ctrl+O to open");
                }
            }
            if let Some(model) = &self.model {
                ui.separator();
                let stats = &model.stats;
                ui.label(format!(
                    "{} triangles, {} lines, {} materials, {}",
                    stats.triangles,
                    stats.lines,
                    stats.materials,
                    format_size(stats.size_m)
                ));
            }
            if !self.warnings.is_empty() {
                ui.separator();
                let noun = if self.warnings.len() == 1 {
                    "warning"
                } else {
                    "warnings"
                };
                ui.label(format!("{} texture {noun}", self.warnings.len()))
                    .on_hover_text(self.warnings.join("\n"));
            }
            if let Some(error) = &self.error {
                ui.separator();
                ui.colored_label(ERROR_COLOR, error);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(match self.view.camera.projection {
                    Projection::Perspective => "Perspective",
                    Projection::Parallel => "Parallel",
                });
                ui.separator();
                ui.label(match self.view.tool {
                    Tool::Orbit => "Orbit",
                    Tool::Pan => "Pan",
                    Tool::Zoom => "Zoom",
                });
            });
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_loading(&ctx, frame);
        if self.dialog.is_none() {
            self.global_input(&ctx);
        }
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::drag());
            self.viewport_pt = (f64::from(rect.width()), f64::from(rect.height()));
            if self.dialog.is_none() {
                self.viewport_input(ui, rect, &response);
            }
            self.paint_viewport(ui, frame, rect);
        });
        if let Some(dialog) = &mut self.dialog {
            match dialog.show(&ctx) {
                DialogOutcome::Open(path) => {
                    self.dialog = None;
                    self.start_loading(&ctx, path);
                }
                DialogOutcome::Cancel => self.dialog = None,
                DialogOutcome::Pending => {}
            }
        }
    }
}

fn to_modifiers(m: egui::Modifiers) -> Modifiers {
    Modifiers {
        shift: m.shift,
        // Ctrl on Linux and Windows, Cmd on macOS.
        ctrl: m.command,
        alt: m.alt,
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

enum DialogOutcome {
    Pending,
    Open(PathBuf),
    Cancel,
}

/// A minimal open-file dialog in plain egui: browse folders, pick a `.dae` file, or type
/// a path.
struct OpenDialog {
    dir: PathBuf,
    /// Subfolders and `.dae` files of `dir`, folders first, each sorted by name.
    entries: Vec<(String, PathBuf, bool)>,
    path_text: String,
    error: Option<String>,
}

impl OpenDialog {
    fn new(dir: PathBuf) -> OpenDialog {
        let mut dialog = OpenDialog {
            dir: PathBuf::new(),
            entries: Vec::new(),
            path_text: String::new(),
            error: None,
        };
        dialog.enter(dir);
        dialog
    }

    fn enter(&mut self, dir: PathBuf) {
        self.error = None;
        self.entries.clear();
        match std::fs::read_dir(&dir) {
            Ok(read_dir) => {
                for entry in read_dir.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let is_dir = path.is_dir();
                    let is_dae = path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("dae"));
                    if (is_dir && !name.starts_with('.')) || (!is_dir && is_dae) {
                        self.entries.push((name, path, is_dir));
                    }
                }
            }
            Err(err) => self.error = Some(format!("{}: {err}", dir.display())),
        }
        self.entries.sort_by(|a, b| {
            b.2.cmp(&a.2)
                .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
        });
        self.path_text = dir.display().to_string();
        self.dir = dir;
    }

    fn show(&mut self, ctx: &egui::Context) -> DialogOutcome {
        let mut outcome = DialogOutcome::Pending;
        let mut enter = None;
        let response = egui::Modal::new(egui::Id::new("open-dialog")).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.heading("Open model");
            ui.label(self.dir.display().to_string());
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if let Some(parent) = self.dir.parent()
                        && ui.selectable_label(false, "../").clicked()
                    {
                        enter = Some(parent.to_path_buf());
                    }
                    for (name, path, is_dir) in &self.entries {
                        let text = if *is_dir {
                            format!("{name}/")
                        } else {
                            name.clone()
                        };
                        if ui.selectable_label(false, text).clicked() {
                            if *is_dir {
                                enter = Some(path.clone());
                            } else {
                                outcome = DialogOutcome::Open(path.clone());
                            }
                        }
                    }
                });
            ui.separator();
            let typed = ui.add(
                egui::TextEdit::singleline(&mut self.path_text)
                    .desired_width(f32::INFINITY)
                    .hint_text("Path to a .dae file or folder"),
            );
            let submitted = typed.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if let Some(error) = &self.error {
                ui.colored_label(ERROR_COLOR, error);
            }
            ui.horizontal(|ui| {
                if ui.button("Open").clicked() || submitted {
                    let path = PathBuf::from(self.path_text.trim());
                    if path.is_dir() {
                        enter = Some(path);
                    } else if path.is_file() {
                        outcome = DialogOutcome::Open(path);
                    } else {
                        self.error = Some(format!("{}: no such file", path.display()));
                    }
                }
                if ui.button("Cancel").clicked() {
                    outcome = DialogOutcome::Cancel;
                }
            });
        });
        if let Some(dir) = enter {
            self.enter(dir);
        }
        if matches!(outcome, DialogOutcome::Pending) && response.should_close() {
            outcome = DialogOutcome::Cancel;
        }
        outcome
    }
}
