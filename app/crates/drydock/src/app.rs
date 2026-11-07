// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The viewer window: an egui status bar around a viewport that shows the
//! offscreen-rendered scene. Behaviour: `docs/specs/m0-app.md`.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use drydock::chrome::{self, MenuState};
use drydock::clearance_offset::selected_instances;
use drydock::components_panel::{ComponentAction, ComponentsPanel, components_panel};
use drydock::dialog;
use drydock::dock::{
    self, DockLayout, PanelEvent, PanelId, Side as DockSide, StackRow, apply_drop,
    drop_destination, panel_header,
};
use drydock::document::{
    Document, FileKind, NATIVE_EXTENSION, is_openable_file, native_path, resolve_texture_path,
};
use drydock::editor::{Editor, TextureOp, Viewport as EditorViewport};
use drydock::entity_info::{EntityInfoPanel, InfoAction, entity_info_panel};
use drydock::fit_check::{self, FitCheck, selected_pair};
use drydock::guide_help::{self, SystemBrowser};
use drydock::hints::hint_label;
use drydock::icons::{self, Icon};
use drydock::input::{self, Button, Modifiers, Tool};
use drydock::look;
use drydock::materials_panel::{MaterialAction, MaterialsPanel, materials_panel};
use drydock::mesh_export::{
    self, ExportDialog, ExportFormat, ExportOptions, ExportOutcome, StlLayout,
};
use drydock::mesh_import::{self, MeshUnit, UnitsOutcome};
use drydock::model_source::{
    entry_for_path, import_component_from, import_meshes_from, local_source_for,
};
use drydock::open_buttons::{self, OpenMode};
use drydock::overlay::{self, Primitive};
use drydock::portal;
use drydock::print_setup::{
    self, FitReport, PrintSettings, PrintSetupEvent, PrintSetupTexts, PrintSetupView,
};
use drydock::recent::{RecentFiles, recent_file};
use drydock::shading;
use drydock::shadows_panel::{self, ShadowsEvent};
use drydock::solid_ui::{MarkTracker, mark_primitives, marked_scene, repair_message};
use drydock::sun_windows::SunWindows;
use drydock::tags_panel::{TagAction, TagsPanel, default_tag_name, tags_panel};
use drydock::text_tool::{self, TextDialog, TextOptions, TextOutcome};
use drydock::texture_handles;
use drydock::texture_panel::{TextureForm, texture_position_section};
use drydock::toolbars::{self, CustomizeState, Edge, Placement, ToolbarEvent, ToolbarLayout};
use drydock::units_ui;
use drydock::viewstate::{Effect, ViewState, scene_bounds, zoom_pivot_at};
use drydock_dae::{FlatScene, RestView};
use drydock_kernel::editpath::EditPath;
use drydock_kernel::model::{BodyId, Material, Model};
use drydock_kernel::paint::Side;
use drydock_kernel::select::Entity;
use drydock_kernel::sun::ShadowSettings;
use drydock_kernel::tools::plane::DrawPlane;
use drydock_kernel::tools::scale;
use drydock_kernel::topology::FaceId;
use drydock_render::{GpuScene, Renderer, Style, Viewport};
use drydock_view::Projection;
use eframe::egui;
use egui::{Color32, Event, MouseWheelUnit, PointerButton, Pos2, Rect, Sense, pos2};
use glam::{DMat4, DVec2, DVec3};
use wgpu::{Backend, DeviceType};

/// Scroll distance in points that counts as one wheel notch (touchpads report points).
const POINTS_PER_SCROLL_LINE: f64 = 50.0;
const ERROR_COLOR: Color32 = Color32::from_rgb(200, 60, 50);
const ACCENT_COLOR: Color32 = Color32::from_rgb(230, 140, 25);
const PREVIEW_COLOR: Color32 = Color32::from_rgb(55, 60, 70);
/// The Rotate protractor's radius on screen before its start point is placed.
const PROTRACTOR_PX: f64 = 48.0;

/// The display name of a wgpu backend (`docs/specs/m4-ui-gpu-info.md`).
fn backend_label(backend: Backend) -> &'static str {
    match backend {
        Backend::Vulkan => "Vulkan",
        Backend::Metal => "Metal",
        Backend::Dx12 => "DX12",
        Backend::Gl => "GL",
        Backend::BrowserWebGpu => "BrowserWebGpu",
        Backend::Noop => "Noop",
    }
}

