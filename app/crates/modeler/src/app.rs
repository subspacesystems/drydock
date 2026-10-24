// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The viewer window: an egui status bar around a viewport that shows the
//! offscreen-rendered scene. Behaviour: `docs/specs/m0-app.md`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use eframe::egui;
use egui::{Color32, Event, MouseWheelUnit, PointerButton, Pos2, Rect, Sense};
use glam::{DMat4, DVec2, DVec3};
use modeler::chrome::{self, MenuState};
use modeler::document::{Document, FileKind, format_size, native_path, resolve_texture_path};
use modeler::editor::{Editor, Viewport as EditorViewport};
use modeler::hints::hint_label;
use modeler::input::{self, Button, Modifiers, Tool};
use modeler::materials_panel::{MaterialAction, MaterialsPanel, materials_panel};
use modeler::overlay::{self, Primitive};
use modeler::recent::{RecentFiles, recent_file};
use modeler::texture_panel::{TextureForm, texture_panel};
use modeler::viewstate::{Effect, ViewState, scene_bounds};
use modeler_dae::{FlatScene, RestView};
use modeler_kernel::editpath::EditPath;
use modeler_kernel::model::{BodyId, Material, Model};
use modeler_kernel::paint::Side;
use modeler_kernel::select::Entity;
use modeler_kernel::tools::plane::DrawPlane;
use modeler_kernel::tools::scale;
use modeler_kernel::topology::FaceId;
use modeler_render::{GpuScene, Renderer, Style, Viewport};
use modeler_view::Projection;

/// Scroll distance in points that counts as one wheel notch (touchpads report points).
const POINTS_PER_SCROLL_LINE: f64 = 50.0;
const ERROR_COLOR: Color32 = Color32::from_rgb(200, 60, 50);
const ACCENT_COLOR: Color32 = Color32::from_rgb(230, 140, 25);
const PREVIEW_COLOR: Color32 = Color32::from_rgb(55, 60, 70);
/// The Rotate protractor's radius on screen before its start point is placed.
const PROTRACTOR_PX: f64 = 48.0;

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

struct Loading {
    path: PathBuf,
    result: Receiver<Result<Document, String>>,
}

pub struct App {
    view: ViewState,
    /// The Help > About window is showing.
    about_open: bool,
    /// The Make Component / Make Group name prompt is showing.
    name_prompt: Option<chrome::NamePrompt>,
    /// The Materials panel and its open form.
    materials: MaterialsPanel,
    /// The Texture Position panel's typed values.
    texture: TextureForm,
    style: Style,
    gpu: Option<Gpu>,
    model: Option<Document>,
    /// One editor per open document; `None` before a document exists.
    editor: Option<Editor>,
    loading: Option<Loading>,
    error: Option<String>,
    /// "Saved <file name>" until the next open or save.
    saved: Option<String>,
    warnings: Vec<String>,
    dialog: Option<OpenDialog>,
    save_dialog: Option<SaveDialog>,
    /// A destructive action waiting for the Save/Discard/Cancel question.
    confirm: Option<PendingAction>,
    /// What to do after a save that was started from the unsaved-changes question.
    after_save: Option<PendingAction>,
    /// Let the close request through after the question has been answered.
    allow_close: bool,
    /// Viewport size in points, for camera input.
    viewport_pt: (f64, f64),
    drag_start_ndc: DVec2,
    drag_start_px: Option<DVec2>,
    /// The rendered image is out of date.
    dirty: bool,
    /// The editor path the uploaded scene was built for.
    shown_path: EditPath,
    /// The File menu's recent files (`docs/specs/m4-ui-menus.md`).
    recent: RecentFiles,
    /// Where `recent` is kept between runs.
    recent_file: Option<PathBuf>,
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
        let recent_file = recent_file(|name| std::env::var_os(name));
        let recent = recent_file
            .as_deref()
            .map(RecentFiles::load)
            .unwrap_or_default();
        let mut app = App {
            view: ViewState::new(),
            about_open: false,
            name_prompt: None,
            materials: MaterialsPanel::default(),
            texture: TextureForm::default(),
            style: Style::default(),
            gpu,
            model: None,
            editor: None,
            loading: None,
            error: None,
            saved: None,
            warnings: Vec::new(),
            dialog: None,
            save_dialog: None,
            confirm: None,
            after_save: None,
            allow_close: false,
            viewport_pt: (1.0, 1.0),
            drag_start_ndc: DVec2::ZERO,
            drag_start_px: None,
            dirty: true,
            shown_path: EditPath::root(),
            recent,
            recent_file,
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
        self.saved = None;
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
        let loaded_path = (!document.untitled).then(|| document.path.clone());
        self.upload_scene(frame, &document.scene);
        self.view
            .set_bounds(scene_bounds(&document.scene), self.viewport_pt);
        self.editor = Some(Editor::new());
        self.materials = MaterialsPanel::default();
        self.texture = TextureForm::default();
        self.shown_path = EditPath::root();
        if let Some(editor) = &mut self.editor {
            editor.set_tool(self.view.tool);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(document.title()));
        self.model = Some(document);
        if let Some(path) = loaded_path {
            self.remember_recent(&path);
        }
        self.dirty = true;
    }

    /// Add `path` to the recent files and store the list; a failed save is ignored.
    fn remember_recent(&mut self, path: &Path) {
        self.recent.add(path);
        if let Some(file) = &self.recent_file {
            let _ = self.recent.save(file);
        }
    }

    /// Make `scene` the scene on screen and keep its texture warnings.
    fn upload_scene(&mut self, frame: &eframe::Frame, scene: &FlatScene) {
        if let (Some(gpu), Some(state)) = (&mut self.gpu, frame.wgpu_render_state()) {
            gpu.scene = gpu.renderer.upload(&state.device, &state.queue, scene);
            self.warnings = gpu.scene.warnings.clone();
        }
    }

    fn undo_state(&self) -> u64 {
        self.editor
            .as_ref()
            .map_or(0, |editor| editor.undo_stack().state())
    }

