# m13-update-launch-fix: links open in the browser; the installer starts through the shell

Two bugs from a real update test on Windows (0.13.0 to 0.13.1): the "Release notes" link in the
"ready to install" prompt did nothing, and "Restart and update" failed with "Could not start the
installer: The requested operation requires elevation (os error 740)".

## 1. Links (already switched on in the skeleton)

egui-winit was built without its `links` feature, so every `ui.hyperlink_to` (Release notes,
About's sponsor link) only logged "feature links not enabled". The skeleton commit on main turns
the feature on in `app/Cargo.toml` (`egui-winit` entry) and adds the new crates to `Cargo.lock`
(webbrowser, url, idna and the icu crates; all MIT, Apache-2.0 or Unicode-3.0). Do not touch
either file for this. `tests/links_feature.rs` keeps it from regressing. Nothing else to do for
links, except: if a gate you can run shows a problem, say so in your final notes.

## 2. Start the installer through the Windows shell

The installer asks for administrator rights (`RequestExecutionLevel admin`). `CreateProcess`
(what `std::process::Command::spawn` uses) fails with error 740 instead of showing the elevation
prompt; `ShellExecuteExW` with the verb `runas` shows it.

### Pure helpers (`drydock-update/src/shell.rs`, skeleton on main)

Implement `quote_arg`, `join_parameters` and `launch_failure` as documented there. Tests:
`crates/drydock-update/tests/shell.rs`.

### Windows launcher (`drydock-update/src/winshell.rs`, skeleton on main)

`run_elevated(installer, args)`:
- Build a `SHELLEXECUTEINFOW`: `cbSize`, `fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC`
  (close the process handle you get back right away; Drydock does not wait for the installer),
  `lpVerb = "runas"`, `lpFile` = the installer path as a NUL-terminated UTF-16 string
  (`OsStrExt::encode_wide`; no quotes around it), `lpParameters = join_parameters(args)`,
  `nShow = SW_SHOWNORMAL`. Leave `lpDirectory` null.
- Call `ShellExecuteExW`. It blocks while the elevation prompt is up; that is fine (the UI thread
  waits, same as it does for the file dialogs). Do not spawn a thread.
- On failure, take the Win32 error code from the `windows::core::Error` (`error.code()` is an
  HRESULT; a Win32 error is `0x8007xxxx`, so the code is its low 16 bits) and return
  `Err(launch_failure(code, &message))`
  where `message` is `error.message()`. A declined prompt gives `Err("cancelled")`
  (`LAUNCH_CANCELLED`).
- Unsafe code: follow `docs/adr/009-winhttp-unsafe.md`; `winshell.rs` has the file-level
  `#![allow(unsafe_code)]` (the crate denies it elsewhere). Keep each `unsafe` block small with a
  one-line reason. Add any further `windows` crate features that this needs to `drydock-update`'s
  Cargo.toml (the skeleton has `Win32_UI_Shell` and `Win32_UI_WindowsAndMessaging`; likely also
  `Win32_System_Registry`, and `Win32_System_Threading` or `Win32_Foundation` for
  `CloseHandle`). The drydock crate itself keeps `unsafe_code = "forbid"`: it calls the safe
  `run_elevated`.
- The code is checked by the oracle's `winclippy` gate (it only compiles for Windows; it cannot
  run on this host). Check it as far as you can: `cargo clippy --target x86_64-pc-windows-gnu`
  may be installed in the worker image; if it is not, read the `windows` 0.62 sources in the
  registry (`show <name> windows`) for exact signatures.

### The launcher and the window (`drydock/src/update_ui.rs`)

- `ProcessLauncher::launch`: on Windows, `drydock_update::run_elevated(installer, &INSTALLER_ARGS)`
  (the `Err` string passes through unchanged); elsewhere keep `Command::spawn` as it is.
- `UpdateUi::launch`: when the launcher's `Err(reason)` equals `drydock_update::LAUNCH_CANCELLED`,
  set the status text to exactly `Update cancelled` and return `Step::Stay`. Any other reason keeps
  today's `Could not start the installer: {reason}`. A success clears the line and returns
  `Step::Quit` as today. The window must keep the update ready (the user can click "Restart and
  update" again) and Drydock must keep running: nothing is quit, closed or saved because of a
  cancel. Check the existing flow only quits after the launch succeeded (it does today; keep it).
- Update the doc comments on `begin_restart` and `Launcher` to mention the cancelled line.
- The line stays visible until the next check, `Later`, or a successful launch (the same rules
  as the existing launcher error).

## Tests

- `crates/drydock-update/tests/shell.rs`: the pure helpers (fail until implemented).
- `crates/drydock/tests/update_ui.rs`: the recording launcher has a `cancel` switch; new tests
  check a declined prompt leaves the app running with the calm line, in the plain flow, after
  "Don't save" and after "Save", and in the window; other errors keep their text.
- `crates/drydock/tests/links_feature.rs`: egui-winit's resolved features include `links` (passes
  from the skeleton on; it is the regression guard).
- The real elevation prompt can only be tried on Windows by hand (Tyrel).

## Where to look

- `show ProcessLauncher`, `show Launcher`, `show INSTALLER_ARGS`, `show UpdateUi::launch`,
  `show UpdateUi::status_text` (all `drydock/src/update_ui.rs`).
- `drydock-update/src/shell.rs` and `winshell.rs` (skeletons), `drydock-update/src/lib.rs`
  (exports).
- `drydock-update/src/winhttp.rs`: the `windows` crate unsafe pattern (handles, UTF-16 strings,
  `windows::core::Error`); `docs/adr/009-winhttp-unsafe.md`.
- Test helpers in `drydock/tests/update_ui.rs`: `Rec`, `ready_ui`, `ready`, `launched`, `has`.
- `packaging/windows/drydock.nsi` (admin level, relaunch not elevated: leave it alone).
