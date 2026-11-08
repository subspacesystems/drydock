# m11-native-file-dialogs: Open, Save and Export use the system's file picker

Open, Import, Add Models, Save, Save As and Export (to a file and to a folder) show the
system's own file picker, owned by the main window. Our own dialogs stay as the fallback when no
system picker exists, and as the picker in tests. Tyrel agreed (2026-11-08).

## What is already on main

- `crates/drydock/src/file_dialogs.rs`: `OpenDialog`, `SaveDialog`, `SaveMode`, `DialogOutcome`,
  `SaveOutcome`, moved out of `app.rs` unchanged. Do not change them.
- `crates/drydock/src/picker.rs`: the types and function signatures with `unimplemented!()`
  bodies. Read its doc comments: they are the contract. Fill in the bodies; keep the signatures.
- `rfd` 0.17 (`default-features = false`, features `xdg-portal`, `wayland`) in
  `app/Cargo.toml` and `crates/drydock/Cargo.toml`. Its tree adds only `rfd` and `pollster`
  to `Cargo.lock`; all licenses are on the allowed list. Do not add other dependencies.

## 1. `picker.rs` (tests: `picker_requests.rs`, `picker_flow.rs`)

- `PickRequest::open` / `save` and `finish_path` as documented. Titles are the headings our own
  dialogs show. Filters come from `document::OPEN_EXTENSIONS`, `NATIVE_EXTENSION` and
  `ExportFormat::extension` / `label`.
- `choose_backend`, `lists_file_chooser` (match the whole interface name: a longer name that
  merely starts with it does not count), `system_picker_available`.
- `Picker::start` / `poll`:
  - One request at a time; `pending` holds the purpose and the receiver.
  - `System`: build the `rfd::FileDialog` on the calling thread (title, start directory, file
    name, filters, `set_parent(frame)` when a frame is given: `eframe::Frame` implements the
    raw-window-handle traits), then show it on a new thread. The thread sends `Option<PathBuf>`
    through `mpsc` and calls `ctx.request_repaint()`. Use `pick_file`, `save_file` or
    `pick_folder` by `PickKind`. If `rfd` types are not `Send`, build the dialog on the thread and
    pass only the raw window handle, or whatever compiles; the UI thread must never block.
  - Any error or cancel from the picker is `None`.
  - `Stub`: record the request, pop the first queued answer, send it through the same kind of
    channel before `start` returns, request a repaint.
  - `poll` maps the received path through `finish_path` (the `Picked` carries the finished
    path), clears `pending`, and returns it once.
- On Windows `rfd` uses the common item dialog; on Linux the portal. Nothing else is needed from
  us for that, but keep `#[cfg]`s out of the app code: `picker.rs` compiles on every platform and
  `system_picker_available` returns false where `rfd` has no backend.

## 2. Wiring in `app.rs`

- `App` gets a `picker: Picker`, created in `App::new` with
  `choose_backend(std::env::var("DRYDOCK_FILE_PICKER").ok().as_deref(), system_picker_available())`.
  The probe runs once at startup.
- Every place that opens our dialog now builds a request and calls `self.picker.start(ctx,
  Some(frame), request)`; when it returns `Started::Builtin` it opens our dialog exactly as today.
  Sites: `open_dialog`, `open_import_dialog`, `open_models_dialog` (these need `ctx` and `frame`
  passed in; update their callers), `open_save_dialog`, `open_export_save_dialog`. Starting
  folder and suggested name are computed as today (`open_folder`, the document's folder and file
  name, `mesh_export::default_export_name`). `open_export_save_dialog` picks `SaveMode::ExportFolder`
  for STL one file per object, as now.
- Once per frame, where the builtin dialogs are drawn, `if let Some(picked) = self.picker.poll()`:
  `path: None` does nothing (for a save also clear `after_save`, as the builtin Cancel does);
  `Some(path)` goes through the same code as a builtin answer. Pull that code out of the two
  `match mode` blocks into `finish_open(mode, path)` and `finish_save(mode, path)` and call them
  from both the builtin result and `poll`, so there is one code path. The system picker asks
  before replacing a file itself, so there is no extra replace question on that path. For
  `ExportFolder` the picked folder is passed to `export_to` as it is.
- While `self.picker.is_busy()` count it as an open dialog wherever the app tests
  `dialog.is_some()` / `save_dialog.is_some()` for the keyboard shortcuts and tool input (the
  `dialog_open` computation and the check near it), so keys do not reach the viewport.
- `Save` with an untitled or DAE document goes to `open_save_dialog` as before.
- `recent.rs` is not involved: Open still records the file in the recent list through the
  existing open path.
- No new menu items or labels. Nothing user-visible changes with the builtin backend.

## Tests

- `picker_requests.rs`, `picker_flow.rs`: pure, must pass.
- `file_dialogs_ui.rs`: our dialogs still work after the move (already passes).
- `harness/oracle` green. Run `cargo deny check licenses bans` and report if `rfd`'s tree fails
  (it should not).
- Not testable here: the portal and Windows pickers. The sign-off session checks the fallback
  with `DRYDOCK_FILE_PICKER=builtin` in `gui-session`; Tyrel checks the system picker on Ubuntu
  GNOME and Windows.

## Where to look

`show Picker`, `show PickRequest`, `show StubPicker` (picker.rs skeleton); `show OpenDialog::show`,
`show SaveDialog::show`, `show SaveDialog::target` (file_dialogs.rs); in `app.rs`:
`show App::open_dialog`, `App::open_folder`, `App::open_save_dialog`, `App::open_export_save_dialog`,
`App::export_to`, `App::import_component_file`, `App::import_mesh_file`, `App::new`, and
`outline app.rs` for the `DialogOutcome` / `SaveOutcome` blocks (search `dialog.show`) and
`dialog_open`; `portal::read_scheme` (how to run `gdbus` with a timeout); `document::native_path`,
`document::OPEN_EXTENSIONS`; `mesh_export::ExportOptions`, `default_export_name`;
`open_buttons::OpenMode`. rfd docs: `~/.cargo/registry/src/*/rfd-0.17.2/src/file_dialog.rs`.