    /// Rebuild the scene after an editor change and upload it.
    fn refresh_document(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let state = self.undo_state();
        let (path, rest) = self.editor.as_ref().map_or_else(
            || (EditPath::root(), RestView::Faded),
            |editor| {
                let rest = if editor.hide_rest() {
                    RestView::Hidden
                } else {
                    RestView::Faded
                };
                (editor.path().clone(), rest)
            },
        );
        let Some(document) = &mut self.model else {
            return;
        };
        document.refresh_editing(&path, rest);
        document.sync_modified(state);
        self.shown_path = path;
        // Zoom Extents frames what is drawn, not the whole model.
        self.view.bounds = scene_bounds(&document.scene);
        let title = document.title();
        if let (Some(gpu), Some(state)) = (&mut self.gpu, frame.wgpu_render_state()) {
            gpu.scene = gpu
                .renderer
                .upload(&state.device, &state.queue, &document.scene);
            self.warnings = gpu.scene.warnings.clone();
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        self.dirty = true;
    }

    /// Rebuild the scene when the editor's open path changed without a model edit.
    fn sync_editor_path(&mut self, _ctx: &egui::Context, frame: &eframe::Frame) {
        let Some(path) = self.editor.as_ref().map(|editor| editor.path().clone()) else {
            return;
        };
        if path != self.shown_path {
            self.show_editor_path(frame);
        }
    }

    /// Redraw for the current open path without rebuilding stats, the title or the
    /// modified flag.
    fn show_editor_path(&mut self, frame: &eframe::Frame) {
        let Some((path, rest)) = self.editor.as_ref().map(|editor| {
            let rest = if editor.hide_rest() {
                RestView::Hidden
            } else {
                RestView::Faded
            };
            (editor.path().clone(), rest)
        }) else {
            return;
        };
        let Some(document) = &mut self.model else {
            return;
        };
        document.show_path(&path, rest);
        self.shown_path = path;
        // Zoom Extents frames what is drawn, not the whole model.
        self.view.bounds = scene_bounds(&document.scene);
        if let (Some(gpu), Some(state)) = (&mut self.gpu, frame.wgpu_render_state()) {
            gpu.scene = gpu
                .renderer
                .upload(&state.device, &state.queue, &document.scene);
            self.warnings = gpu.scene.warnings.clone();
        }
        self.dirty = true;
    }

    /// Replace the document with a new, empty one and a default view.
    fn new_document(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let document = Document::new_empty();
        self.upload_scene(frame, &document.scene);
        self.view = ViewState::new();
        self.editor = Some(Editor::new());
        self.materials = MaterialsPanel::default();
        self.texture = TextureForm::default();
        self.shown_path = EditPath::root();
        self.loading = None;
        self.error = None;
        self.saved = None;
        self.confirm = None;
        self.after_save = None;
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(document.title()));
        self.model = Some(document);
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

    fn open_save_dialog(&mut self) {
        let Some(document) = &self.model else {
            return;
        };
        let dir = document
            .path
            .parent()
            .map(Path::to_path_buf)
            .filter(|dir| dir.is_dir())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let name = native_path(&document.path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "model.modeler".to_owned());
        self.save_dialog = Some(SaveDialog::new(dir, name));
    }

    /// Save the open document, asking for a file name when it has none (or is a DAE).
    fn save(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        if self.loading.is_some() {
            return;
        }
        let Some(document) = &self.model else {
            return;
        };
        self.error = None;
        self.saved = None;
        if document.untitled || document.kind == FileKind::Dae {
            self.open_save_dialog();
            return;
        }
        let result = self.model.as_mut().map(Document::save);
        match result {
            Some(Ok(path)) => {
                let state = self.undo_state();
                if let Some(document) = &mut self.model {
                    document.mark_saved_at(state);
                }
                self.saved = Some(file_name(&path));
                self.saved_now(ctx, frame);
            }
            Some(Err(message)) => {
                self.error = Some(message);
                self.after_save = None;
            }
            None => {}
        }
    }

    /// A save succeeded: refresh the title and continue an interrupted open/new/close.
    fn saved_now(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        if let Some(document) = &self.model {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(document.title()));
        }
        if let Some(action) = self.after_save.take() {
            self.perform(action, ctx, frame);
        }
    }

    /// Ask about unsaved changes first when the document is modified.
    fn request(&mut self, action: PendingAction, ctx: &egui::Context, frame: &eframe::Frame) {
        if self.loading.is_some() {
            return;
        }
        let modified = self.model.as_ref().is_some_and(|model| model.modified);
        if modified {
            self.confirm = Some(action);
        } else {
            self.perform(action, ctx, frame);
        }
    }