/// The display name of a wgpu device type (`docs/specs/m4-ui-gpu-info.md`).
fn device_type_label(device_type: DeviceType) -> &'static str {
    match device_type {
        DeviceType::Other => "other",
        DeviceType::IntegratedGpu => "integrated GPU",
        DeviceType::DiscreteGpu => "discrete GPU",
        DeviceType::VirtualGpu => "virtual GPU",
        DeviceType::Cpu => "CPU",
    }
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
    /// The View > Model Info window is showing.
    model_info_open: bool,
    /// The Tools > Print Setup window is showing.
    print_setup_open: bool,
    /// The print bed, outline and overhang angle kept between runs.
    print_settings: PrintSettings,
    /// Where `print_settings` is kept.
    print_setup_file: Option<PathBuf>,
    /// The Print Setup boxes' text.
    print_setup_texts: PrintSetupTexts,
    /// The last fits-the-bed result, with the selection and undo state it belongs to.
    print_report: Option<(FitReport, Vec<Entity>, u64)>,
    /// The Shadows and Geolocation windows.
    sun_windows: SunWindows,
    /// The Make Component / Make Group name prompt is showing.
    name_prompt: Option<chrome::NamePrompt>,
    /// The Materials panel and its open form.
    materials: MaterialsPanel,
    /// The docked and floating panel layout.
    dock: DockLayout,
    /// Where `dock` is kept between runs.
    dock_file: Option<PathBuf>,
    /// Each column's panel rows, for the height each panel gets.
    stacks: [Vec<StackRow>; 2],
    /// The Entity Info panel's state.
    entity_info: EntityInfoPanel,
    /// The Components panel's state.
    components: ComponentsPanel,
    /// The Tags panel's state.
    tags: TagsPanel,
    /// The sun settings being dragged in the Shadows panel, not yet recorded.
    shadows_preview: Option<ShadowSettings>,
    /// The Texture Position panel's typed values.
    texture: TextureForm,
    style: Style,
    gpu: Option<Gpu>,
    model: Option<Document>,
    /// One editor per open document; `None` before a document exists.
    editor: Option<Editor>,
    /// The fit check showing, if any.
    fit: Option<FitCheck>,
    /// Why the last Fit Check request was refused, and the selection it was made with.
    fit_note: Option<(String, Vec<Entity>)>,
    /// The clearance field's text, in millimeters.
    clearance_text: String,
    loading: Option<Loading>,
    error: Option<String>,
    /// "Saved <file name>" until the next open or save.
    saved: Option<String>,
    warnings: Vec<String>,
    dialog: Option<OpenDialog>,
    /// The units choice shown before opening or importing an STL.
    units_prompt: Option<UnitsPrompt>,
    /// The unit last picked for a mesh file this session.
    mesh_unit: MeshUnit,
    /// The export dialog showing, if any.
    export_dialog: Option<ExportPrompt>,
    /// The 3D Text dialog showing, if any.
    text_dialog: Option<TextPrompt>,
    /// The 3D Text options last used this session.
    text_options: Option<TextOptions>,
    /// The options last used for each export format this session.
    export_options: Vec<ExportOptions>,
    /// The last export's status line, until the next action.
    export_note: Option<String>,
    save_dialog: Option<SaveDialog>,
    /// A destructive action waiting for the Save/Discard/Cancel question.
    confirm: Option<PendingAction>,
    /// An app-level destructive action waiting for confirmation (`docs/specs/m6-confirm.md`).
    app_confirm: Option<AppConfirm>,
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
    /// The problem marks of the selected group or component.
    solid_marks: MarkTracker,
    /// The last repair's note, with the selection and undo state it belongs to.
    repair_note: Option<(String, Vec<Entity>, u64)>,
    /// The File menu's recent files (`docs/specs/m4-ui-menus.md`).
    recent: RecentFiles,
    /// Where `recent` is kept between runs.
    recent_file: Option<PathBuf>,
    /// The toolbars' buttons, order and placement.
    toolbars: ToolbarLayout,
    /// Where `toolbars` is kept between runs.
    toolbars_file: Option<PathBuf>,
    /// The screen rectangles of the toolbars drawn this frame, by layout index.
    toolbar_rects: Vec<(usize, Rect)>,
    /// The Customize Toolbars window is showing.
    customize_open: bool,
    /// The Customize window's selected toolbar and typed name.
    customize: CustomizeState,
    /// Ctrl/Cmd is held down.
    ctrl_down: bool,
    /// Ctrl/Cmd went down as a lone modifier (no key or button since), so releasing it
    /// toggles Move's copy mode.
    ctrl_lone: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, file: Option<PathBuf>) -> App {
        look::apply(&cc.egui_ctx);
        portal::watch(cc.egui_ctx.clone());
        icons::install(&cc.egui_ctx);
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
        let toolbars_file = toolbars::toolbars_file(|name| std::env::var_os(name));
        let toolbars = toolbars_file
            .as_deref()
            .map(ToolbarLayout::load)
            .unwrap_or_default();
        let dock_file = dock::panels_file(|name| std::env::var_os(name));
        let dock = dock_file
            .as_deref()
            .map(DockLayout::load)
            .unwrap_or_default();
        let print_setup_file = print_setup::settings_file(|name| std::env::var_os(name));
        let print_settings = print_setup_file
            .as_deref()
            .map(PrintSettings::load)
            .unwrap_or_default();
        let print_setup_texts =
            PrintSetupTexts::for_view(&print_settings.bed, print_settings.overhang_degrees);
        let mut app = App {
            view: ViewState::new(),
            about_open: false,
            model_info_open: false,
            print_setup_open: false,
            print_settings,
            print_setup_file,
            print_setup_texts,
            print_report: None,
            sun_windows: SunWindows::new(),
            name_prompt: None,
            materials: MaterialsPanel::default(),
            dock,
            dock_file,
            stacks: [Vec::new(), Vec::new()],
            entity_info: EntityInfoPanel::default(),
            components: ComponentsPanel::default(),
            tags: TagsPanel::default(),
            shadows_preview: None,
            texture: TextureForm::default(),
            style: look::viewport_style(look::theme_of(&cc.egui_ctx.global_style().visuals)),
            gpu,
            model: None,
            editor: None,
            fit: None,
            fit_note: None,
            clearance_text: fit_check::format_mm(drydock_dae::DEFAULT_CLEARANCE_M),
            loading: None,
            error: None,
            saved: None,
            warnings: Vec::new(),
            dialog: None,
            units_prompt: None,
            mesh_unit: MeshUnit::default(),
            export_dialog: None,
            text_dialog: None,
            text_options: None,
            export_options: Vec::new(),
            export_note: None,
            save_dialog: None,
            confirm: None,
            app_confirm: None,
            after_save: None,
            allow_close: false,
            viewport_pt: (1.0, 1.0),
            drag_start_ndc: DVec2::ZERO,
            drag_start_px: None,
            dirty: true,
            shown_path: EditPath::root(),
            solid_marks: MarkTracker::default(),
            repair_note: None,
            recent,
            recent_file,
            toolbars,
            toolbars_file,
            toolbar_rects: Vec::new(),
            customize_open: false,
            customize: CustomizeState::default(),
            ctrl_down: false,
            ctrl_lone: false,
        };
        app.view.bed = app.print_settings.bed;
        app.view.bed_visible = app.print_settings.show_bed;
        app.view.overhang_degrees = app.print_settings.overhang_degrees;
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

    /// Open the STL file at `path` with `unit` on a background thread, as `start_loading`.
    fn start_loading_mesh(&mut self, ctx: &egui::Context, path: PathBuf, unit: MeshUnit) {
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        let thread_path = path.clone();
        std::thread::spawn(move || {
            let result = Document::open_mesh(&thread_path, unit);
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
        self.fit = None;
        self.fit_note = None;
        self.print_report = None;
        self.view
            .set_bounds(scene_bounds(&document.scene), self.viewport_pt);
        self.editor = Some(Editor::new());
        self.materials = MaterialsPanel::default();
        self.texture = TextureForm::default();
        self.shadows_preview = None;
        self.shown_path = EditPath::root();
        if let Some(editor) = &mut self.editor {
            editor.set_tool(self.view.tool);
        }
        let title = document.title();
        self.model = Some(document);
        self.solid_marks.clear();
        self.show_scene(frame);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
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

    /// Bring the problem marks up to date with the open model, edit path, selection and
    /// undo state; true when the marks changed and the scene must be uploaded again.
    fn sync_solid_marks(&mut self) -> bool {
        let undo_state = self.undo_state();
        let Some(editor) = self.editor.as_ref() else {
            return false;
        };
        let Some(model) = self
            .model
            .as_ref()
            .and_then(|document| document.kernel.as_ref())
        else {
            return false;
        };
        self.solid_marks
            .update(model, editor.path(), editor.selection(), undo_state)
    }

    /// Upload the current model's scene with the overhang, fit and problem tints, and keep
    /// its texture warnings.
    fn show_scene(&mut self, frame: &eframe::Frame) {
        self.sync_solid_marks();
        let stale = self.fit.as_ref().is_some_and(|fit| {
            let Some(editor) = &self.editor else {
                return true;
            };
            !fit.is_current(editor.path(), editor.selection(), self.undo_state())
        });
        if stale {
            self.fit = None;
        }
        let Some(scene) = self.model.as_ref().map(|document| &document.scene) else {
            self.dirty = true;
            return;
        };
        let overhang = self.view.overhangs.then_some(self.view.overhang_degrees);
        let display = fit_check::display_scene(scene, overhang, self.fit.as_ref());
        let scene = marked_scene(&display, self.solid_marks.marks());
        if let (Some(gpu), Some(state)) = (&mut self.gpu, frame.wgpu_render_state()) {
            gpu.scene = gpu.renderer.upload(&state.device, &state.queue, &scene);
            self.warnings = gpu.scene.warnings.clone();
        }
        self.dirty = true;
    }

    /// Upload the model's current scene again.
    fn show_current_scene(&mut self, frame: &eframe::Frame) {
        self.show_scene(frame);
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
        let title = {
            let Some(document) = &mut self.model else {
                return;
            };
            document.refresh_editing(&path, rest);
            document.sync_modified(state);
            self.shown_path = path;
            // Zoom Extents frames what is drawn, not the whole model.
            self.view.bounds = scene_bounds(&document.scene);
            document.title()
        };
        self.show_scene(frame);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        self.dirty = true;
    }

    /// Rebuild the scene when the editor's open path changed without a model edit.
    fn sync_editor_path(&mut self, _ctx: &egui::Context, frame: &eframe::Frame) {
        let Some((path, selection)) = self
            .editor
            .as_ref()
            .map(|editor| (editor.path().clone(), editor.selection().to_vec()))
        else {
            return;
        };
        let stale = self
            .fit
            .as_ref()
            .is_some_and(|fit| !fit.is_current(&path, &selection, self.undo_state()));
        if stale {
            self.fit = None;
        }
        if stale || path != self.shown_path {
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
        self.show_scene(frame);
        self.dirty = true;
    }

    /// Replace the document with a new, empty one and a default view.
    fn new_document(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let document = Document::new_empty();
        let title = document.title();
        self.fit = None;
        self.fit_note = None;
        self.print_report = None;
        self.view = ViewState::new();
        self.view.bed = self.print_settings.bed;
        self.view.bed_visible = self.print_settings.show_bed;
        self.view.overhang_degrees = self.print_settings.overhang_degrees;
        self.editor = Some(Editor::new());
        self.materials = MaterialsPanel::default();
        self.texture = TextureForm::default();
        self.shadows_preview = None;
        self.shown_path = EditPath::root();
        self.loading = None;
        self.error = None;
        self.saved = None;
        self.confirm = None;
        self.after_save = None;
        self.model = Some(document);
        self.solid_marks.clear();
        self.show_scene(frame);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        self.dirty = true;
    }

    fn open_dialog(&mut self) {
        self.dialog = Some(OpenDialog::new(self.open_folder()));
    }

    fn open_import_dialog(&mut self) {
        self.dialog = Some(OpenDialog::import(self.open_folder()));
    }

    fn open_models_dialog(&mut self) {
        self.dialog = Some(OpenDialog::models(self.open_folder()));
    }

    fn open_user_guide(&mut self) {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf));
        let message = guide_help::open_guide(
            &guide_help::guide_candidates(exe_dir.as_deref()),
            |path| path.is_file(),
            &SystemBrowser,
        );
        self.export_note = Some(message);
    }

    fn open_folder(&self) -> PathBuf {
        self.model
            .as_ref()
            .and_then(|model| model.path.parent().map(Path::to_path_buf))
            .filter(|dir| dir.is_dir())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Read `path` as a component source, add it to the open model and start placing it.
    fn import_component_file(&mut self, ctx: &egui::Context, frame: &eframe::Frame, path: &Path) {
        let source = local_source_for(path);
        let entry = entry_for_path(path);
        let result = if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) {
            match document.kernel.as_mut() {
                Some(model) => import_component_from(editor, model, &source, &entry).map(|_| ()),
                None => Err(document
                    .kernel_error
                    .clone()
                    .unwrap_or_else(|| format!("{}: no model", file_name(path)))),
            }
        } else {
            Err(format!("{}: no open model", file_name(path)))
        };
        match result {
            Ok(()) => {
                self.error = None;
                self.refresh_document(ctx, frame);
            }
            Err(message) => self.error = Some(message),
        }
    }

    /// Read the mesh file at `path` with `unit`, add its groups to the open model and redraw.
    fn import_mesh_file(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        path: &Path,
        unit: MeshUnit,
    ) {
        let source = local_source_for(path);
        let entry = entry_for_path(path);
        let result = if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) {
            match document.kernel.as_mut() {
                Some(model) => import_meshes_from(editor, model, &source, &entry, unit).map(|_| ()),
                None => Err(document
                    .kernel_error
                    .clone()
                    .unwrap_or_else(|| format!("{}: no model", file_name(path)))),
            }
        } else {
            Err(format!("{}: no open model", file_name(path)))
        };
        match result {
            Ok(()) => {
                self.error = None;
                self.refresh_document(ctx, frame);
            }
            Err(message) => self.error = Some(message),
        }
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
            .unwrap_or_else(|| "model.drydock".to_owned());
        self.save_dialog = Some(SaveDialog::new(dir, name));
    }

    /// Open the save dialog for an export, with a name from the document and the format.
    fn open_export_save_dialog(&mut self, options: ExportOptions) {
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
        let name = mesh_export::default_export_name(&document.path, options.format, options.layout);
        let mode = if options.format == ExportFormat::Stl
            && options.layout == StlLayout::OneFilePerObject
        {
            SaveMode::ExportFolder(options)
        } else {
            SaveMode::Export(options)
        };
        self.save_dialog = Some(SaveDialog::with_mode(dir, name, mode));
    }

    /// Write an export with the open document's kernel and the editor's selection.
    fn export_to(&mut self, path: &Path, options: ExportOptions) {
        let Some(document) = &self.model else {
            return;
        };
        let Some(model) = document.kernel.as_ref() else {
            self.error = Some(format!("{}: no model", file_name(&document.path)));
            return;
        };
        let selection = self
            .editor
            .as_ref()
            .map(|editor| editor.selection().to_vec())
            .unwrap_or_default();
        match mesh_export::export(model, &selection, &options, path) {
            Ok(report) => {
                self.error = None;
                self.export_note = Some(mesh_export::status_message(&report, &options));
            }
            Err(message) => {
                self.export_note = None;
                self.error = Some(message);
            }
        }
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
            PendingAction::Open(Some(path)) => {
                if mesh_import::needs_units(&path) {
                    self.units_prompt = Some(UnitsPrompt {
                        path,
                        purpose: UnitsPurpose::Open,
                        unit: self.mesh_unit,
                    });
                } else {
                    self.start_loading(ctx, path);
                }
            }
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
            MaterialAction::Eyedropper => {
                if let Some(editor) = &mut self.editor {
                    editor.arm_eyedropper();
                }
            }
            MaterialAction::CancelEyedropper => {
                if let Some(editor) = &mut self.editor {
                    editor.disarm_eyedropper();
                }
            }
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

    fn fit_effect(&mut self, effect: Effect, frame: &eframe::Frame) {
        match effect {
            Effect::FitCheck => {
                if self.fit.is_some() {
                    self.fit = None;
                    self.fit_note = None;
                    self.show_current_scene(frame);
                    return;
                }
                let selection = self
                    .editor
                    .as_ref()
                    .map(|editor| editor.selection().to_vec())
                    .unwrap_or_default();
                let result = if let (Some(editor), Some(document)) = (&self.editor, &self.model) {
                    document.kernel.as_ref().map(|model| {
                        FitCheck::run(
                            model,
                            editor.path(),
                            editor.selection(),
                            self.undo_state(),
                            self.view.clearance_m,
                        )
                    })
                } else {
                    None
                };
                match result {
                    Some(Ok(fit)) => {
                        self.fit = Some(fit);
                        self.fit_note = None;
                        self.show_current_scene(frame);
                    }
                    Some(Err(message)) => self.fit_note = Some((message, selection)),
                    None => {}
                }
            }
            Effect::RecomputeFit => {
                let recomputed = if let (Some(fit), Some(document)) = (&self.fit, &self.model) {
                    document
                        .kernel
                        .as_ref()
                        .map(|model| fit.recompute(model, self.view.clearance_m))
                } else {
                    None
                };
                if let Some(fit) = recomputed {
                    self.fit = Some(fit);
                    self.show_current_scene(frame);
                }
            }
            _ => {}
        }
    }

    fn save_print_settings(&self) {
        if let Some(file) = &self.print_setup_file {
            let _ = self.print_settings.save(file);
        }
    }

    fn current_print_report(&self) -> Option<&FitReport> {
        let (report, selection, undo_state) = self.print_report.as_ref()?;
        let current = self
            .editor
            .as_ref()
            .map_or(&[][..], |editor| editor.selection());
        (current == selection.as_slice() && self.undo_state() == *undo_state).then_some(report)
    }

    fn sync_print_report(&mut self) {
        if self.current_print_report().is_none() {
            self.print_report = None;
        }
    }

    fn run_print_fit(&self) -> Option<FitReport> {
        let editor = self.editor.as_ref()?;
        let model = self.model.as_ref()?.kernel.as_ref()?;
        let instances: Option<Vec<_>> = if editor.path().is_root() {
            let ids: Vec<_> = editor
                .selection()
                .iter()
                .filter_map(|entity| match *entity {
                    Entity::Instance(id) => Some(id),
                    _ => None,
                })
                .collect();
            (!ids.is_empty()).then_some(ids)
        } else {
            None
        };
        Some(print_setup::fit_report(
            model,
            instances.as_deref(),
            &self.view.bed,
        ))
    }

    fn print_setup_window(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
        free_area: egui::Rect,
    ) {
        if !self.print_setup_open {
            return;
        }
        self.sync_print_report();
        let mut texts = std::mem::take(&mut self.print_setup_texts);
        let mut open = self.print_setup_open;
        let mut event = PrintSetupEvent::None;
        {
            let view = PrintSetupView {
                bed: self.view.bed,
                show_bed: self.view.bed_visible,
                overhang_degrees: self.view.overhang_degrees,
                can_drop: self.editor.as_ref().is_some_and(|editor| {
                    editor
                        .selection()
                        .iter()
                        .any(|entity| matches!(entity, Entity::Instance(_)))
                }),
                report: self.current_print_report(),
            };
            egui::Window::new("Print Setup")
                .default_pos(print_setup::window_pos(free_area))
                .id(egui::Id::new("print_setup"))
                .open(&mut open)
                .show(ctx, |ui| {
                    event = print_setup::print_setup_ui(ui, &mut texts, &view);
                });
        }
        self.print_setup_texts = texts;
        self.print_setup_open = open;
        match event {
            PrintSetupEvent::None => {}
            PrintSetupEvent::SetBed(bed) => {
                self.view.bed = bed;
                self.print_settings.bed = bed;
                self.print_setup_texts =
                    PrintSetupTexts::for_view(&self.view.bed, self.view.overhang_degrees);
                self.save_print_settings();
                self.print_report = None;
                self.dirty = true;
            }
            PrintSetupEvent::ShowBed(on) => {
                self.view.bed_visible = on;
                self.print_settings.show_bed = on;
                self.save_print_settings();
                self.dirty = true;
            }
            PrintSetupEvent::SetOverhang(degrees) => {
                let effect = self.view.set_overhang_degrees(degrees);
                self.print_settings.overhang_degrees = self.view.overhang_degrees;
                self.save_print_settings();
                if effect == Effect::ReuploadScene {
                    self.show_current_scene(frame);
                    self.dirty = true;
                }
            }
            PrintSetupEvent::CheckFit => {
                if let Some(report) = self.run_print_fit() {
                    let selection = self
                        .editor
                        .as_ref()
                        .map(|editor| editor.selection().to_vec())
                        .unwrap_or_default();
                    let undo_state = self.undo_state();
                    self.print_report = Some((report, selection, undo_state));
                }
            }
            PrintSetupEvent::DropToBed => {
                let changed =
                    if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) {
                        match document.kernel.as_mut() {
                            Some(model) => editor.drop_to_bed(model),
                            None => false,
                        }
                    } else {
                        false
                    };
                if changed {
                    self.refresh_document(ctx, frame);
                }
                self.print_report = None;
            }
        }
    }

    fn apply(&mut self, action: input::Action, ctx: &egui::Context, frame: &eframe::Frame) {
        self.export_note = None;
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
            Effect::ImportDialog => {
                if self.model.is_some() {
                    self.open_models_dialog();
                }
            }
            Effect::ExportDialog(format) => {
                if self.model.is_some() {
                    let options = self
                        .export_options
                        .iter()
                        .find(|options| options.format == format)
                        .copied()
                        .unwrap_or_else(|| ExportOptions::new(format));
                    self.export_dialog = Some(ExportPrompt {
                        dialog: ExportDialog::new(options),
                    });
                }
            }
            Effect::Save => self.save(ctx, frame),
            Effect::SaveAsDialog => self.open_save_dialog(),
            Effect::Edit(key) => self.edit(key, ctx, frame),
            Effect::About => self.about_open = true,
            Effect::UserGuide => self.open_user_guide(),
            Effect::ModelInfo => self.model_info_open = true,
            Effect::ToggleShadows => self.reveal_panel(PanelId::Shadows, ctx),
            Effect::TogglePanel(id) => self.reveal_panel(id, ctx),
            Effect::ResetPanels => {
                self.app_confirm = Some(AppConfirm::ResetPanels);
            }
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
            Effect::FitCheck | Effect::RecomputeFit => self.fit_effect(effect, frame),
            Effect::PrintSetup => self.print_setup_open = true,
            Effect::Text3dDialog => {
                if self.model.is_some() {
                    let dialog = self
                        .text_options
                        .as_ref()
                        .map(TextDialog::from_options)
                        .unwrap_or_default();
                    self.text_dialog = Some(TextPrompt { dialog });
                    self.dirty = true;
                }
            }
            Effect::OffsetByClearance(direction) => {
                let mut note = None;
                let mut changed = 0;
                if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model)
                    && let Some(model) = document.kernel.as_mut()
                    && let Ok(outcome) =
                        editor.offset_instances(model, direction, self.view.clearance_m)
                {
                    changed = outcome.changed;
                    note = Some((
                        outcome.message(),
                        editor.selection().to_vec(),
                        editor.undo_stack().state(),
                    ));
                }
                self.repair_note = note;
                if changed > 0 {
                    self.refresh_document(ctx, frame);
                }
            }
            Effect::ReuploadScene => {
                self.show_current_scene(frame);
                self.dirty = true;
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
                let copied = if let (Some(editor), Some(document)) =
                    (&mut self.editor, &mut self.model)
                    && let Some(model) = document.kernel.as_ref()
                {
                    let count = editor.selection().len();
                    editor.copy(model).then_some(count)
                } else {
                    None
                };
                if let Some(count) = copied {
                    ctx.copy_text(clipboard_text(count, "copied"));
                }
                self.dirty = true;
            }
            Effect::Cut => {
                let (changed, count) =
                    if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) {
                        match document.kernel.as_mut() {
                            Some(model) => {
                                let count = editor.selection().len();
                                (editor.cut(model), count)
                            }
                            None => (false, 0),
                        }
                    } else {
                        (false, 0)
                    };
                if changed {
                    ctx.copy_text(clipboard_text(count, "cut"));
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

    fn track_move_copy_modifier(&mut self, events: &[Event], typing: bool) {
        for event in events {
            match event {
                Event::ModifiersChanged(modifiers) => {
                    let down = modifiers.command;
                    if down && !self.ctrl_down {
                        self.ctrl_down = true;
                        self.ctrl_lone = !typing
                            && self
                                .editor
                                .as_ref()
                                .is_some_and(|editor| editor.tool() == Tool::Move);
                    } else if !down && self.ctrl_down {
                        let toggle = self.ctrl_lone && !typing;
                        self.ctrl_down = false;
                        self.ctrl_lone = false;
                        if toggle
                            && let Some(editor) = &mut self.editor
                            && editor.tool() == Tool::Move
                        {
                            let on = editor.move_copy();
                            editor.set_move_copy(!on);
                        }
                    }
                }
                Event::Key {
                    key, pressed: true, ..
                } => {
                    if !matches!(
                        key,
                        egui::Key::ControlLeft
                            | egui::Key::ControlRight
                            | egui::Key::SuperLeft
                            | egui::Key::SuperRight
                    ) {
                        self.ctrl_lone = false;
                    }
                }
                Event::PointerButton { pressed: true, .. } => self.ctrl_lone = false,
                _ => {}
            }
        }
    }

    /// Keyboard shortcuts and dropped files (window-wide).
    fn global_input(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let (events, dropped) = ctx.input(|i| (i.events.clone(), i.raw.dropped_files.clone()));
        self.track_move_copy_modifier(&events, ctx.egui_wants_keyboard_input());
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
        let typing = self
            .editor
            .as_ref()
            .is_some_and(|editor| !editor.typed().is_empty());
        if !typing {
            for action in chrome::clipboard_actions(&events) {
                self.apply(action, ctx, frame);
            }
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
                let action = input::key(key.name(), mods);
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
        let armed = self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.eyedropper_armed());
        let left_tool = tool.and_then(|tool| input::left_button_tool(tool, armed));
        if response.contains_pointer() && armed {
            ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        }
        if response.contains_pointer()
            && let Some(handles) = self.editor.as_ref().and_then(|editor| {
                self.model
                    .as_ref()
                    .and_then(|document| document.kernel.as_ref())
                    .and_then(|model| editor.texture_handles(model))
            })
        {
            let icon = handles
                .held
                .or(handles.hover)
                .map_or(egui::CursorIcon::Default, |grip| grip.cursor());
            ui.ctx().set_cursor_icon(icon);
        }
        for (egui_button, button) in [
            (PointerButton::Primary, Button::Left),
            (PointerButton::Middle, Button::Middle),
            (PointerButton::Secondary, Button::Right),
        ] {
            if response.drag_started_by(egui_button)
                && let Some(origin) = press_origin.or(response.interact_pointer_pos())
            {
                self.drag_start_ndc = ndc(origin);
                let mut grabbed_texture = false;
                if egui_button == PointerButton::Primary && self.texture_handles_showing() {
                    let view = EditorViewport {
                        camera: self.view.camera,
                        size_px: (f64::from(rect.width()), f64::from(rect.height())),
                    };
                    if self.editor_click(&view, local(origin), mods, 1) {
                        self.refresh_document(ctx, frame);
                    }
                    grabbed_texture = self.texture_grip_held();
                }
                let orbits = match button {
                    Button::Middle => !mods.shift,
                    Button::Left => left_tool == Some(Tool::Orbit) && !mods.shift,
                    Button::Right => false,
                };
                if orbits && !grabbed_texture {
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
                if egui_button == PointerButton::Primary && !grabbed_texture {
                    // Only a Select drag is a box; an Orbit drag must not leave a start behind.
                    self.drag_start_px = (left_tool == Some(Tool::Select)).then(|| local(origin));
                    if left_tool == Some(Tool::Zoom) {
                        let size = (f64::from(rect.width()), f64::from(rect.height()));
                        self.view.zoom_pivot = zoom_pivot_at(
                            &self.view.camera,
                            self.model.as_ref().map(|document| &document.scene),
                            size,
                            local(origin),
                        );
                    }
                    if left_tool == Some(Tool::Scale) {
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
                let texture_drag =
                    egui_button == PointerButton::Primary && self.texture_grip_held();
                let sampling = button == Button::Left && armed;
                if !texture_drag && !sampling {
                    let delta = response.drag_delta();
                    let delta = DVec2::new(f64::from(delta.x), f64::from(delta.y));
                    if let Some(action) =
                        input::drag(self.view.tool, button, mods, delta, self.drag_start_ndc)
                    {
                        self.apply(action, ctx, frame);
                    }
                }
            }
        }
        if response.drag_stopped() {
            self.view.pivot = None;
            self.view.zoom_pivot = None;
        }
        if let Some(cursor) = response.hover_pos() {
            let mut lines = 0.0;
            let mut through = 0.0;
            for event in events {
                if let Event::MouseWheel {
                    unit,
                    delta,
                    modifiers,
                    ..
                } = event
                {
                    let notches = match unit {
                        MouseWheelUnit::Line => f64::from(delta.y),
                        MouseWheelUnit::Point => f64::from(delta.y) / POINTS_PER_SCROLL_LINE,
                        MouseWheelUnit::Page => f64::from(delta.y) * 3.0,
                    };
                    if modifiers.command {
                        through += notches;
                    } else {
                        lines += notches;
                    }
                }
            }
            if lines != 0.0 || through != 0.0 {
                let size = (f64::from(rect.width()), f64::from(rect.height()));
                let scene = self.model.as_ref().map(|document| &document.scene);
                if lines != 0.0 {
                    self.view.scroll_zoom(lines, local(cursor), scene, size);
                }
                if through != 0.0 {
                    self.view
                        .scroll_through(through, local(cursor), scene, size);
                }
                self.dirty = true;
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
                | Tool::Offset
                | Tool::Rotate
                | Tool::Move
                | Tool::Scale
                | Tool::Tape
                | Tool::Erase
                | Tool::Paint
                | Tool::FollowMe
        );
        if !drawing && !armed {
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
            if self.texture_grip_held() {
                if let Some(end) = cursor
                    && self.editor_click(&view, local(end), mods, 1)
                {
                    self.refresh_document(ctx, frame);
                }
                self.drag_start_px = None;
            } else if left_tool == Some(Tool::Select) {
                if let (Some(start), Some(end)) = (
                    self.drag_start_px.take(),
                    response.interact_pointer_pos().map(local),
                ) {
                    self.editor_box_select(&view, start, end, mods);
                }
            } else if left_tool == Some(Tool::Scale) {
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

    fn texture_handles_showing(&self) -> bool {
        let (Some(editor), Some(document)) = (&self.editor, &self.model) else {
            return false;
        };
        document
            .kernel
            .as_ref()
            .is_some_and(|model| editor.texture_handles(model).is_some())
    }

    fn texture_grip_held(&self) -> bool {
        let (Some(editor), Some(document)) = (&self.editor, &self.model) else {
            return false;
        };
        document.kernel.as_ref().is_some_and(|model| {
            editor
                .texture_handles(model)
                .is_some_and(|h| h.held.is_some())
        })
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
        let theme = look::theme_of(&ui.ctx().global_style().visuals);
        let mut extra = if self.view.axes {
            overlay::axis_lines(
                self.open_transform(),
                self.view.camera.distance * 100.0,
                ui.ctx().pixels_per_point(),
                theme,
            )
        } else {
            Vec::new()
        };
        if self.view.bed_visible {
            extra.extend(print_setup::bed_outline(
                &self.view.bed,
                ui.ctx().pixels_per_point(),
            ));
        }
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
                    &viewport.display_view,
                    wgpu::FilterMode::Linear,
                    id,
                ),
                None => {
                    gpu.texture = Some(egui_renderer.register_native_texture(
                        &state.device,
                        &viewport.display_view,
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
            gpu.renderer.render_with_lines(
                &state.device,
                &state.queue,
                viewport,
                &gpu.scene,
                &self.view.camera,
                &self.style,
                &extra,
            );
            self.dirty = false;
        }
        if let Some(texture) = gpu.texture {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            ui.painter().image(texture, rect, uv, Color32::WHITE);
        }
    }

    /// The transform of the open instance, or the identity when none is open.
    fn open_transform(&self) -> DMat4 {
        self.editor
            .as_ref()
            .zip(self.model.as_ref())
            .and_then(|(editor, document)| {
                document
                    .kernel
                    .as_ref()
                    .map(|kernel| editor.path().transform(kernel).unwrap_or(DMat4::IDENTITY))
            })
            .unwrap_or(DMat4::IDENTITY)
    }

    /// Draw the editor's preview, hint, selection and draw plane over the rendered view.
    fn paint_overlays(&self, ui: &egui::Ui, rect: Rect) {
        let theme = look::theme_of(&ui.ctx().global_style().visuals);
        let view = EditorViewport {
            camera: self.view.camera,
            size_px: (f64::from(rect.width()), f64::from(rect.height())),
        };
        let project = |p: DVec3| {
            view.project(p)
                .map(|q| rect.min + egui::vec2(q.x as f32, q.y as f32))
        };
        let painter = ui.painter();
        let open_transform = self.open_transform();
        let (Some(editor), Some(document)) = (&self.editor, &self.model) else {
            return;
        };
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
        if let Some(model) = &document.kernel {
            for (a, b) in editor.place_preview(model) {
                if let (Some(a), Some(b)) = (project(a), project(b)) {
                    painter.line_segment([a, b], egui::Stroke::new(1.5, preview_color));
                }
            }
            for (a, b) in editor.text_preview(model) {
                if let (Some(a), Some(b)) = (project(a), project(b)) {
                    painter.line_segment([a, b], egui::Stroke::new(1.5, preview_color));
                }
            }
        }
        if let Some(kernel) = &document.kernel {
            let open_body = editor.path().body(kernel).unwrap_or(BodyId::Root);
            let primitives =
                overlay::selection(kernel, open_body, open_transform, editor.selection());
            paint_primitives(painter, &self.view.camera, &primitives, &project, theme);
            if let Some(marks) = self.solid_marks.marks() {
                let primitives = mark_primitives(marks);
                paint_primitives(painter, &self.view.camera, &primitives, &project, theme);
            }
            if let Some(handles) = editor.texture_handles(kernel) {
                let primitives = texture_handles::primitives(&handles);
                let primitives = overlay::transformed(primitives, open_transform);
                paint_primitives(painter, &self.view.camera, &primitives, &project, theme);
            }
            if let Some(face) = editor.pushpull_face()
                && let Some(body) = kernel.body(open_body)
            {
                let primitives = overlay::face_highlight(body.geometry(), face);
                let primitives = overlay::transformed(primitives, open_transform);
                paint_primitives(painter, &self.view.camera, &primitives, &project, theme);
            }
            if let Some(face) = editor.offset_face()
                && let Some(body) = kernel.body(open_body)
            {
                let primitives = overlay::face_highlight(body.geometry(), face);
                let primitives = overlay::transformed(primitives, open_transform);
                paint_primitives(painter, &self.view.camera, &primitives, &project, theme);
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
                    paint_primitives(painter, &self.view.camera, &primitives, &project, theme);
                }
            }
            let primitives = overlay::guides(
                kernel.root().guides(),
                self.view.camera.target,
                self.view.camera.distance * 10.0,
            );
            paint_primitives(painter, &self.view.camera, &primitives, &project, theme);
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
                paint_primitives(painter, &self.view.camera, &primitives, &project, theme);
            }
        }
        if let Some(hint) = editor.hint()
            && let Some(point) = project(hint.point)
        {
            let label = hint_label(&hint, theme);
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
        if editor.eyedropper_armed()
            && let Some(pointer) = ui.ctx().input(|input| input.pointer.latest_pos())
            && rect.contains(pointer)
        {
            let cursor_rect = icons::eyedropper_cursor_rect(pointer);
            let painter = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("eyedropper-cursor"),
            ));
            let color = ui.style().visuals.text_color();
            let brightness = u32::from(color.r()) + u32::from(color.g()) + u32::from(color.b());
            let contrast = if brightness > 384 {
                Color32::BLACK
            } else {
                Color32::WHITE
            };
            let shadow =
                Rect::from_min_size(cursor_rect.min + egui::vec2(1.0, 1.0), cursor_rect.size());
            icons::paint(&painter, shadow, Icon::Eyedropper, contrast);
            icons::paint(&painter, cursor_rect, Icon::Eyedropper, color);
        }
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
                    [drydock_kernel::select::Entity::Instance(_)]
                )
            }),
            hide_rest: self
                .editor
                .as_ref()
                .is_some_and(|editor| editor.hide_rest()),
            axes: self.view.axes,
            overhangs: self.view.overhangs,
            fit_check: self.fit.is_some(),
            can_fit_check: self
                .editor
                .as_ref()
                .and_then(|editor| selected_pair(editor.selection()))
                .is_some(),
            can_offset: self
                .editor
                .as_ref()
                .is_some_and(|editor| !selected_instances(editor.selection()).is_empty()),
            can_text: self
                .model
                .as_ref()
                .is_some_and(|document| document.kernel.is_some()),
            can_export: self
                .model
                .as_ref()
                .and_then(|d| d.kernel.as_ref())
                .is_some_and(mesh_export::can_export),
            shadows: self.dock.is_open(PanelId::Shadows),
            entity_info: self.dock.is_open(PanelId::EntityInfo),
            materials: self.dock.is_open(PanelId::Materials),
            components: self.dock.is_open(PanelId::Components),
            tags: self.dock.is_open(PanelId::Tags),
            texture_position: self.dock.is_open(PanelId::TexturePosition),
            recent: self.recent.paths(),
            tool: self.view.tool,
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
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
                }
                if let Some((note, selection, undo_state)) = &self.repair_note
                    && self
                        .editor
                        .as_ref()
                        .is_some_and(|editor| editor.selection() == selection.as_slice())
                    && self.undo_state() == *undo_state
                {
                    ui.separator();
                    ui.label(note);
                }
                if let Some(error) = &model.kernel_error {
                    ui.separator();
                    ui.colored_label(ERROR_COLOR, error);
                }
            }
            if let Some(message) = self.fit.as_ref().map(FitCheck::message) {
                ui.separator();
                ui.label(message);
            }
            if self.fit.is_some() {
                let current = self.view.clearance_m;
                if let Some(meters) = chrome::clearance_field(ui, &mut self.clearance_text, current)
                {
                    let effect = self.view.set_clearance_mm(meters * 1000.0);
                    self.fit_effect(effect, frame);
                }
            }
            if let Some((message, selection)) = &self.fit_note
                && self
                    .editor
                    .as_ref()
                    .is_some_and(|editor| editor.selection() == selection.as_slice())
            {
                ui.separator();
                ui.label(message);
            }
            if let Some(note) = &self.export_note {
                ui.separator();
                ui.label(note);
            }
            if let Some(report) = self.current_print_report() {
                ui.separator();
                ui.label(&report.message);
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
            // The editor's message is drawn above; don't show the same text twice.
            if let Some(error) = &self.error
                && self.editor.as_ref().and_then(Editor::message) != Some(error.as_str())
            {
                ui.separator();
                ui.colored_label(ERROR_COLOR, error);
            }
            if let Some(handles) = self
                .editor
                .as_ref()
                .zip(
                    self.model
                        .as_ref()
                        .and_then(|document| document.kernel.as_ref()),
                )
                .and_then(|(editor, model)| editor.texture_handles(model))
            {
                ui.separator();
                ui.label(texture_handles::help_text(&handles));
            }
            if self.editor.as_ref().is_some_and(Editor::pasting) {
                ui.separator();
                ui.label("Click to place the paste (Esc cancels)");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(match self.view.camera.projection {
                    Projection::Perspective => "Perspective",
                    Projection::Parallel => "Parallel",
                });
                ui.separator();
                if let Some((editor, kernel)) = self.editor.as_ref().zip(
                    self.model
                        .as_ref()
                        .and_then(|document| document.kernel.as_ref()),
                ) {
                    let measurement = editor.measurement(kernel).map(|(label, value)| {
                        let label = if label == "Distance"
                            && editor.tool() == Tool::Move
                            && editor.move_copy()
                        {
                            "Distance (copy)"
                        } else {
                            label
                        };
                        (label, value)
                    });
                    let typed = editor.typed();
                    if measurement.is_some() || !typed.is_empty() {
                        let measurement = measurement
                            .as_ref()
                            .map(|(label, value)| (*label, value.as_str()));
                        chrome::status_measurement(ui, measurement, typed);
                        ui.separator();
                    }
                }
                let armed = self.editor.as_ref().is_some_and(Editor::eyedropper_armed);
                ui.label(match self.view.tool {
                    _ if armed => "Eyedropper",
                    Tool::Orbit => "Orbit",
                    Tool::Pan => "Pan",
                    Tool::Zoom => "Zoom",
                    Tool::Select => "Select",
                    Tool::Line => "Line",
                    Tool::Rectangle => "Rectangle",
                    Tool::Circle => "Circle",
                    Tool::Arc => "Arc",
                    Tool::Arc3 => "3-Point Arc",
                    Tool::PushPull => "Extrude",
                    Tool::Rotate => "Rotate",
                    Tool::Move => "Move",
                    Tool::Scale => "Scale",
                    Tool::Tape => "Tape Measure",
                    Tool::Erase => "Erase",
                    Tool::Paint => "Paint Bucket",
                    Tool::Offset => "Offset",
                    Tool::FollowMe => "Follow Me",
                });
                if self.view.tool == Tool::Move {
                    ui.separator();
                    ui.label("Ctrl: copy");
                }
                if self.view.tool == Tool::FollowMe
                    && let Some(hint) = self.editor.as_ref().and_then(Editor::follow_hint)
                {
                    ui.separator();
                    ui.label(hint);
                }
            });
        });
    }
}

