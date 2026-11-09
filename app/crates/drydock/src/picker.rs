// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Open, Import, Add Models, Save, Save As and Export through the system's own file picker
//! (`docs/specs/m11-native-file-dialogs.md`). The app describes what it wants as a
//! [`PickRequest`]; a [`Picker`] either shows the system picker on its own thread and hands the
//! answer back through a channel ([`Picker::poll`]), or tells the app to open its own dialog
//! (`crate::file_dialogs`) because no system picker is available. A [`Backend::Stub`] answers
//! with given paths so tests can check what the app asked for.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

use eframe::egui;

use crate::document::{NATIVE_EXTENSION, OPEN_EXTENSIONS, native_path};
use crate::file_dialogs::SaveMode;
use crate::open_buttons::OpenMode;

/// What kind of answer the picker gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickKind {
    /// One existing file.
    OpenFile,
    /// A file name to write (the system picker asks before replacing).
    SaveFile,
    /// An existing folder (the picker offers to create one).
    Folder,
}

/// What the picked path is for; it comes back with the answer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PickPurpose {
    Open(OpenMode),
    Save(SaveMode),
}

/// One file-type filter: a label and extensions without the dot, lower case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickFilter {
    pub label: String,
    pub extensions: Vec<String>,
}

/// Everything the app tells a picker.
#[derive(Clone, Debug, PartialEq)]
pub struct PickRequest {
    pub purpose: PickPurpose,
    pub kind: PickKind,
    /// The window title (the heading our own dialog shows for the same mode).
    pub title: String,
    /// Empty for a folder.
    pub filters: Vec<PickFilter>,
    /// The folder the picker starts in.
    pub dir: PathBuf,
    /// The suggested file name; empty for open and folder requests.
    pub name: String,
}

impl PickRequest {
    /// A request for `mode` starting in `dir`.
    /// - Titles: `Open` "Open model", `Component` "Import component", `Models` "Import into model".
    /// - Kind `OpenFile`; one filter labeled "Models" with `document::OPEN_EXTENSIONS` in
    ///   that order; empty name.
    pub fn open(mode: OpenMode, dir: PathBuf) -> PickRequest {
        let title = match mode {
            OpenMode::Open => "Open model",
            OpenMode::Component => "Import component",
            OpenMode::Models => "Import into model",
        };
        PickRequest {
            purpose: PickPurpose::Open(mode),
            kind: PickKind::OpenFile,
            title: title.to_owned(),
            filters: vec![PickFilter {
                label: "Models".to_owned(),
                extensions: OPEN_EXTENSIONS
                    .iter()
                    .map(|extension| (*extension).to_owned())
                    .collect(),
            }],
            dir,
            name: String::new(),
        }
    }

    /// A request for `mode` starting in `dir`, suggesting `name`.
    /// - `Model`: title "Save model", kind `SaveFile`, one filter labeled "Drydock model" with
    ///   `["drydock"]`.
    /// - `Export(options)`: title `"Export {format label}"`, kind `SaveFile`, one filter
    ///   labeled with the format label (`"STL"`, `"3MF"`) with its extension.
    /// - `ExportFolder(options)`: title `"Export {format label} to folder"`, kind `Folder`, no
    ///   filters, empty name (a folder picker has no name box).
    pub fn save(mode: SaveMode, dir: PathBuf, name: String) -> PickRequest {
        match mode {
            SaveMode::Model => PickRequest {
                purpose: PickPurpose::Save(mode),
                kind: PickKind::SaveFile,
                title: "Save model".to_owned(),
                filters: vec![PickFilter {
                    label: "Drydock model".to_owned(),
                    extensions: vec![NATIVE_EXTENSION.to_owned()],
                }],
                dir,
                name,
            },
            SaveMode::Export(options) => PickRequest {
                purpose: PickPurpose::Save(mode),
                kind: PickKind::SaveFile,
                title: format!("Export {}", options.format.label()),
                filters: vec![PickFilter {
                    label: options.format.label().to_owned(),
                    extensions: vec![options.format.extension().to_owned()],
                }],
                dir,
                name,
            },
            SaveMode::ExportFolder(options) => PickRequest {
                purpose: PickPurpose::Save(mode),
                kind: PickKind::Folder,
                title: format!("Export {} to folder", options.format.label()),
                filters: Vec::new(),
                dir,
                name: String::new(),
            },
        }
    }
}