    fn perform(&mut self, action: PendingAction, ctx: &egui::Context, frame: &eframe::Frame) {
        match action {
            PendingAction::New => self.new_document(ctx, frame),
            PendingAction::Open(None) => self.open_dialog(),
            PendingAction::Open(Some(path)) => self.start_loading(ctx, path),
            PendingAction::Close => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Save from the unsaved-changes question, then continue what was asked for.
    fn save_before(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let Some(action) = self.confirm.take() else {
            return;
        };
        self.after_save = Some(action);
        self.save(ctx, frame);
    }

    fn edit(&mut self, key: input::EditKey, ctx: &egui::Context, frame: &eframe::Frame) {
        let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) else {
            return;
        };
        let Some(model) = document.kernel.as_mut() else {
            return;
        };
        if editor.key(key, model) {
            self.refresh_document(ctx, frame);
        }
    }

    /// Resolve a form material's relative texture path against the document's folder.
    fn resolve_material_texture(&self, mut material: Material) -> Material {
        let Some(texture) = &mut material.texture else {
            return material;
        };
        if Path::new(&texture.path).is_relative() {
            let folder = self
                .model
                .as_ref()
                .and_then(|document| document.path.parent())
                .unwrap_or_else(|| Path::new(""));
            texture.path = resolve_texture_path(&texture.path, folder);
        }
        material
    }

    /// The selected face side the Texture Position panel acts on: exactly one selected
    /// face, with a texture on the side facing the camera.
    fn texture_target(&self, model: &Model) -> Option<(FaceId, Side)> {
        let editor = self.editor.as_ref()?;
        let [Entity::Face(face)] = editor.selection() else {
            return None;
        };
        let face = *face;
        let body = editor.path().body(model)?;
        let transform = editor.path().transform(model)?;
        let plane = model.body(body)?.geometry().face_plane(face)?;
        let camera = self.view.camera;
        let (forward, _, _) = camera.basis();
        let direction = match camera.projection {
            Projection::Perspective => {
                let eye = transform.inverse().transform_point3(camera.eye());
                (plane.point - eye).normalize_or_zero()
            }
            Projection::Parallel => transform
                .inverse()
                .transform_vector3(forward)
                .normalize_or_zero(),
        };
        let side = if direction.dot(plane.normal) > 0.0 {
            Side::Back
        } else {
            Side::Front
        };
        let materials = model.body(body)?.face_materials(face);
        let material = match side {
            Side::Front => materials.front,
            Side::Back => materials.back,
        }?;
        model.material(material)?.texture.as_ref()?;
        Some((face, side))
    }

    /// Apply what the Materials panel asked for.
    fn apply_material_action(
        &mut self,
        action: MaterialAction,
        ctx: &egui::Context,
        frame: &eframe::Frame,
    ) {
        match action {
            MaterialAction::Choose(material) => {
                if let Some(editor) = &mut self.editor {
                    editor.set_material(material);
                }
                self.apply(input::Action::SelectTool(Tool::Paint), ctx, frame);
            }
            MaterialAction::Create(material) => {
                let material = self.resolve_material_texture(material);
                let created =
                    if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) {
                        match document.kernel.as_mut() {
                            Some(model) => editor.create_material(model, material).is_some(),
                            None => false,
                        }
                    } else {
                        false
                    };
                if created {
                    self.refresh_document(ctx, frame);
                }
            }
            MaterialAction::Edit(id, material) => {
                let material = self.resolve_material_texture(material);
                let changed =
                    if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) {
                        match document.kernel.as_mut() {
                            Some(model) => editor.edit_material(model, id, material),
                            None => false,
                        }
                    } else {
                        false
                    };
                if changed {
                    self.refresh_document(ctx, frame);
                }
            }
            MaterialAction::Purge => {
                let removed =
                    if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) {
                        match document.kernel.as_mut() {
                            Some(model) => editor.purge_materials(model),
                            None => 0,
                        }
                    } else {
                        0
                    };
                if removed > 0 {
                    self.refresh_document(ctx, frame);
                }
            }
        }
    }

    fn apply(&mut self, action: input::Action, ctx: &egui::Context, frame: &eframe::Frame) {
        let selects_tool = matches!(action, input::Action::SelectTool(_));
        let effect = self.view.apply(action, self.viewport_pt);
        if selects_tool {
            if let Some(editor) = &mut self.editor {
                editor.set_tool(self.view.tool);
            }
            self.dirty = true;
        }
        match effect {
            Effect::None => self.dirty = true,
            Effect::New => self.request(PendingAction::New, ctx, frame),
            Effect::OpenDialog => self.request(PendingAction::Open(None), ctx, frame),
            Effect::Save => self.save(ctx, frame),
            Effect::SaveAsDialog => self.open_save_dialog(),
            Effect::Edit(key) => self.edit(key, ctx, frame),
            Effect::About => self.about_open = true,
            Effect::OpenRecent(index) => {
                let Some(path) = self.recent.paths().get(index).cloned() else {
                    return;
                };
                if path.exists() {
                    self.request(PendingAction::Open(Some(path)), ctx, frame);
                } else {
                    self.recent.remove(&path);
                    if let Some(file) = &self.recent_file {
                        let _ = self.recent.save(file);
                    }
                    self.error = Some(format!("Can't find {}", file_name(&path)));
                }
            }
            Effect::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Effect::ToggleHideRest => {
                if let Some(editor) = &mut self.editor {
                    let hide = !editor.hide_rest();
                    editor.set_hide_rest(hide);
                }
                self.show_editor_path(frame);
            }
            Effect::MakeComponent { group } => {
                self.name_prompt = Some(chrome::NamePrompt::new(group));
            }
            Effect::Explode => {
                let changed =
                    if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) {
                        match document.kernel.as_mut() {
                            Some(model) => editor.explode_selection(model),
                            None => false,
                        }
                    } else {
                        false
                    };
                if changed {
                    self.refresh_document(ctx, frame);
                }
            }
            Effect::Erase => {
                let changed =
                    if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) {
                        match document.kernel.as_mut() {
                            Some(model) => editor.erase_selection(model),
                            None => false,
                        }
                    } else {
                        false
                    };
                if changed {
                    self.refresh_document(ctx, frame);
                }
            }
            Effect::Copy => {
                if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model)
                    && let Some(model) = document.kernel.as_ref()
                {
                    editor.copy(model);
                }
                self.dirty = true;
            }
            Effect::Cut => {
                let changed =
                    if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) {
                        match document.kernel.as_mut() {
                            Some(model) => editor.cut(model),
                            None => false,
                        }
                    } else {
                        false
                    };
                if changed {
                    self.refresh_document(ctx, frame);
                } else {
                    self.dirty = true;
                }
            }
            Effect::Paste => {
                let started =
                    if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) {
                        match document.kernel.as_ref() {
                            Some(model) => editor.paste(model),
                            None => false,
                        }
                    } else {
                        false
                    };
                if started {
                    self.refresh_document(ctx, frame);
                } else {
                    self.dirty = true;
                }
            }
            Effect::Flip(axis) => {
                let changed =
                    if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) {
                        match document.kernel.as_mut() {
                            Some(model) => editor.flip(axis, model),
                            None => false,
                        }
                    } else {
                        false
                    };
                if changed {
                    self.refresh_document(ctx, frame);
                }
            }
        }
    }

    /// Keyboard shortcuts and dropped files (window-wide).
    fn global_input(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let (events, dropped) = ctx.input(|i| (i.events.clone(), i.raw.dropped_files.clone()));
        if let Some(path) = dropped.into_iter().find_map(|file| {
            let path = file.path();
            (!path.as_os_str().is_empty()).then(|| path.to_path_buf())
        }) {
            self.request(PendingAction::Open(Some(path)), ctx, frame);
            return;
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let mods = ctx.input(|i| to_modifiers(i.modifiers));
        if !mods.ctrl && !mods.alt {
            for text in events.iter().filter_map(|event| match event {
                Event::Text(text) => Some(text.clone()),
                _ => None,
            }) {
                if let Some(editor) = &mut self.editor {
                    editor.text(&text);
                }
            }
        }
        for event in events {
            if let Event::Key {
                key,
                pressed: true,
                repeat: false,
                modifiers,
                ..
            } = event
            {
                let mods = to_modifiers(modifiers);
                // Cut, Copy and Paste are not in `input::key` so the shape letters keep
                // their plain meanings on every platform.
                let action = match key.name() {
                    "C" if mods.ctrl && !mods.shift && !mods.alt => Some(input::Action::Copy),
                    "X" if mods.ctrl && !mods.shift && !mods.alt => Some(input::Action::Cut),
                    "V" if mods.ctrl && !mods.shift && !mods.alt => Some(input::Action::Paste),
                    _ => input::key(key.name(), mods),
                };
                let Some(action) = action else {
                    continue;
                };
                let typing = self
                    .editor
                    .as_ref()
                    .is_some_and(|editor| !editor.typed().is_empty());
                let editor_key = matches!(
                    action,
                    input::Action::Edit(
                        input::EditKey::Enter
                            | input::EditKey::Cancel
                            | input::EditKey::Backspace
                            | input::EditKey::Undo
                            | input::EditKey::Redo
                            | input::EditKey::Lock(_)
                    )
                );
                if typing && !editor_key {
                    continue;
                }
                self.apply(action, ctx, frame);
            }
        }
    }

    /// Mouse input over the viewport rectangle.
    fn viewport_input(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        ui: &egui::Ui,
        rect: Rect,
        response: &egui::Response,
    ) {
        let ndc = |p: Pos2| {
            DVec2::new(
                f64::from((p.x - rect.left()) / rect.width()) * 2.0 - 1.0,
                1.0 - f64::from((p.y - rect.top()) / rect.height()) * 2.0,
            )
        };
        let local = |p: Pos2| DVec2::new(f64::from(p.x - rect.left()), f64::from(p.y - rect.top()));
        let (mods, press_origin, events) = ui.input(|i| {
            (
                to_modifiers(i.modifiers),
                i.pointer.press_origin(),
                i.events.clone(),
            )
        });
        let tool = self.editor.as_ref().map(Editor::tool);
        for (egui_button, button) in [
            (PointerButton::Primary, Button::Left),
            (PointerButton::Middle, Button::Middle),
            (PointerButton::Secondary, Button::Right),
        ] {
            if response.drag_started_by(egui_button)
                && let Some(origin) = press_origin.or(response.interact_pointer_pos())
            {
                self.drag_start_ndc = ndc(origin);
                let orbits = match button {
                    Button::Middle => !mods.shift,
                    Button::Left => self.view.tool == Tool::Orbit && !mods.shift,
                    Button::Right => false,
                };
                if orbits {
                    let view = EditorViewport {
                        camera: self.view.camera,
                        size_px: (f64::from(rect.width()), f64::from(rect.height())),
                    };
                    let (ray_origin, ray_dir) = view.ray(local(origin));
                    self.view.pivot = self
                        .model
                        .as_ref()
                        .and_then(|document| document.scene.raycast(ray_origin, ray_dir))
                        .or_else(|| self.view.camera.ground_pivot(ray_origin, ray_dir));
                }
                if egui_button == PointerButton::Primary {
                    // Only a Select drag is a box; an Orbit drag must not leave a start behind.
                    self.drag_start_px = (tool == Some(Tool::Select)).then(|| local(origin));
                    if tool == Some(Tool::Scale) {
                        let view = EditorViewport {
                            camera: self.view.camera,
                            size_px: (f64::from(rect.width()), f64::from(rect.height())),
                        };
                        if self.editor_click(&view, local(origin), mods, 1) {
                            self.refresh_document(ctx, frame);
                        }
                    }
                }
            }
            if response.dragged_by(egui_button) {
                let delta = response.drag_delta();
                let delta = DVec2::new(f64::from(delta.x), f64::from(delta.y));
                if let Some(action) =
                    input::drag(self.view.tool, button, mods, delta, self.drag_start_ndc)
                {
                    self.apply(action, ctx, frame);
                }
            }
        }
        if response.drag_stopped() {
            self.view.pivot = None;
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
                        self.apply(input::scroll(lines, ndc(cursor)), ctx, frame);
                    }
                }
            }
        }
        let Some(tool) = tool else {
            return;
        };
        if tool == Tool::Tape
            && let Some(editor) = &mut self.editor
        {
            editor.set_point_guides(mods.ctrl);
        }
        let drawing = matches!(
            tool,
            Tool::Select
                | Tool::Line
                | Tool::Rectangle
                | Tool::Circle
                | Tool::Arc
                | Tool::Arc3
                | Tool::PushPull
                | Tool::Rotate
                | Tool::Move
                | Tool::Scale
                | Tool::Tape
                | Tool::Erase
                | Tool::Paint
        );
        if !drawing {
            return;
        }
        let view = EditorViewport {
            camera: self.view.camera,
            size_px: (f64::from(rect.width()), f64::from(rect.height())),
        };
        let cursor = response
            .interact_pointer_pos()
            .or_else(|| response.hover_pos());
        if let Some(cursor) = cursor {
            self.editor_pointer_move(&view, local(cursor), mods);
        }
        if response.drag_stopped_by(PointerButton::Primary) {
            if tool == Tool::Select {
                if let (Some(start), Some(end)) = (
                    self.drag_start_px.take(),
                    response.interact_pointer_pos().map(local),
                ) {
                    self.editor_box_select(&view, start, end, mods);
                }
            } else if tool == Tool::Scale {
                if let Some(end) = cursor
                    && self.editor_click(&view, local(end), mods, 1)
                {
                    self.refresh_document(ctx, frame);
                }
                self.drag_start_px = None;
            } else {
                self.drag_start_px = None;
            }
        }
        let count = if response.triple_clicked_by(PointerButton::Primary) {
            3
        } else if response.double_clicked_by(PointerButton::Primary) {
            2
        } else if response.clicked_by(PointerButton::Primary) {
            1
        } else {
            0
        };
        if count > 0
            && let Some(cursor) = response.interact_pointer_pos()
            && self.editor_click(&view, local(cursor), mods, count)
        {
            self.refresh_document(ctx, frame);
        }
    }

    fn editor_pointer_move(&mut self, view: &EditorViewport, cursor: DVec2, mods: Modifiers) {
        let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) else {
            return;
        };
        let Some(model) = document.kernel.as_mut() else {
            return;
        };
        editor.pointer_move(model, view, cursor, mods);
    }

    fn editor_click(
        &mut self,
        view: &EditorViewport,
        cursor: DVec2,
        mods: Modifiers,
        count: u32,
    ) -> bool {
        let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) else {
            return false;
        };
        let Some(model) = document.kernel.as_mut() else {
            return false;
        };
        editor.click(model, view, cursor, mods, count)
    }

    fn editor_box_select(
        &mut self,
        view: &EditorViewport,
        start: DVec2,
        end: DVec2,
        mods: Modifiers,
    ) {
        let (Some(editor), Some(document)) = (&mut self.editor, &self.model) else {
            return;
        };
        let Some(model) = document.kernel.as_ref() else {
            return;
        };
        editor.box_select(model, view, start, end, mods);
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

    /// Draw the editor's preview, hint, selection and draw plane over the rendered view.
    fn paint_overlays(&self, ui: &egui::Ui, rect: Rect) {
        let (Some(editor), Some(document)) = (&self.editor, &self.model) else {
            return;
        };
        let view = EditorViewport {
            camera: self.view.camera,
            size_px: (f64::from(rect.width()), f64::from(rect.height())),
        };
        let project = |p: DVec3| {
            view.project(p)
                .map(|q| rect.min + egui::vec2(q.x as f32, q.y as f32))
        };
        let painter = ui.painter();
        if editor.tool() == Tool::Select
            && let Some(start) = self.drag_start_px
            && let Some(now) = ui.input(|i| i.pointer.latest_pos())
        {
            let a = rect.min + egui::vec2(start.x as f32, start.y as f32);
            let band = Rect::from_two_pos(a, now);
            // Left to right is a window box, right to left a crossing box.
            let color = if now.x >= a.x {
                ACCENT_COLOR
            } else {
                Color32::from_rgb(60, 160, 60)
            };
            painter.rect_filled(band, 0.0, color.gamma_multiply(0.15));
            painter.rect_stroke(
                band,
                0.0,
                egui::Stroke::new(1.0, color),
                egui::StrokeKind::Inside,
            );
        }
        let plane = editor.draw_plane();
        let preview_color = preview_color(plane);
        for (a, b) in editor.preview() {
            if let (Some(a), Some(b)) = (project(a), project(b)) {
                painter.line_segment([a, b], egui::Stroke::new(1.5, preview_color));
            }
        }
        for (a, b) in editor.paste_preview() {
            if let (Some(a), Some(b)) = (project(a), project(b)) {
                painter.line_segment([a, b], egui::Stroke::new(1.5, preview_color));
            }
        }
        if editor.pasting() {
            painter.text(
                rect.left_top() + egui::vec2(12.0, 12.0),
                egui::Align2::LEFT_TOP,
                "Click to place the paste (Esc cancels)",
                egui::FontId::proportional(13.0),
                Color32::from_rgb(220, 220, 220),
            );
        }
        if let Some(kernel) = &document.kernel {
            let open_body = editor.path().body(kernel).unwrap_or(BodyId::Root);
            let open_transform = editor.path().transform(kernel).unwrap_or(DMat4::IDENTITY);
            let primitives =
                overlay::selection(kernel, open_body, open_transform, editor.selection());
            paint_primitives(painter, &primitives, &project);
            if let Some(face) = editor.pushpull_face()
                && let Some(body) = kernel.body(open_body)
            {
                let primitives = overlay::face_highlight(body.geometry(), face);
                let primitives = overlay::transformed(primitives, open_transform);
                paint_primitives(painter, &primitives, &project);
            }
            if let Some(tool) = editor.scale_tool() {
                let bounds = tool
                    .preview()
                    .or_else(|| scale::bounds(kernel, open_body, editor.selection()));
                if let Some(bounds) = bounds {
                    let primitives = overlay::scale_handles(
                        &bounds,
                        editor.scale_hover(),
                        tool.handle_grabbed(),
                    );
                    let primitives = overlay::transformed(primitives, open_transform);
                    paint_primitives(painter, &primitives, &project);
                }
            }
            let primitives = overlay::guides(
                kernel.root().guides(),
                self.view.camera.target,
                self.view.camera.distance * 10.0,
            );
            paint_primitives(painter, &primitives, &project);
        }
        if let Some(tool) = editor.rotate_tool() {
            // Before the center click the protractor follows the cursor, so the plane it
            // would rotate in shows; until the start click it keeps a fixed screen size.
            let center = tool.center().or_else(|| editor.hint().map(|h| h.point));
            if let Some(center) = center {
                let preview = tool.preview();
                let (radius, start) = match preview.first() {
                    Some((_, point)) if tool.center().is_some() => {
                        (point.distance(center), Some(*point - center))
                    }
                    _ => (protractor_radius(&view, center, tool.plane()), None),
                };
                let primitives =
                    overlay::protractor(center, tool.plane(), radius, start, tool.angle());
                paint_primitives(painter, &primitives, &project);
            }
        }
        if let Some(hint) = editor.hint()
            && let Some(point) = project(hint.point)
        {
            let label = hint_label(&hint);
            let color = Color32::from_rgb(label.color[0], label.color[1], label.color[2]);
            if hint.axis.is_some() {
                let preview = editor.preview();
                if let Some((anchor, _)) = preview.first()
                    && let Some(anchor) = project(*anchor)
                {
                    painter.line_segment([anchor, point], egui::Stroke::new(1.5, color));
                }
            }

            let square = Rect::from_center_size(point, egui::vec2(8.0, 8.0));
            painter.rect_filled(square, 1.0, color);
            painter.rect_stroke(
                square,
                1.0,
                egui::Stroke::new(1.0, Color32::from_rgb(30, 30, 30)),
                egui::StrokeKind::Outside,
            );

            let mut text = label.text;
            if editor.lock().is_some() {
                text = format!("Locked: {text}");
            }
            let galley = painter.layout_no_wrap(text, egui::FontId::proportional(12.0), color);
            let padding = egui::vec2(5.0, 2.0);
            let size = galley.size() + padding * 2.0;
            let mut min = point + egui::vec2(14.0, 14.0);
            min.x = min.x.min(rect.right() - size.x).max(rect.left());
            min.y = min.y.min(rect.bottom() - size.y).max(rect.top());
            let box_rect = Rect::from_min_size(min, size);
            painter.rect_filled(box_rect, 3.0, Color32::from_rgb(30, 30, 30));
            painter.rect_stroke(
                box_rect,
                3.0,
                egui::Stroke::new(1.0, color),
                egui::StrokeKind::Inside,
            );
            painter.galley(box_rect.min + padding, galley, color);
        }
    }

    /// The measurement box at the bottom right of the viewport.
    fn measurement_box(&self, ctx: &egui::Context, rect: Rect) {
        let Some(editor) = &self.editor else {
            return;
        };
        let pos = rect.right_bottom() - egui::vec2(226.0, 74.0);
        egui::Area::new(egui::Id::new("measurement-box"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(210.0);
                    match editor.measurement() {
                        Some((label, value)) => {
                            ui.horizontal(|ui| {
                                ui.label(label);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.monospace(&value);
                                    },
                                );
                            });
                        }
                        None => {
                            ui.label("");
                        }
                    }
                    if !editor.typed().is_empty() {
                        ui.separator();
                        ui.monospace(editor.typed());
                    }
                });
            });
    }

    fn menu_state(&self) -> MenuState<'_> {
        MenuState {
            undo: self
                .editor
                .as_ref()
                .and_then(|editor| editor.undo_stack().undo_label()),
            redo: self
                .editor
                .as_ref()
                .and_then(|editor| editor.undo_stack().redo_label()),
            parallel: self.view.camera.projection == Projection::Parallel,
            has_selection: self
                .editor
                .as_ref()
                .is_some_and(|editor| !editor.selection().is_empty()),
            has_clipboard: self
                .editor
                .as_ref()
                .and_then(Editor::clipboard)
                .is_some_and(|clipboard| !clipboard.is_empty()),
            can_explode: self.editor.as_ref().is_some_and(|editor| {
                matches!(
                    editor.selection(),
                    [modeler_kernel::select::Entity::Instance(_)]
                )
            }),
            hide_rest: self
                .editor
                .as_ref()
                .is_some_and(|editor| editor.hide_rest()),
            recent: self.recent.paths(),
            tool: self.view.tool,
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
                if let Some(message) = self.editor.as_ref().and_then(Editor::message) {
                    ui.colored_label(ERROR_COLOR, message);
                } else {
                    let stats = &model.stats;
                    ui.label(format!(
                        "{} triangles, {} lines, {} materials, {}",
                        stats.triangles,
                        stats.lines,
                        stats.materials,
                        format_size(stats.size_m)
                    ));
                    if model.kernel.is_some() {
                        ui.separator();
                        ui.label(format!("{} faces, {} edges", stats.faces, stats.edges));
                    }
                }
                if let Some(error) = &model.kernel_error {
                    ui.separator();
                    ui.colored_label(ERROR_COLOR, error);
                }
            }
            if let Some(saved) = &self.saved {
                ui.separator();
                ui.label(format!("Saved {saved}"));
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
                    Tool::Select => "Select",
                    Tool::Line => "Line",
                    Tool::Rectangle => "Rectangle",
                    Tool::Circle => "Circle",
                    Tool::Arc => "Arc",
                    Tool::Arc3 => "3-Point Arc",
                    Tool::PushPull => "Push/Pull",
                    Tool::Rotate => "Rotate",
                    Tool::Move => "Move",
                    Tool::Scale => "Scale",
                    Tool::Tape => "Tape Measure",
                    Tool::Erase => "Erase",
                    Tool::Paint => "Paint Bucket",
                });
            });
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|input| input.viewport().close_requested()) && !self.allow_close {
            let modified = self.model.as_ref().is_some_and(|model| model.modified);
            if modified {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                if self.confirm.is_none() && self.dialog.is_none() && self.save_dialog.is_none() {
                    self.confirm = Some(PendingAction::Close);
                }
            } else {
                self.allow_close = true;
            }
        }
        self.poll_loading(&ctx, frame);

        let mut menu_action = None;
        egui::Panel::top("menu").show(ui, |ui| {
            menu_action = chrome::menu_bar(ui, &self.menu_state());
        });
        let mut tool_action = None;
        egui::Panel::left("toolbar").show(ui, |ui| {
            tool_action = chrome::toolbar(ui, self.view.tool);
        });
        if let Some(action) = menu_action.or(tool_action) {
            self.apply(action, &ctx, frame);
        }

        let mut material_action = None;
        let mut texture_action = None;
        if let Some(model) = self
            .model
            .as_ref()
            .and_then(|document| document.kernel.as_ref())
        {
            let current = self.editor.as_ref().and_then(Editor::material);
            let target = self.texture_target(model);
            egui::Panel::right("materials")
                .resizable(true)
                .default_size(180.0)
                .show(ui, |ui| {
                    // Texture Position stays in view at the bottom; a model's material
                    // list can be far taller than the window, so it scrolls.
                    egui::Panel::bottom("texture-position").show(ui, |ui| {
                        if let Some(op) = texture_panel(ui, &mut self.texture, target.is_some())
                            && let Some((face, side)) = target
                        {
                            texture_action = Some((face, side, op));
                        }
                    });
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            material_action =
                                materials_panel(ui, &mut self.materials, model, current);
                        });
                });
        }
        if let Some(action) = material_action {
            self.apply_material_action(action, &ctx, frame);
        }
        if let Some((face, side, op)) = texture_action {
            let changed =
                if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) {
                    match document.kernel.as_mut() {
                        Some(model) => editor.position_texture(model, face, side, op),
                        None => false,
                    }
                } else {
                    false
                };
            if changed {
                self.refresh_document(&ctx, frame);
            }
        }

        let dialog_open = self.dialog.is_some()
            || self.save_dialog.is_some()
            || self.confirm.is_some()
            || self.name_prompt.is_some();
        if !dialog_open {
            self.global_input(&ctx, frame);
            self.sync_editor_path(&ctx, frame);
        }
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
            self.viewport_pt = (f64::from(rect.width()), f64::from(rect.height()));
            if !dialog_open {
                self.viewport_input(&ctx, frame, ui, rect, &response);
                self.sync_editor_path(&ctx, frame);
            }
            self.paint_viewport(ui, frame, rect);
            self.paint_overlays(ui, rect);
            self.measurement_box(&ctx, rect);
        });
        chrome::about_window(&ctx, &mut self.about_open);
        if let Some(prompt) = &mut self.name_prompt {
            match chrome::name_prompt(&ctx, prompt) {
                Some(chrome::PromptOutcome::Ok(name)) => {
                    let group = prompt.group;
                    self.name_prompt = None;
                    let changed = if let (Some(editor), Some(document)) =
                        (&mut self.editor, &mut self.model)
                    {
                        match document.kernel.as_mut() {
                            Some(model) => editor.make_component(model, &name, group),
                            None => false,
                        }
                    } else {
                        false
                    };
                    if changed {
                        self.refresh_document(&ctx, frame);
                    }
                    self.dirty = true;
                }
                Some(chrome::PromptOutcome::Cancel) => {
                    self.name_prompt = None;
                    self.dirty = true;
                }
                None => {}
            }
        }
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
        if let Some(dialog) = &mut self.save_dialog {
            match dialog.show(&ctx) {
                SaveOutcome::Save(path) => {
                    self.save_dialog = None;
                    let state = self.undo_state();
                    let result = self.model.as_mut().map(|document| document.save_as(&path));
                    match result {
                        Some(Ok(written)) => {
                            if let Some(document) = &mut self.model {
                                document.mark_saved_at(state);
                            }
                            self.error = None;
                            self.saved = Some(file_name(&written));
                            self.remember_recent(&written);
                            self.saved_now(&ctx, frame);
                        }
                        Some(Err(message)) => {
                            self.error = Some(message);
                            self.after_save = None;
                        }
                        None => {}
                    }
                }
                SaveOutcome::Cancel => {
                    self.save_dialog = None;
                    self.after_save = None;
                }
                SaveOutcome::Pending => {}
            }
        }
        if let Some(action) = self.confirm.clone() {
            let name = self
                .model
                .as_ref()
                .map(|model| file_name(&model.path))
                .unwrap_or_else(|| "this model".to_owned());
            match unsaved_dialog(&ctx, &name) {
                Some(UnsavedChoice::Save) => {
                    self.confirm = None;
                    self.save_before(&ctx, frame);
                }
                Some(UnsavedChoice::Discard) => {
                    self.confirm = None;
                    self.perform(action, &ctx, frame);
                }
                Some(UnsavedChoice::Cancel) => self.confirm = None,
                None => {}
            }
        }
    }
}

