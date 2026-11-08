// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Open, Import, Add Models, Save, Save As and Export through the system's own file picker
//! (`docs/specs/m11-native-file-dialogs.md`). The app describes what it wants as a
//! [`PickRequest`]; a [`Picker`] either shows the system picker on its own thread and hands the
//! answer back through a channel ([`Picker::poll`]), or tells the app to open its own dialog
//! (`crate::file_dialogs`) because no system picker is available. A [`Backend::Stub`] answers
//! with given paths so tests can check what the app asked for.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

use eframe::egui;

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
        let _ = (mode, dir);
        unimplemented!("m11-native-file-dialogs")
    }

    /// A request for `mode` starting in `dir`, suggesting `name`.
    /// - `Model`: title "Save model", kind `SaveFile`, one filter labeled "Drydock model" with
    ///   `["drydock"]`.
    /// - `Export(options)`: title `"Export {format label}"`, kind `SaveFile`, one filter
    ///   labeled with the format label (`"STL"`, `"3MF"`) with its extension.
    /// - `ExportFolder(options)`: title `"Export {format label} to folder"`, kind `Folder`, no
    ///   filters, empty name (a folder picker has no name box).
    pub fn save(mode: SaveMode, dir: PathBuf, name: String) -> PickRequest {
        let _ = (mode, dir, name);
        unimplemented!("m11-native-file-dialogs")
    }
}

/// The path to hand to the open/save/export code for a path the system picker returned.
/// - Open and folder: unchanged.
/// - `Save(Model)`: `document::native_path` (adds `.drydock` unless it already has a native
///   extension).
/// - `Save(Export(options))`: unchanged when it has an extension, else the format's extension
///   added.
pub fn finish_path(purpose: PickPurpose, path: PathBuf) -> PathBuf {
    let _ = (purpose, path);
    unimplemented!("m11-native-file-dialogs")
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
    let _ = (setting, available);
    unimplemented!("m11-native-file-dialogs")
}

/// True when the text `gdbus introspect --session --dest org.freedesktop.portal.Desktop
/// --object-path /org/freedesktop/portal/desktop` prints lists the interface
/// `org.freedesktop.portal.FileChooser`.
pub fn lists_file_chooser(introspection: &str) -> bool {
    let _ = introspection;
    unimplemented!("m11-native-file-dialogs")
}

/// Whether the system picker can be used here. Windows: always true. Linux: runs that
/// `gdbus introspect` (about 2 seconds at most, like `portal::read_scheme`) and checks it with
/// [`lists_file_chooser`]; false when `gdbus` is missing or fails. Other systems: false.
pub fn system_picker_available() -> bool {
    unimplemented!("m11-native-file-dialogs")
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
}

impl Picker {
    pub fn new(backend: Backend) -> Picker {
        Picker {
            backend,
            pending: None,
        }
    }

    /// The backend requests go to.
    pub fn backend(&self) -> &Backend {
        &self.backend
    }

    /// True while a request is out and not yet answered by [`Picker::poll`].
    pub fn is_busy(&self) -> bool {
        self.pending.is_some()
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
        let _ = (ctx, parent, request);
        unimplemented!("m11-native-file-dialogs")
    }

    /// The answer, once, when it has arrived; never blocks.
    pub fn poll(&mut self) -> Option<Picked> {
        unimplemented!("m11-native-file-dialogs")
    }
}