/// The path to hand to the open/save/export code for a path the system picker returned.
/// - Open and folder: unchanged.
/// - `Save(Model)`: `document::native_path` (adds `.drydock` unless it already has a native
///   extension).
/// - `Save(Export(options))`: unchanged when it has an extension, else the format's extension
///   added.
pub fn finish_path(purpose: PickPurpose, path: PathBuf) -> PathBuf {
    match purpose {
        PickPurpose::Open(_) | PickPurpose::Save(SaveMode::ExportFolder(_)) => path,
        PickPurpose::Save(SaveMode::Model) => native_path(&path),
        PickPurpose::Save(SaveMode::Export(options)) => {
            if path.extension().is_some() {
                path
            } else {
                path.with_extension(options.format.extension())
            }
        }
    }
}

/// A test double for the system picker: queued answers out, requests recorded. Clones share
/// the same state, so a test keeps one clone and gives the other to the [`Picker`].
#[derive(Clone, Debug, Default)]
pub struct StubPicker {
    state: Arc<Mutex<StubState>>,
}

#[derive(Debug, Default)]
struct StubState {
    answers: Vec<Option<PathBuf>>,
    requests: Vec<PickRequest>,
}

impl StubPicker {
    pub fn new() -> StubPicker {
        StubPicker::default()
    }

    /// Queue the answer for the next request: `Some(path)` picks it, `None` cancels. With
    /// nothing queued a request is answered `None`. Answers are used in the order queued.
    pub fn answer(&self, path: Option<PathBuf>) {
        self.state.lock().unwrap().answers.push(path);
    }

    /// Every request the picker has been given, oldest first.
    pub fn requests(&self) -> Vec<PickRequest> {
        self.state.lock().unwrap().requests.clone()
    }
}

/// Where requests go.
#[derive(Clone, Debug)]
pub enum Backend {
    /// The system's picker (portal on Linux, the common item dialog on Windows).
    System,
    /// The app's own dialogs: [`Picker::start`] returns [`Started::Builtin`].
    Builtin,
    /// Test double, answers through the same channel as `System`.
    Stub(StubPicker),
}

/// The backend to use. `setting` is the value of the environment variable
/// `DRYDOCK_FILE_PICKER`: `"builtin"` Builtin, `"system"` System (even when `available` is
/// false), anything else or `None` System when `available`, else Builtin.
pub fn choose_backend(setting: Option<&str>, available: bool) -> Backend {
    match setting {
        Some("builtin") => Backend::Builtin,
        Some("system") => Backend::System,
        _ if available => Backend::System,
        _ => Backend::Builtin,
    }
}

/// True when the text `gdbus introspect --session --dest org.freedesktop.portal.Desktop
/// --object-path /org/freedesktop/portal/desktop` prints lists the interface
/// `org.freedesktop.portal.FileChooser`.
pub fn lists_file_chooser(introspection: &str) -> bool {
    const INTERFACE: &str = "org.freedesktop.portal.FileChooser";
    let mut rest = introspection;
    while let Some(index) = rest.find(INTERFACE) {
        let after = &rest[index + INTERFACE.len()..];
        if !after
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            return true;
        }
        rest = after;
    }
    false
}

/// Whether the system picker can be used here. Windows: always true. Linux: runs that
/// `gdbus introspect` (about 2 seconds at most, like `portal::read_scheme`) and checks it with
/// [`lists_file_chooser`]; false when `gdbus` is missing or fails. Other systems: false.
pub fn system_picker_available() -> bool {
    #[cfg(target_os = "windows")]
    {
        true
    }
    #[cfg(target_os = "linux")]
    {
        system_picker_available_linux()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        false
    }
}