/// What the user asked for that may need the document saved first.
#[derive(Clone, Debug)]
enum PendingAction {
    New,
    Open(Option<PathBuf>),
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnsavedChoice {
    Save,
    Discard,
    Cancel,
}

fn unsaved_dialog(ctx: &egui::Context, name: &str) -> Option<UnsavedChoice> {
    let mut choice = None;
    let response = egui::Modal::new(egui::Id::new("unsaved-changes")).show(ctx, |ui| {
        ui.set_width(360.0);
        ui.heading("Unsaved changes");
        ui.label(format!("Save changes to {name}?"));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                choice = Some(UnsavedChoice::Save);
            }
            if ui.button("Discard").clicked() {
                choice = Some(UnsavedChoice::Discard);
            }
            if ui.button("Cancel").clicked() {
                choice = Some(UnsavedChoice::Cancel);
            }
        });
    });
    if choice.is_none() && response.should_close() {
        choice = Some(UnsavedChoice::Cancel);
    }
    choice
}

/// Stroke colour and width for an overlay primitive.
fn overlay_style(style: overlay::Style) -> (Color32, f32) {
    match style {
        overlay::Style::FaceHighlight => (ACCENT_COLOR, 2.0),
        overlay::Style::BoundsBox => (Color32::from_rgb(130, 140, 155), 1.0),
        overlay::Style::Handle => (Color32::from_rgb(190, 200, 215), 1.0),
        overlay::Style::HandleHover => (Color32::from_rgb(255, 220, 120), 1.5),
        overlay::Style::HandleGrabbed => (Color32::from_rgb(255, 255, 255), 2.0),
        overlay::Style::ProtractorRing => (Color32::from_rgb(120, 200, 210), 1.5),
        overlay::Style::ProtractorTick => (Color32::from_rgb(120, 200, 210), 1.0),
        overlay::Style::ProtractorArm => (Color32::from_rgb(220, 240, 245), 2.0),
        overlay::Style::Guide => (Color32::from_rgb(160, 160, 160), 1.0),
        overlay::Style::GuidePoint => (Color32::from_rgb(220, 220, 220), 1.0),
        overlay::Style::Selected | overlay::Style::SelectedFace => (ACCENT_COLOR, 3.0),
    }
}