impl App {
    /// Draw the docked panels and floating toolbars; returns what the user did with them.
    fn draw_toolbars(&mut self, ui: &mut egui::Ui) -> Vec<ToolbarEvent> {
        let layout = self.toolbars.clone();
        let current = self.view.tool;
        let mut events = Vec::new();
        let mut rects: Vec<(usize, Rect)> = Vec::new();
        let mut dragging: Option<(usize, Rect)> = None;
        for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            let indices = layout.docked(edge);
            if indices.is_empty() {
                continue;
            }
            let vertical = matches!(edge, Edge::Left | Edge::Right);
            let panel = toolbars::edge_panel(edge, &ui.style().clone());
            panel.show(ui, |ui| {
                let strips = |ui: &mut egui::Ui| {
                    for &index in &indices {
                        let response = toolbars::toolbar_strip_response(
                            ui,
                            &layout.toolbars()[index],
                            index,
                            current,
                            vertical,
                        );
                        rects.push((index, response.rect));
                        if response.dragging {
                            dragging = Some((index, response.rect));
                        }
                        if let Some(event) = response.event {
                            events.push(event);
                        }
                    }
                };
                if vertical {
                    ui.vertical(strips);
                } else {
                    ui.horizontal(strips);
                }
            });
        }
        for index in layout.floating() {
            let Placement::Floating { x, y } = layout.toolbars()[index].placement else {
                continue;
            };
            egui::Area::new(egui::Id::new(("toolbar-area", index)))
                .fixed_pos(egui::pos2(x, y))
                .show(ui.ctx(), |ui| {
                    egui::Frame::new()
                        .inner_margin(egui::Margin::same(2))
                        .fill(ui.visuals().window_fill)
                        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                        .show(ui, |ui| {
                            let response = toolbars::toolbar_strip_response(
                                ui,
                                &layout.toolbars()[index],
                                index,
                                current,
                                false,
                            );
                            rects.push((index, response.rect));
                            if response.dragging {
                                dragging = Some((index, response.rect));
                            }
                            if let Some(event) = response.event {
                                events.push(event);
                            }
                        });
                });
        }
        self.toolbar_rects = rects;