#[cfg(target_os = "linux")]
fn system_picker_available_linux() -> bool {
    use std::time::Duration;

    let args = [
        "introspect",
        "--session",
        "--dest",
        "org.freedesktop.portal.Desktop",
        "--object-path",
        "/org/freedesktop/portal/desktop",
    ];
    gdbus_output(&args, Duration::from_secs(2)).is_some_and(|text| lists_file_chooser(&text))
}

#[cfg(target_os = "linux")]
fn gdbus_output(args: &[&str], timeout: std::time::Duration) -> Option<String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::Instant;

    let mut child = Command::new("gdbus")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut text = String::new();
                child.stdout.take()?.read_to_string(&mut text).ok()?;
                return Some(text);
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

/// What [`Picker::start`] did with a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Started {
    /// The request is with the picker; the answer comes from [`Picker::poll`].
    Native,
    /// No picker: the caller opens its own dialog for the request.
    Builtin,
}

/// An answer: what it was for and the path, `None` when the user cancelled (or the picker
/// could not be shown). Cancel means nothing happens.
#[derive(Clone, Debug, PartialEq)]
pub struct Picked {
    pub purpose: PickPurpose,
    /// Already passed through [`finish_path`].
    pub path: Option<PathBuf>,
}

/// Runs requests on a backend, one at a time.
pub struct Picker {
    backend: Backend,
    pending: Option<(PickPurpose, Receiver<Option<PathBuf>>)>,
    /// A request waiting for [`Picker::start_queued`].
    queued: Option<PickRequest>,
}

/// Frames in a row with no modal prompt window before a queued picker may start. A prompt's
/// OS window is dropped (and its owner enabled again) at the end of the frame that stopped
/// drawing it or a little later, so the picker waits this many quiet frames.
pub const QUIET_FRAMES: u8 = 2;

/// Counts frames without a modal prompt window (m11-picker-after-prompt).
#[derive(Clone, Copy, Debug, Default)]
pub struct ModalGate {
    quiet: u8,
}

impl ModalGate {
    pub fn new() -> ModalGate {
        ModalGate::default()
    }

    /// Call once per frame with whether any of our modal prompts is open or was drawn this
    /// frame. `true` resets the count and returns `false`; `false` adds one and returns
    /// whether [`QUIET_FRAMES`] quiet frames in a row have now passed (stays `true` while
    /// quiet).
    pub fn frame(&mut self, modal_open: bool) -> bool {
        if modal_open {
            self.quiet = 0;
            false
        } else {
            self.quiet = self.quiet.saturating_add(1);
            self.quiet >= QUIET_FRAMES
        }
    }
}

impl Picker {
    pub fn new(backend: Backend) -> Picker {
        Picker {
            backend,
            pending: None,
            queued: None,
        }
    }

    /// Ask for `request` to be started by a later [`Picker::start_queued`]. A request queued
    /// earlier and not yet started is replaced. Nothing reaches the backend yet.
    pub fn queue(&mut self, request: PickRequest) {
        self.queued = Some(request);
    }

    /// True while a request is queued and not yet started.
    pub fn has_queued(&self) -> bool {
        self.queued.is_some()
    }

    /// When a request is queued and `may_start` (a [`ModalGate::frame`] result): take it and
    /// [`Picker::start`] it, returning what start answered and the request, so the caller can
    /// open its own dialog on `Started::Builtin`. Otherwise `None` and the request stays
    /// queued. If a request is already out it stays queued until that one is answered.
    pub fn start_queued(
        &mut self,
        ctx: &egui::Context,
        parent: Option<&eframe::Frame>,
        may_start: bool,
    ) -> Option<(Started, PickRequest)> {
        if !may_start || self.pending.is_some() {
            return None;
        }
        let request = self.queued.take()?;
        let started = self.start(ctx, parent, request.clone());
        Some((started, request))
    }