/// Paint overlay primitives, projecting world points into the viewport.
/// The world radius that draws `PROTRACTOR_PX` pixels wide at `center` in `plane`.
fn protractor_radius(view: &EditorViewport, center: DVec3, plane: DrawPlane) -> f64 {
    let step = view.camera.distance.max(1e-6) * 0.01;
    let (Some(a), Some(b)) = (view.project(center), view.project(center + plane.u * step)) else {
        return 0.0;
    };
    let px = a.distance(b);
    if px > 1e-9 {
        PROTRACTOR_PX * step / px
    } else {
        0.0
    }
}

fn paint_primitives(
    painter: &egui::Painter,
    primitives: &[Primitive],
    project: &dyn Fn(DVec3) -> Option<Pos2>,
) {
    for primitive in primitives {
        match primitive {
            Primitive::Line { a, b, style } => {
                let (color, width) = overlay_style(*style);
                if let (Some(a), Some(b)) = (project(*a), project(*b)) {
                    painter.line_segment([a, b], egui::Stroke::new(width, color));
                }
            }
            Primitive::Polygon { points, style } => {
                let (color, width) = overlay_style(*style);
                let projected: Vec<Pos2> = points.iter().filter_map(|p| project(*p)).collect();
                if projected.len() < 2 {
                    continue;
                }
                if *style == overlay::Style::SelectedFace {
                    let fill = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 55);
                    painter.add(egui::Shape::convex_polygon(
                        projected.clone(),
                        fill,
                        egui::Stroke::NONE,
                    ));
                    continue;
                }
                if *style == overlay::Style::FaceHighlight {
                    let fill = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 45);
                    painter.add(egui::Shape::convex_polygon(
                        projected.clone(),
                        fill,
                        egui::Stroke::NONE,
                    ));
                }
                for (i, &a) in projected.iter().enumerate() {
                    let b = projected[(i + 1) % projected.len()];
                    painter.line_segment([a, b], egui::Stroke::new(width, color));
                }
            }
            Primitive::Marker { at, style } => {
                let (color, _) = overlay_style(*style);
                let Some(at) = project(*at) else {
                    continue;
                };
                if *style == overlay::Style::GuidePoint {
                    let r = 3.0;
                    painter.line_segment(
                        [at + egui::vec2(-r, 0.0), at + egui::vec2(r, 0.0)],
                        egui::Stroke::new(1.0, color),
                    );
                    painter.line_segment(
                        [at + egui::vec2(0.0, -r), at + egui::vec2(0.0, r)],
                        egui::Stroke::new(1.0, color),
                    );
                } else {
                    let square = Rect::from_center_size(at, egui::vec2(7.0, 7.0));
                    painter.rect_filled(square, 0.0, color);
                    painter.rect_stroke(
                        square,
                        0.0,
                        egui::Stroke::new(1.0, Color32::from_rgb(25, 25, 25)),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        }
    }
}

/// The preview is dark; a non-ground draw plane tints it to show its normal.
fn preview_color(plane: Option<DrawPlane>) -> Color32 {
    let Some(plane) = plane else {
        return PREVIEW_COLOR;
    };
    let normal = plane.normal();
    if normal.z.abs() > 0.9 {
        PREVIEW_COLOR
    } else if normal.y.abs() > normal.x.abs() {
        Color32::from_rgb(30, 95, 75)
    } else {
        Color32::from_rgb(110, 45, 45)
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
                    let is_model = path.extension().is_some_and(|ext| {
                        ext.eq_ignore_ascii_case("dae") || ext.eq_ignore_ascii_case("modeler")
                    });
                    if (is_dir && !name.starts_with('.')) || (!is_dir && is_model) {
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
                    .hint_text("Path to a .dae or .modeler file, or a folder"),
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

enum SaveOutcome {
    Pending,
    Save(PathBuf),
    Cancel,
}

/// A minimal save-file dialog in plain egui: browse folders, list `.modeler` files,
/// and choose a file name.
struct SaveDialog {
    dir: PathBuf,
    /// Subfolders and `.modeler` files of `dir`, folders first, each sorted by name.
    entries: Vec<(String, PathBuf, bool)>,
    name_text: String,
    error: Option<String>,
    /// A target that already exists and awaits confirmation.
    replacing: Option<PathBuf>,
}

impl SaveDialog {
    fn new(dir: PathBuf, name: String) -> SaveDialog {
        let mut dialog = SaveDialog {
            dir: PathBuf::new(),
            entries: Vec::new(),
            name_text: name,
            error: None,
            replacing: None,
        };
        dialog.enter(dir);
        dialog
    }

    fn enter(&mut self, dir: PathBuf) {
        self.error = None;
        self.replacing = None;
        self.entries.clear();
        match std::fs::read_dir(&dir) {
            Ok(read_dir) => {
                for entry in read_dir.flatten() {
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let is_dir = path.is_dir();
                    let is_model = path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("modeler"));
                    if (is_dir && !name.starts_with('.')) || (!is_dir && is_model) {
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
        self.dir = dir;
    }

    fn target(&self) -> Option<PathBuf> {
        let name = self.name_text.trim();
        (!name.is_empty()).then(|| native_path(&self.dir.join(name)))
    }

    fn show(&mut self, ctx: &egui::Context) -> SaveOutcome {
        let mut outcome = SaveOutcome::Pending;
        let mut enter = None;
        // Cancel on the replace question goes back to the file name.
        let mut back = false;
        let response = egui::Modal::new(egui::Id::new("save-dialog")).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.heading("Save model");
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
                                self.name_text = name.clone();
                                self.replacing = None;
                            }
                        }
                    }
                });
            ui.separator();
            if let Some(target) = &self.replacing {
                ui.label(format!("Replace {}?", file_name(target)));
                ui.horizontal(|ui| {
                    if ui.button("Replace").clicked() {
                        outcome = SaveOutcome::Save(target.clone());
                    }
                    if ui.button("Cancel").clicked() {
                        back = true;
                    }
                });
            } else {
                let name = ui.add(
                    egui::TextEdit::singleline(&mut self.name_text)
                        .desired_width(f32::INFINITY)
                        .hint_text("File name"),
                );
                let submitted = name.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if let Some(error) = &self.error {
                    ui.colored_label(ERROR_COLOR, error);
                }
                ui.horizontal(|ui| {
                    if (ui.button("Save").clicked() || submitted)
                        && let Some(target) = self.target()
                    {
                        if target.exists() {
                            self.replacing = Some(target);
                        } else {
                            outcome = SaveOutcome::Save(target);
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = SaveOutcome::Cancel;
                    }
                });
            }
        });
        if back {
            self.replacing = None;
        }
        if let Some(dir) = enter {
            self.enter(dir);
        }
        if matches!(outcome, SaveOutcome::Pending) && response.should_close() {
            outcome = SaveOutcome::Cancel;
        }
        outcome
    }
}