        if let Some((index, rect)) = dragging
            && let Some(pointer) = ui.ctx().input(|input| input.pointer.latest_pos())
        {
            let size = rect.size();
            let window = ui.ctx().content_rect();
            let outline = match toolbars::drop_target(pointer, window) {
                Some(edge) => {
                    let member_rects = self.edge_toolbar_rects(index, edge);
                    let at = toolbars::insertion_index(edge, &member_rects, pointer);
                    toolbars::drop_outline(edge, &member_rects, at, window, size)
                }
                None => Rect::from_min_size(pointer, size),
            };
            ui.ctx()
                .layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("toolbar-drop-outline"),
                ))
                .rect_stroke(
                    outline,
                    0.0,
                    egui::Stroke::new(1.5, ui.visuals().selection.bg_fill),
                    egui::StrokeKind::Inside,
                );
        }
        events
    }

    /// The drawn rectangles of the toolbars docked along `edge`, in docked order, leaving
    /// out toolbar `index` when it is docked there itself.
    fn edge_toolbar_rects(&self, index: usize, edge: Edge) -> Vec<Rect> {
        let own_edge = match self.toolbars.toolbars().get(index).map(|t| t.placement) {
            Some(Placement::Docked { edge, .. }) => Some(edge),
            _ => None,
        };
        self.toolbars
            .docked(edge)
            .into_iter()
            .filter(|&member| !(own_edge == Some(edge) && member == index))
            .filter_map(|member| {
                self.toolbar_rects
                    .iter()
                    .find(|(drawn, _)| *drawn == member)
                    .map(|(_, rect)| *rect)
            })
            .collect()
    }

    /// Save the toolbar layout if there is a file for it; write errors are ignored.
    fn save_toolbars(&self) {
        if let Some(file) = &self.toolbars_file {
            let _ = self.toolbars.save(file);
        }
    }

    /// Save the panel layout if there is a file for it; write errors are ignored.
    fn save_panels(&self) {
        if let Some(file) = &self.dock_file {
            let _ = self.dock.save(file);
        }
    }

    /// Reveal a panel from the View menu, bringing a floating panel to the front.
    fn reveal_panel(&mut self, id: PanelId, ctx: &egui::Context) {
        self.dock.reveal(id);
        if matches!(self.dock.panel(id).place, dock::Place::Floating { .. }) {
            ctx.move_to_top(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new(("floating-panel", id)),
            ));
        }
        self.save_panels();
    }

    /// Draw the docking columns and floating panels, apply what their headers and contents
    /// asked for, and save the layout when it changed.
    fn draw_panels(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, frame: &eframe::Frame) {
        let layout = self.dock.clone();
        let mut stacks = self.stacks.clone();
        let mut events = Vec::new();
        let mut rects: Vec<(PanelId, Rect)> = Vec::new();
        let mut columns: Vec<(DockSide, Rect)> = Vec::new();
        let mut actions = Vec::new();
        let mut changed = false;
        ctx.data_mut(|data| data.remove::<PanelId>(egui::Id::new("dock-dragging")));

        for side in [DockSide::Left, DockSide::Right] {
            let ids = layout.docked(side);
            if ids.is_empty() {
                continue;
            }
            let stack = match side {
                DockSide::Left => 0,
                DockSide::Right => 1,
            };
            let rows = &mut stacks[stack];
            if rows.len() != ids.len() {
                *rows = ids
                    .iter()
                    .map(|_| StackRow {
                        collapsed: false,
                        weight: 1.0,
                    })
                    .collect();
            }
            for (row, id) in rows.iter_mut().zip(&ids) {
                row.collapsed = layout.panel(*id).collapsed;
            }
            let panel = match side {
                DockSide::Left => egui::Panel::left("panels-left"),
                DockSide::Right => egui::Panel::right("panels-right"),
            };
            let response = panel
                .resizable(true)
                .default_size(layout.width(side))
                .min_size(dock::MIN_WIDTH)
                .max_size(dock::MAX_WIDTH)
                .show(ui, |ui| {
                    let available = ui.available_height();
                    let heights = dock::stack_heights(available, rows);
                    egui::ScrollArea::vertical()
                        .id_salt(("panel-column", side))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let mut drawn: Vec<(PanelId, Rect)> = Vec::new();
                            for (index, &id) in ids.iter().enumerate() {
                                let collapsed = layout.panel(id).collapsed;
                                let height = heights[index].max(dock::HEADER_HEIGHT);
                                let row = ui.allocate_ui_with_layout(
                                    egui::vec2(ui.available_width(), height),
                                    egui::Layout::top_down(egui::Align::Min),
                                    |ui| {
                                        if let Some(event) = panel_header(ui, id, collapsed, true) {
                                            events.push(event);
                                        }
                                        if !collapsed {
                                            let body_height =
                                                (height - dock::HEADER_HEIGHT).max(0.0);
                                            egui::ScrollArea::vertical()
                                                .id_salt(("panel-body", id))
                                                .max_height(body_height)
                                                .auto_shrink([false, false])
                                                .show(ui, |ui| {
                                                    // Rows are sibling uis with one id;
                                                    // give each panel's state its own.
                                                    ui.push_id(("panel", id), |ui| {
                                                        actions.extend(self.panel_body(ui, id));
                                                    });
                                                });
                                        }
                                    },
                                );
                                drawn.push((id, row.response.rect));
                                rects.push((id, row.response.rect));
                            }

                            for index in 0..ids.len().saturating_sub(1) {
                                if rows[index].collapsed || rows[index + 1].collapsed {
                                    continue;
                                }
                                let rect = drawn[index].1;
                                let splitter = Rect::from_min_max(
                                    pos2(rect.left(), rect.bottom() - 2.0),
                                    pos2(rect.right(), rect.bottom() + 2.0),
                                );
                                let response = ui.interact(
                                    splitter,
                                    egui::Id::new(("panel-split", ids[index])),
                                    egui::Sense::drag(),
                                );
                                if response.hovered() || response.dragged() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                                }
                                if response.dragged() {
                                    let delta = ui.input(|input| input.pointer.delta().y);
                                    dock::drag_splitter(rows, &heights, index, delta);
                                }
                            }
                        });
                });
            let rect = response.response.rect;
            columns.push((side, rect));
            let width = rect.width();
            if (width - self.dock.width(side)).abs() > 0.5 {
                self.dock.set_width(side, width);
                changed = true;
            }
        }
        self.stacks = stacks;

        for id in layout.floating() {
            let dock::Place::Floating { x, y, w, h } = layout.panel(id).place else {
                continue;
            };
            let response = egui::Window::new(dock::title(id))
                .id(egui::Id::new(("floating-panel", id)))
                .title_bar(false)
                .collapsible(false)
                .movable(false)
                .resizable(true)
                .current_pos(pos2(x, y))
                .default_size(egui::vec2(w, h))
                .min_size(egui::vec2(dock::MIN_FLOAT_SIZE.0, dock::MIN_FLOAT_SIZE.1))
                .show(ctx, |ui| {
                    if let Some(event) = panel_header(ui, id, false, false) {
                        events.push(event);
                    }
                    actions.extend(self.panel_body(ui, id));
                });
            if let Some(response) = response {
                let rect = response.response.rect;
                let stored = Rect::from_min_size(pos2(x, y), egui::vec2(w, h));
                if (rect.min - stored.min).length() > 0.5
                    || (rect.size() - stored.size()).length() > 0.5
                {
                    self.dock.float(id, rect);
                    changed = true;
                }
            }
        }

        if ctx
            .data(|data| data.get_temp::<PanelId>(egui::Id::new("dock-dragging")))
            .is_some()
            && let Some(pointer) = ctx.input(|input| input.pointer.latest_pos())
        {
            let screen = ctx.content_rect();
            if let Some(side) = self.dock.drop_side(screen, pointer) {
                let rect = columns
                    .iter()
                    .find(|(other, _)| *other == side)
                    .map(|(_, rect)| *rect)
                    .unwrap_or_else(|| match side {
                        DockSide::Left => Rect::from_min_max(
                            screen.min,
                            pos2(screen.left() + dock::EDGE_ZONE, screen.bottom()),
                        ),
                        DockSide::Right => Rect::from_min_max(
                            pos2(screen.right() - dock::EDGE_ZONE, screen.top()),
                            screen.max,
                        ),
                    });
                ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("dock-drop-tint"),
                ))
                .rect_filled(
                    rect,
                    0.0,
                    Color32::from_rgba_unmultiplied(230, 140, 25, 32),
                );
            }
        }

        for action in actions {
            match action {
                PanelAction::Info(action) => self.apply_info_action(action, ctx, frame),
                PanelAction::Material(action) => self.apply_material_action(action, ctx, frame),
                PanelAction::Component(ComponentAction::Import) => self.open_import_dialog(),
                PanelAction::Component(ComponentAction::Place(id)) => {
                    let started =
                        if let (Some(editor), Some(document)) = (&mut self.editor, &self.model) {
                            match document.kernel.as_ref() {
                                Some(model) => editor.place_definition(model, id),
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
                PanelAction::Tag(action) => {
                    let changed = if let (Some(editor), Some(document)) =
                        (&mut self.editor, &mut self.model)
                    {
                        match document.kernel.as_mut() {
                            Some(model) => match action {
                                TagAction::Add => {
                                    let name = default_tag_name(model);
                                    editor.add_tag(model, &name).is_some()
                                }
                                TagAction::Rename(id, name) => editor.rename_tag(model, id, &name),
                                TagAction::SetVisible(id, visible) => {
                                    editor.set_tag_visible(model, id, visible)
                                }
                                TagAction::Delete(id) => editor.remove_tag(model, id),
                                TagAction::Assign(tag) => editor.assign_selection_tag(model, tag),
                            },
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
                PanelAction::Texture(face, side, op) => {
                    let changed = if let (Some(editor), Some(document)) =
                        (&mut self.editor, &mut self.model)
                    {
                        match document.kernel.as_mut() {
                            Some(model) => editor.position_texture(model, face, side, op),
                            None => false,
                        }
                    } else {
                        false
                    };
                    if changed {
                        self.refresh_document(ctx, frame);
                    }
                }
                PanelAction::Handles(action) => {
                    if let (Some(editor), Some(document)) = (&mut self.editor, &self.model) {
                        match action {
                            Some((face, side)) => {
                                if let Some(model) = document.kernel.as_ref() {
                                    editor.start_texture_handles(model, face, side);
                                }
                            }
                            None => editor.end_texture_handles(),
                        }
                    }
                }
                PanelAction::Shadow(event, settings) => {
                    self.apply_shadow_event(event, settings, ctx, frame);
                }
            }
        }

        for event in events {
            match event {
                PanelEvent::ToggleCollapsed(id) => {
                    let collapsed = self.dock.panel(id).collapsed;
                    self.dock.set_collapsed(id, !collapsed);
                }
                PanelEvent::Close(id) => {
                    self.dock.set_open(id, false);
                    if id == PanelId::Shadows {
                        self.shadows_preview = None;
                    }
                }
                PanelEvent::Dropped { id, pos } => {
                    let screen = ctx.content_rect();
                    let middles: Vec<f32> = self
                        .dock
                        .drop_side(screen, pos)
                        .map(|side| {
                            self.dock
                                .docked(side)
                                .into_iter()
                                .filter(|panel| *panel != id)
                                .filter_map(|panel| {
                                    rects
                                        .iter()
                                        .find(|(other, _)| *other == panel)
                                        .map(|(_, rect)| rect.center().y)
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let target = drop_destination(&self.dock, screen, pos, &middles);
                    apply_drop(&mut self.dock, id, target, pos);
                    if id == PanelId::Shadows && !self.dock.is_open(id) {
                        self.shadows_preview = None;
                    }
                }
            }
            changed = true;
        }

        if changed {
            self.save_panels();
        }
    }

    /// The contents under a panel's header.
    fn panel_body(&mut self, ui: &mut egui::Ui, id: PanelId) -> Vec<PanelAction> {
        let mut actions = Vec::new();
        match id {
            PanelId::EntityInfo => {
                let (Some(editor), Some(document)) = (&self.editor, &self.model) else {
                    ui.label("No model");
                    return actions;
                };
                let Some(model) = document.kernel.as_ref() else {
                    ui.label("No model");
                    return actions;
                };
                let body = editor.path().body(model).unwrap_or(BodyId::Root);
                if let Some(action) =
                    entity_info_panel(ui, &mut self.entity_info, model, body, editor.selection())
                {
                    actions.push(PanelAction::Info(action));
                }
            }
            PanelId::Materials => {
                let (Some(editor), Some(document)) = (&self.editor, &self.model) else {
                    ui.label("No model");
                    return actions;
                };
                let Some(model) = document.kernel.as_ref() else {
                    ui.label("No model");
                    return actions;
                };
                let current = editor.material();
                self.materials.eyedropper_armed = editor.eyedropper_armed();
                if let Some(action) = materials_panel(ui, &mut self.materials, model, current) {
                    actions.push(PanelAction::Material(action));
                }
            }
            PanelId::Components => {
                let (Some(_editor), Some(document)) = (&self.editor, &self.model) else {
                    ui.label("No model");
                    return actions;
                };
                let Some(model) = document.kernel.as_ref() else {
                    ui.label("No model");
                    return actions;
                };
                if let Some(action) = components_panel(ui, &mut self.components, model) {
                    actions.push(PanelAction::Component(action));
                }
            }
            PanelId::Tags => {
                let (Some(editor), Some(document)) = (&self.editor, &self.model) else {
                    ui.label("No model");
                    return actions;
                };
                let Some(model) = document.kernel.as_ref() else {
                    ui.label("No model");
                    return actions;
                };
                self.tags.selected = editor
                    .selection()
                    .iter()
                    .filter(|entity| matches!(entity, Entity::Instance(_)))
                    .count();
                if let Some(action) = tags_panel(ui, &mut self.tags, model) {
                    actions.push(PanelAction::Tag(action));
                }
            }
            PanelId::Shadows => {
                let Some(document) = &self.model else {
                    ui.label("No model");
                    return actions;
                };
                let Some(model) = document.kernel.as_ref() else {
                    ui.label("No model");
                    return actions;
                };
                let mut settings = self
                    .shadows_preview
                    .unwrap_or_else(|| model.shadow_settings());
                let event = shadows_panel::shadows_panel(ui, &mut settings);
                if event != ShadowsEvent::None {
                    actions.push(PanelAction::Shadow(event, settings));
                }
            }
            PanelId::TexturePosition => {
                let (Some(editor), Some(document)) = (&self.editor, &self.model) else {
                    ui.label("No model");
                    return actions;
                };
                let Some(model) = document.kernel.as_ref() else {
                    ui.label("No model");
                    return actions;
                };
                let target = self.texture_target(model);
                let texture_on = editor.texture_handles(model).is_some();
                let (op, toggled) = texture_position_section(
                    ui,
                    &mut self.texture,
                    target.is_some(),
                    texture_on,
                    model.length_format().input_unit(),
                );
                if toggled {
                    actions.push(PanelAction::Handles(if texture_on { None } else { target }));
                }
                if let Some(op) = op
                    && let Some((face, side)) = target
                {
                    actions.push(PanelAction::Texture(face, side, op));
                }
            }
        }
        actions
    }

    /// Apply a rename or repair from the Entity Info panel.
    fn apply_info_action(
        &mut self,
        action: InfoAction,
        ctx: &egui::Context,
        frame: &eframe::Frame,
    ) {
        let mut changed = false;
        let mut note = None;
        let mut failed = false;
        if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model)
            && let Some(model) = document.kernel.as_mut()
        {
            match action {
                InfoAction::RenameDefinition(id, name) => {
                    changed = editor.rename_definition(model, id, &name);
                }
                InfoAction::RenameInstance(id, name) => {
                    changed = editor.rename_instance(model, id, &name);
                }
                InfoAction::SetTag(tag) => {
                    changed = editor.assign_selection_tag(model, tag);
                }
                InfoAction::Repair(id) => match editor.repair_instance(model, id) {
                    Ok(report) => {
                        note = Some((
                            repair_message(&report),
                            editor.selection().to_vec(),
                            editor.undo_stack().state(),
                        ));
                        changed = report.changed();
                    }
                    Err(_) => failed = true,
                },
            }
        }
        if failed {
            self.repair_note = None;
        }
        if let Some(note) = note {
            self.repair_note = Some(note);
        }
        if changed {
            self.refresh_document(ctx, frame);
        }
    }

    /// Preview or commit a change from the Shadows panel.
    fn apply_shadow_event(
        &mut self,
        event: ShadowsEvent,
        settings: ShadowSettings,
        ctx: &egui::Context,
        frame: &eframe::Frame,
    ) {
        match event {
            ShadowsEvent::Preview => self.shadows_preview = Some(settings),
            ShadowsEvent::Commit => {
                self.shadows_preview = None;
                let changed =
                    if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model) {
                        match document.kernel.as_mut() {
                            Some(model) if settings != model.shadow_settings() => {
                                editor.set_shadow_settings(model, settings)
                            }
                            Some(_) => false,
                            None => false,
                        }
                    } else {
                        false
                    };
                if changed {
                    self.refresh_document(ctx, frame);
                }
            }
            ShadowsEvent::None => {}
        }
    }
}

/// What a panel's contents asked the app to do.
enum PanelAction {
    Info(InfoAction),
    Material(MaterialAction),
    Component(ComponentAction),
    Texture(FaceId, Side, TextureOp),
    Handles(Option<(FaceId, Side)>),
    Shadow(ShadowsEvent, ShadowSettings),
    Tag(TagAction),
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let theme = look::theme_of(&ctx.global_style().visuals);
        if look::apply_viewport_theme(&mut self.style, theme) {
            self.dirty = true;
        }
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
        self.sync_print_report();

        let mut menu_action = None;
        let mut toolbars_changed = false;
        let mut customize_toolbars = false;
        let mut reset_toolbars = false;
        {
            let mut toolbars = std::mem::take(&mut self.toolbars);
            egui::Panel::top("menu").show(ui, |ui| {
                let outcome = chrome::menu_bar_with_toolbars(ui, &self.menu_state(), &mut toolbars);
                menu_action = outcome.action;
                toolbars_changed = outcome.toolbars_changed;
                customize_toolbars = outcome.customize_toolbars;
                reset_toolbars = outcome.reset_toolbars;
            });
            self.toolbars = toolbars;
        }
        if toolbars_changed {
            self.save_toolbars();
        }
        if customize_toolbars {
            self.customize_open = true;
        }
        if reset_toolbars {
            self.app_confirm = Some(AppConfirm::ResetToolbars);
        }
        if let Some(action) = menu_action {
            self.apply(action, &ctx, frame);
        }
        let toolbar_events = self.draw_toolbars(ui);
        for event in toolbar_events {
            match event {
                ToolbarEvent::Select(tool) => {
                    self.apply(input::Action::SelectTool(tool), &ctx, frame);
                }
                ToolbarEvent::Dropped { index, pos } => {
                    match toolbars::drop_target(pos, ctx.content_rect()) {
                        Some(edge) => {
                            let rects = self.edge_toolbar_rects(index, edge);
                            let position = toolbars::insertion_index(edge, &rects, pos);
                            self.toolbars.dock(index, edge, position);
                        }
                        None => self.toolbars.float(index, pos.x, pos.y),
                    }
                    self.save_toolbars();
                }
            }
        }

        self.draw_panels(ui, &ctx, frame);

        let dialog_open = self.dialog.is_some()
            || self.units_prompt.is_some()
            || self.export_dialog.is_some()
            || self.text_dialog.is_some()
            || self.save_dialog.is_some()
            || self.confirm.is_some()
            || self.app_confirm.is_some()
            || self.name_prompt.is_some()
            || self.materials.confirming();
        if !dialog_open {
            self.global_input(&ctx, frame);
            self.sync_editor_path(&ctx, frame);
            if self.sync_solid_marks() {
                self.show_current_scene(frame);
            }
        }
        let mut sun_changed = false;
        if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model)
            && let Some(model) = document.kernel.as_mut()
            && self.sun_windows.show(&ctx, model, editor)
        {
            sun_changed = true;
        }
        if sun_changed {
            self.refresh_document(&ctx, frame);
        }
        let sun = if let Some(model) = self
            .model
            .as_ref()
            .and_then(|document| document.kernel.as_ref())
        {
            match self.shadows_preview {
                Some(settings) => shading::sun_light_for(&model.geolocation(), &settings),
                None => self.sun_windows.sun_light(model),
            }
        } else {
            None
        };
        if shading::update_style(&mut self.style, sun) {
            self.dirty = true;
        }
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui, frame));
        let mut free_area = egui::Rect::NOTHING;
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let (rect, response) =
                ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
            free_area = rect;
            self.viewport_pt = (f64::from(rect.width()), f64::from(rect.height()));
            if !dialog_open {
                self.viewport_input(&ctx, frame, ui, rect, &response);
                self.sync_editor_path(&ctx, frame);
                if self.sync_solid_marks() {
                    self.show_current_scene(frame);
                }
            }
            self.paint_viewport(ui, frame, rect);
            self.paint_overlays(ui, rect);
        });
        self.print_setup_window(&ctx, frame, free_area);
        chrome::about_window(&ctx, &mut self.about_open);
        if self.customize_open {
            let mut open = true;
            let mut changed = false;
            egui::Window::new("Customize Toolbars")
                .id(egui::Id::new("customize_toolbars"))
                .open(&mut open)
                .show(&ctx, |ui| {
                    changed =
                        toolbars::customize_panel(ui, &mut self.toolbars, &mut self.customize);
                });
            self.customize_open = open;
            if changed {
                self.save_toolbars();
            }
        }
        if let Some(confirm) = self.app_confirm {
            let (title, message, action) = match confirm {
                AppConfirm::ResetToolbars => (
                    "Reset Toolbars",
                    "The toolbars go back to how they were installed and your own toolbars are deleted.",
                    "Reset",
                ),
                AppConfirm::ResetPanels => (
                    "Reset Panels",
                    "Every panel goes back to its default place and size.",
                    "Reset",
                ),
            };
            if let Some(ok) =
                dialog::confirm_dialog(&ctx, egui::Id::new("app-confirm"), title, message, action)
            {
                self.app_confirm = None;
                if ok {
                    match confirm {
                        AppConfirm::ResetToolbars => {
                            self.toolbars.reset();
                            self.save_toolbars();
                        }
                        AppConfirm::ResetPanels => {
                            self.dock.reset();
                            self.shadows_preview = None;
                            self.save_panels();
                        }
                    }
                }
            }
        }
        let mut picked_unit = None;
        let mut geolocation = false;
        if let Some(model) = &self.model {
            let kernel = model.kernel.is_some();
            let format = model
                .kernel
                .as_ref()
                .map(Model::length_format)
                .unwrap_or_default();
            let mut lines = chrome::model_info_lines(&model.stats, kernel, format);
            if let Some(state) = frame.wgpu_render_state() {
                let info = state.adapter.get_info();
                lines.push(chrome::adapter_line(
                    &info.name,
                    backend_label(info.backend),
                    device_type_label(info.device_type),
                ));
            }
            chrome::model_info_window_with(&ctx, &mut self.model_info_open, &lines, |ui| {
                if kernel {
                    ui.separator();
                    picked_unit = units_ui::units_section(ui, format);
                    ui.separator();
                }
                if ui
                    .add(
                        egui::Button::new("Geolocation...")
                            .min_size(drydock::dialog::BUTTON_MIN_SIZE),
                    )
                    .clicked()
                {
                    geolocation = true;
                }
            });
        } else {
            self.model_info_open = false;
            self.sun_windows.close();
        }
        if let Some(format) = picked_unit {
            let mut changed = false;
            if let (Some(editor), Some(document)) = (&mut self.editor, &mut self.model)
                && let Some(model) = document.kernel.as_mut()
            {
                changed = editor.set_length_format(model, format);
            }
            if changed {
                self.refresh_document(&ctx, frame);
            }
        }
        if geolocation {
            self.sun_windows.geo_open = true;
        }
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
                    let mode = dialog.mode;
                    self.dialog = None;
                    match mode {
                        OpenMode::Component => self.import_component_file(&ctx, frame, &path),
                        OpenMode::Models => {
                            if mesh_import::needs_units(&path) {
                                self.units_prompt = Some(UnitsPrompt {
                                    path,
                                    purpose: UnitsPurpose::Import,
                                    unit: self.mesh_unit,
                                });
                            } else {
                                self.import_mesh_file(&ctx, frame, &path, MeshUnit::default());
                            }
                        }
                        OpenMode::Open => {
                            self.request(PendingAction::Open(Some(path)), &ctx, frame)
                        }
                    }
                }
                DialogOutcome::Cancel => self.dialog = None,
                DialogOutcome::Pending => {}
            }
        }
        if let Some(prompt) = &mut self.units_prompt {
            let mut outcome = None;
            let file = file_name(&prompt.path);
            let response = egui::Modal::new(egui::Id::new("mesh-units")).show(&ctx, |ui| {
                ui.set_width(320.0);
                ui.heading("Mesh units");
                outcome = mesh_import::units_choice(ui, &file, &mut prompt.unit);
            });
            if outcome.is_none() && response.should_close() {
                outcome = Some(UnitsOutcome::Cancel);
            }
            match outcome {
                Some(UnitsOutcome::Ok) => {
                    let prompt = self.units_prompt.take().unwrap();
                    self.mesh_unit = prompt.unit;
                    match prompt.purpose {
                        UnitsPurpose::Open => {
                            self.start_loading_mesh(&ctx, prompt.path, prompt.unit)
                        }
                        UnitsPurpose::Import => {
                            self.import_mesh_file(&ctx, frame, &prompt.path, prompt.unit)
                        }
                    }
                }
                Some(UnitsOutcome::Cancel) => self.units_prompt = None,
                None => {}
            }
        }
        let has_selection = self.editor.as_ref().is_some_and(|editor| {
            editor
                .selection()
                .iter()
                .any(|entity| matches!(entity, Entity::Instance(_)))
        });
        if let Some(prompt) = &mut self.text_dialog {
            let mut outcome = None;
            let response = egui::Modal::new(egui::Id::new("text-dialog")).show(&ctx, |ui| {
                ui.set_width(360.0);
                ui.heading("3D Text");
                outcome = text_tool::text_dialog(ui, &mut prompt.dialog);
            });
            if outcome.is_none() && response.should_close() {
                outcome = Some(TextOutcome::Cancel);
            }
            match outcome {
                Some(TextOutcome::Ok) => {
                    let prompt = self.text_dialog.take().unwrap();
                    if let Some(options) = prompt.dialog.options() {
                        self.text_options = Some(options.clone());
                        if let (Some(editor), Some(document)) = (&mut self.editor, &self.model)
                            && let Some(model) = document.kernel.as_ref()
                        {
                            editor.begin_text(model, options);
                        }
                        self.dirty = true;
                    }
                }
                Some(TextOutcome::Cancel) => self.text_dialog = None,
                None => {}
            }
        }
        if let Some(prompt) = &mut self.export_dialog {
            let mut outcome = None;
            let response = egui::Modal::new(egui::Id::new("export-dialog")).show(&ctx, |ui| {
                ui.set_width(360.0);
                ui.heading(format!("Export {}", prompt.dialog.options.format.label()));
                outcome = mesh_export::export_dialog(ui, &mut prompt.dialog, has_selection);
            });
            if outcome.is_none() && response.should_close() {
                outcome = Some(ExportOutcome::Cancel);
            }
            match outcome {
                Some(ExportOutcome::Ok) => {
                    let prompt = self.export_dialog.take().unwrap();
                    let options = prompt.dialog.options;
                    if let Some(remembered) = self
                        .export_options
                        .iter_mut()
                        .find(|remembered| remembered.format == options.format)
                    {
                        *remembered = options;
                    } else {
                        self.export_options.push(options);
                    }
                    self.open_export_save_dialog(options);
                }
                Some(ExportOutcome::Cancel) => self.export_dialog = None,
                None => {}
            }
        }
        if let Some(dialog) = &mut self.save_dialog {
            match dialog.show(&ctx) {
                SaveOutcome::Save(path) => {
                    let mode = dialog.mode;
                    self.save_dialog = None;
                    match mode {
                        SaveMode::Model => {
                            let state = self.undo_state();
                            let result =
                                self.model.as_mut().map(|document| document.save_as(&path));
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
                        SaveMode::Export(options) | SaveMode::ExportFolder(options) => {
                            self.export_to(&path, options);
                        }
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

/// The export dialog waiting for its options.
struct ExportPrompt {
    dialog: ExportDialog,
}

/// The 3D Text dialog waiting for its options.
struct TextPrompt {
    dialog: TextDialog,
}

/// The units choice waiting for an STL file.
struct UnitsPrompt {
    path: PathBuf,
    purpose: UnitsPurpose,
    unit: MeshUnit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnitsPurpose {
    Open,
    Import,
}

/// What the user asked for that may need the document saved first.
#[derive(Clone, Debug)]
enum PendingAction {
    New,
    Open(Option<PathBuf>),
    Close,
}

/// A destructive app-level action waiting for confirmation (`docs/specs/m6-confirm.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppConfirm {
    ResetToolbars,
    ResetPanels,
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
            if look::primary_button(ui, "Save").clicked() {
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
fn overlay_style(style: overlay::Style, theme: look::Theme) -> (Color32, f32) {
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
        overlay::Style::Problem => {
            let ([r, g, b, a], width) = overlay::problem_stroke();
            (Color32::from_rgba_unmultiplied(r, g, b, a), width)
        }
        overlay::Style::Axis { axis, .. } => {
            let [r, g, b] = look::axis_color(theme, axis);
            (Color32::from_rgb(r, g, b), 1.5)
        }
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
    camera: &drydock_view::Camera,
    primitives: &[Primitive],
    project: &dyn Fn(DVec3) -> Option<Pos2>,
    theme: look::Theme,
) {
    for primitive in primitives {
        match primitive {
            Primitive::Line { a, b, style } => {
                let (color, width) = overlay_style(*style, theme);
                let Some((a, b)) = overlay::clip_in_front(camera, *a, *b) else {
                    continue;
                };
                if let (Some(a), Some(b)) = (project(a), project(b)) {
                    let stroke = egui::Stroke::new(width, color);
                    if matches!(
                        style,
                        overlay::Style::Axis {
                            positive: false,
                            ..
                        }
                    ) {
                        painter.extend(egui::Shape::dashed_line(&[a, b], stroke, 6.0, 4.0));
                    } else {
                        painter.line_segment([a, b], stroke);
                    }
                }
            }
            Primitive::Polygon { points, style } => {
                let (color, width) = overlay_style(*style, theme);
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
                let (color, _) = overlay_style(*style, theme);
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

/// Short text for the system clipboard after a Copy or Cut, so the next Ctrl+V arrives.
fn clipboard_text(count: usize, verb: &str) -> String {
    if count == 1 {
        format!("Drydock: 1 item {verb}")
    } else {
        format!("Drydock: {count} items {verb}")
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

/// A minimal open-file dialog in plain egui: browse folders, pick a model file, or type
/// a path.
struct OpenDialog {
    dir: PathBuf,
    /// Subfolders and model files of `dir`, folders first, each sorted by name.
    entries: Vec<(String, PathBuf, bool)>,
    path_text: String,
    error: Option<String>,
    mode: OpenMode,
}

impl OpenDialog {
    fn new(dir: PathBuf) -> OpenDialog {
        OpenDialog::with_mode(dir, OpenMode::Open)
    }

    fn import(dir: PathBuf) -> OpenDialog {
        OpenDialog::with_mode(dir, OpenMode::Component)
    }

    fn models(dir: PathBuf) -> OpenDialog {
        OpenDialog::with_mode(dir, OpenMode::Models)
    }

    fn with_mode(dir: PathBuf, mode: OpenMode) -> OpenDialog {
        let mut dialog = OpenDialog {
            dir: PathBuf::new(),
            entries: Vec::new(),
            path_text: String::new(),
            error: None,
            mode,
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
                    let is_model = is_openable_file(&path);
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
            ui.heading(match self.mode {
                OpenMode::Open => "Open model",
                OpenMode::Component => "Import component",
                OpenMode::Models => "Import into model",
            });
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
                    .hint_text("Path to a .dae, .modeler, .stl or .3mf file, or a folder"),
            );
            let submitted = typed.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if let Some(error) = &self.error {
                ui.colored_label(ERROR_COLOR, error);
            }
            let buttons = open_buttons::open_buttons(ui, self.mode);
            if buttons.chosen || submitted {
                let path = PathBuf::from(self.path_text.trim());
                if path.is_dir() {
                    enter = Some(path);
                } else if path.is_file() {
                    outcome = DialogOutcome::Open(path);
                } else {
                    self.error = Some(format!("{}: no such file", path.display()));
                }
            }
            if buttons.cancelled {
                outcome = DialogOutcome::Cancel;
            }
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

/// What a save dialog is choosing a name for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum SaveMode {
    /// The open document (a `.modeler` file).
    Model,
    /// An export to one file.
    Export(ExportOptions),
    /// An export to a folder, one file per object.
    ExportFolder(ExportOptions),
}

/// A minimal save-file dialog in plain egui: browse folders, list matching files,
/// and choose a file or folder name.
struct SaveDialog {
    dir: PathBuf,
    /// Subfolders and `.modeler` files of `dir`, folders first, each sorted by name.
    entries: Vec<(String, PathBuf, bool)>,
    name_text: String,
    error: Option<String>,
    /// A target that already exists and awaits confirmation.
    replacing: Option<PathBuf>,
    /// What the name is for.
    mode: SaveMode,
}

impl SaveDialog {
    fn new(dir: PathBuf, name: String) -> SaveDialog {
        SaveDialog::with_mode(dir, name, SaveMode::Model)
    }

    fn with_mode(dir: PathBuf, name: String, mode: SaveMode) -> SaveDialog {
        let mut dialog = SaveDialog {
            dir: PathBuf::new(),
            entries: Vec::new(),
            name_text: name,
            error: None,
            replacing: None,
            mode,
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
                    let extension = match self.mode {
                        SaveMode::Model => Some(NATIVE_EXTENSION),
                        SaveMode::Export(options) => Some(options.format.extension()),
                        SaveMode::ExportFolder(_) => None,
                    };
                    let is_model = extension.is_some_and(|extension| {
                        path.extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
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
        self.dir = dir;
    }

    fn target(&self) -> Option<PathBuf> {
        let name = self.name_text.trim();
        if name.is_empty() {
            return None;
        }
        let path = self.dir.join(name);
        Some(match self.mode {
            SaveMode::Model => native_path(&path),
            SaveMode::Export(_) => path,
            SaveMode::ExportFolder(_) => path.with_extension(""),
        })
    }

    fn show(&mut self, ctx: &egui::Context) -> SaveOutcome {
        let mut outcome = SaveOutcome::Pending;
        let mut enter = None;
        // Cancel on the replace question goes back to the file name.
        let mut back = false;
        let response = egui::Modal::new(egui::Id::new("save-dialog")).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.heading(match self.mode {
                SaveMode::Model => "Save model".to_string(),
                SaveMode::Export(options) => {
                    format!("Export {}", options.format.label())
                }
                SaveMode::ExportFolder(options) => {
                    format!("Export {} to folder", options.format.label())
                }
            });
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
                    if look::primary_button(ui, "Replace").clicked() {
                        outcome = SaveOutcome::Save(target.clone());
                    }
                    if ui.button("Cancel").clicked() {
                        back = true;
                    }
                });
            } else {
                let hint = match self.mode {
                    SaveMode::ExportFolder(_) => "Folder name",
                    SaveMode::Model | SaveMode::Export(_) => "File name",
                };
                let name = ui.add(
                    egui::TextEdit::singleline(&mut self.name_text)
                        .desired_width(f32::INFINITY)
                        .hint_text(hint),
                );
                let submitted = name.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if let Some(error) = &self.error {
                    ui.colored_label(ERROR_COLOR, error);
                }
                let choose = match self.mode {
                    SaveMode::Model => "Save",
                    SaveMode::Export(_) | SaveMode::ExportFolder(_) => "Export",
                };
                ui.horizontal(|ui| {
                    if (look::primary_button(ui, choose).clicked() || submitted)
                        && let Some(target) = self.target()
                    {
                        if matches!(self.mode, SaveMode::ExportFolder(_)) && target.is_dir() {
                            outcome = SaveOutcome::Save(target);
                        } else if target.exists() {
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