    /// Like [`Picker::poll`]. When it returns an answer (path, cancel or error alike) and
    /// `modal_open` is false, `release` is called once before returning, so the app can make
    /// sure the main window is enabled (Windows restores a disabled owner when a picker
    /// closes). No answer, or a prompt of ours open: `release` is not called.
    pub fn poll_releasing(&mut self, modal_open: bool, release: impl FnOnce()) -> Option<Picked> {
        let picked = self.poll()?;
        if !modal_open {
            release();
        }
        Some(picked)
    }

    /// The backend requests go to.
    pub fn backend(&self) -> &Backend {
        &self.backend
    }

    /// True while a request is out and not yet answered by [`Picker::poll`], or queued.
    pub fn is_busy(&self) -> bool {
        self.pending.is_some() || self.queued.is_some()
    }

    /// Start `request`.
    /// - `Builtin`: returns [`Started::Builtin`], nothing else happens.
    /// - `System`: shows the picker on a new thread (owned by `parent` when given: rfd's
    ///   `set_parent` with the frame's window handle; start folder, file name and filters from
    ///   the request), sends its answer through a channel and calls `ctx.request_repaint()`.
    ///   An error from the picker counts as a cancel.
    /// - `Stub`: records the request, takes the next queued answer (`None` when the queue is
    ///   empty), sends it through the same channel and requests a repaint.
    /// - When a request is already out, the new one is ignored and `Started::Native` returned.
    pub fn start(
        &mut self,
        ctx: &egui::Context,
        parent: Option<&eframe::Frame>,
        request: PickRequest,
    ) -> Started {
        if self.pending.is_some() {
            return Started::Native;
        }
        match &self.backend {
            Backend::Builtin => return Started::Builtin,
            Backend::Stub(stub) => {
                let (sender, receiver) = mpsc::channel();
                {
                    let mut state = stub.state.lock().unwrap();
                    state.requests.push(request.clone());
                    let answer = if state.answers.is_empty() {
                        None
                    } else {
                        state.answers.remove(0)
                    };
                    let _ = sender.send(answer);
                }
                self.pending = Some((request.purpose, receiver));
                ctx.request_repaint();
            }
            Backend::System => {
                let mut dialog = rfd::FileDialog::new()
                    .set_title(request.title.clone())
                    .set_directory(request.dir.clone());
                for filter in &request.filters {
                    dialog = dialog.add_filter(&filter.label, &filter.extensions);
                }
                if !request.name.is_empty() {
                    dialog = dialog.set_file_name(request.name.clone());
                }
                if let Some(parent) = parent {
                    dialog = dialog.set_parent(parent);
                }
                let (sender, receiver) = mpsc::channel();
                let ctx = ctx.clone();
                let kind = request.kind;
                std::thread::spawn(move || {
                    let path = match kind {
                        PickKind::OpenFile => dialog.pick_file(),
                        PickKind::SaveFile => dialog.save_file(),
                        PickKind::Folder => dialog.pick_folder(),
                    };
                    let _ = sender.send(path);
                    ctx.request_repaint();
                });
                self.pending = Some((request.purpose, receiver));
            }
        }
        Started::Native
    }

    /// The answer, once, when it has arrived; never blocks.
    pub fn poll(&mut self) -> Option<Picked> {
        let (purpose, result) = {
            let (purpose, receiver) = self.pending.as_ref()?;
            (*purpose, receiver.try_recv())
        };
        let path = match result {
            Ok(path) => path,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => None,
        };
        self.pending = None;
        Some(Picked {
            purpose,
            path: path.map(|path| finish_path(purpose, path)),
        })
    }
}

/// Make sure the main window accepts input: Windows `EnableWindow(hwnd, TRUE)` on the frame's
/// window handle; nothing on other systems. Harmless when it is enabled already.
#[cfg(windows)]
pub fn enable_main_window(frame: &eframe::Frame) {
    use winit::platform::windows::WindowExtWindows;
    if let Some(window) = frame.winit_window() {
        window.set_enable(true);
    }
}

/// Nothing: only Windows keeps the owner disabled after a picker closes.
#[cfg(not(windows))]
pub fn enable_main_window(_frame: &eframe::Frame) {}
