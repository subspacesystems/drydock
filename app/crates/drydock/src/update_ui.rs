// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The app's side of updating: the three-choice setting, the update channel, the Updates
//! window (Help > Check for Updates), the "ready to install" prompt and the restart guard.
//! Behaviour: `docs/specs/m13-update-ui.md`. The skeleton below is on main; fill in the bodies,
//! keep every signature, and do not edit `tests/update_ui.rs`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use drydock_update::{Channel, Mode, State, Transport, Version};
use eframe::egui;

use crate::prompts::UnsavedChoice;

/// Arguments given to the downloaded installer (`docs/specs/m13-installer-update.md`).
pub const INSTALLER_ARGS: [&str; 2] = ["/S", "/UPDATE"];

/// Build-time URL of the built-in channel's manifest (`DRYDOCK_UPDATE_URL`; the release script
/// sets it). `None` in ordinary builds: no checking.
pub const BUILT_IN_URL: Option<&str> = option_env!("DRYDOCK_UPDATE_URL");

/// Build-time public key as 64 hex digits (`DRYDOCK_UPDATE_KEY`); ordinary builds use the
/// throwaway test key in `packaging/update-key.pub`.
pub const BUILT_IN_KEY: Option<&str> = option_env!("DRYDOCK_UPDATE_KEY");

/// Turn updating on for tests on any OS (a process-wide flag read by [`supported`]).
pub fn force_supported(on: bool) {
    let _ = on;
    unimplemented!("m13-update-ui")
}

/// Updating is offered: the OS is Windows, or a test forced it with [`force_supported`]. The
/// menu item, the window and the checks all gate on this one function.
pub fn supported() -> bool {
    unimplemented!("m13-update-ui")
}

/// The user's choice, kept in the file `updates` of the config folder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UpdateSetting {
    /// "Check automatically" (the default).
    #[default]
    Auto,
    /// "Notify only".
    NotifyOnly,
    /// "Off".
    Off,
}

impl UpdateSetting {
    pub const ALL: [UpdateSetting; 3] = [
        UpdateSetting::Auto,
        UpdateSetting::NotifyOnly,
        UpdateSetting::Off,
    ];

    /// "Check automatically", "Notify only" or "Off".
    pub fn label(self) -> &'static str {
        unimplemented!("m13-update-ui")
    }

    /// The updater's mode for this choice.
    pub fn mode(self) -> Mode {
        unimplemented!("m13-update-ui")
    }

    /// One record per line (`\r\n` also ends a line): `check auto`, `check notify` or
    /// `check off`. Unknown lines and values are ignored, the last good line wins, and
    /// anything else (empty text) gives the default.
    pub fn parse(text: &str) -> UpdateSetting {
        let _ = text;
        unimplemented!("m13-update-ui")
    }

    /// `check auto\n`, `check notify\n` or `check off\n`.
    pub fn to_text(self) -> String {
        unimplemented!("m13-update-ui")
    }

    /// Read `file`; a missing or unreadable file gives the default.
    pub fn load(file: &Path) -> UpdateSetting {
        let _ = file;
        unimplemented!("m13-update-ui")
    }

    /// Write `file` (creating its folder) through a temporary file and a rename, as
    /// `PrintSettings::save` does.
    pub fn save(self, file: &Path) -> std::io::Result<()> {
        let _ = file;
        unimplemented!("m13-update-ui")
    }
}

/// `updates` in `recent::config_dir`.
pub fn settings_file(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let _ = var;
    unimplemented!("m13-update-ui")
}

/// `update-channel` in `recent::config_dir`: a channel definition that overrides the built-in one.
pub fn channel_file(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let _ = var;
    unimplemented!("m13-update-ui")
}

/// `update-downloads` in `recent::config_dir`: where installers are downloaded to.
pub fn download_dir(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let _ = var;
    unimplemented!("m13-update-ui")
}

/// The public key from a key file's text: the first line that is not blank and does not start
/// with `#`, 64 hex digits (`packaging/update-key.pub` has a comment line first).
pub fn parse_key_file(text: &str) -> Option<[u8; 32]> {
    let _ = text;
    unimplemented!("m13-update-ui")
}

/// The built-in channel: name "built-in", `url` as the manifest URL (must be `https://`; else
/// `None`, as for `None`), `key_hex` the public key as 64 hex digits (else `None`), no headers.
pub fn built_in_channel(url: Option<&str>, key_hex: &str) -> Option<Channel> {
    let _ = (url, key_hex);
    unimplemented!("m13-update-ui")
}

/// The channel to use: `file` (the channel file's bytes) when it parses (`Channel::parse`),
/// else `built_in`. A file that does not parse is ignored with one `eprintln!` line starting
/// `drydock:` (never shown to the user); `None` for both gives `None`: no checking.
pub fn resolve_channel(built_in: Option<Channel>, file: Option<&[u8]>) -> Option<Channel> {
    let _ = (built_in, file);
    unimplemented!("m13-update-ui")
}

/// Starts the downloaded installer. The app's launcher runs the file with [`INSTALLER_ARGS`]
/// and does not wait; tests use a recording one.
pub trait Launcher: Send {
    fn launch(&mut self, installer: &Path) -> Result<(), String>;
}

/// The real launcher: `std::process::Command::new(installer).args(INSTALLER_ARGS).spawn()`.
pub struct ProcessLauncher;

impl Launcher for ProcessLauncher {
    fn launch(&mut self, installer: &Path) -> Result<(), String> {
        let _ = installer;
        unimplemented!("m13-update-ui")
    }
}

/// Everything [`UpdateUi::new`] needs.
pub struct Parts {
    /// `None`: no channel, so no updater and no network.
    pub channel: Option<Channel>,
    pub transport: Arc<dyn Transport>,
    pub launcher: Box<dyn Launcher>,
    /// Where the setting is kept (`None`: not saved).
    pub settings_file: Option<PathBuf>,
    pub download_dir: PathBuf,
    pub running: Version,
    /// The first value of the setting (the caller loaded it).
    pub setting: UpdateSetting,
    /// Passed to the updater's `Config`.
    pub startup_delay: Duration,
    pub interval: Duration,
}

/// What the user asked for in the windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateEvent {
    /// "Restart and update" was clicked: the app calls [`UpdateUi::begin_restart`].
    Restart,
}

/// What the app does next in the restart flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Show the unsaved-changes prompt; answer with [`UpdateUi::answer_unsaved`].
    Ask,
    /// Save the document, then call [`UpdateUi::after_saved`] (or [`UpdateUi::cancel_restart`]
    /// when the save was cancelled).
    Save,
    /// The installer was started: quit the app now, without asking again.
    Quit,
    /// Nothing started; the app keeps running.
    Stay,
}

/// The updater, the setting, the windows and the restart guard.
#[allow(dead_code)]
pub struct UpdateUi {
    // Fields are the worker's to choose (an `Arc<Updater>`, the `Background` handle, the
    // setting and its file, the launcher, window and prompt flags, ...).
    placeholder: (),
}

impl UpdateUi {
    /// With a channel: build the `Updater` (mode from `parts.setting`) and `spawn` its schedule.
    /// Without: nothing is created and nothing touches the network.
    pub fn new(parts: Parts) -> UpdateUi {
        let _ = parts;
        unimplemented!("m13-update-ui")
    }

    /// A channel is configured.
    pub fn has_channel(&self) -> bool {
        unimplemented!("m13-update-ui")
    }

    pub fn setting(&self) -> UpdateSetting {
        unimplemented!("m13-update-ui")
    }

    /// Change the setting: tell the updater (`set_mode`) and save the file (an error is
    /// ignored; the choice still holds for this run).
    pub fn set_setting(&mut self, setting: UpdateSetting) {
        let _ = setting;
        unimplemented!("m13-update-ui")
    }

    /// Help > Check for Updates: open the Updates window and start a manual check
    /// ([`UpdateUi::check_now`]).
    pub fn open_dialog(&mut self) {
        unimplemented!("m13-update-ui")
    }

    pub fn dialog_open(&self) -> bool {
        unimplemented!("m13-update-ui")
    }

    /// A manual check (works in every mode); does nothing without a channel.
    pub fn check_now(&mut self) {
        unimplemented!("m13-update-ui")
    }

    /// The updater's state; `None` without a channel.
    pub fn state(&self) -> Option<State> {
        unimplemented!("m13-update-ui")
    }

    /// The line for the status bar after a manual check: `None` until the user has asked
    /// (automatic checks never show anything, not even a failure) and after
    /// [`UpdateUi::clear_status`]. The texts are in the spec ("Status texts").
    pub fn status_text(&self) -> Option<String> {
        unimplemented!("m13-update-ui")
    }

    /// Forget the manual-check report (the app calls it where it clears `export_note`).
    pub fn clear_status(&mut self) {
        unimplemented!("m13-update-ui")
    }

    /// Per frame: with the setting `Auto` and a manual check that found an update, ask for the
    /// download once per version (`Background::download_now`); keep the UI repainting while a
    /// check or download runs. Then draw the Updates window (while open) and the "ready"
    /// prompt (while [`UpdateUi::prompt_showing`]). Returns what was clicked.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<UpdateEvent> {
        let _ = ctx;
        unimplemented!("m13-update-ui")
    }

    /// The state is `Ready` and the user has not answered "Later" for that version this run.
    pub fn prompt_showing(&self) -> bool {
        unimplemented!("m13-update-ui")
    }

    /// "Restart and update": with the state not `Ready` -> `Stay`. With `modified` (unsaved
    /// changes) -> `Ask`, nothing launched. Otherwise start the installer: `Quit` when the
    /// launcher succeeds; on failure the status text says so ("Could not start the installer:
    /// {reason}") and the step is `Stay`.
    pub fn begin_restart(&mut self, modified: bool) -> Step {
        let _ = modified;
        unimplemented!("m13-update-ui")
    }

    /// The unsaved-changes answer: `Save` -> `Save`; `Discard` -> start the installer as in
    /// `begin_restart`; `Cancel` -> `Stay` and the installer is not started.
    pub fn answer_unsaved(&mut self, choice: UnsavedChoice) -> Step {
        let _ = choice;
        unimplemented!("m13-update-ui")
    }

    /// The save asked for by `Step::Save` finished: `true` starts the installer (as above),
    /// `false` (the save failed or was cancelled) is `Stay`.
    pub fn after_saved(&mut self, saved: bool) -> Step {
        let _ = saved;
        unimplemented!("m13-update-ui")
    }

    /// The restart flow was abandoned (the save dialog was cancelled): nothing is launched.
    pub fn cancel_restart(&mut self) {
        unimplemented!("m13-update-ui")
    }

    /// A restart is waiting for the unsaved-changes answer or for the save.
    pub fn restart_pending(&self) -> bool {
        unimplemented!("m13-update-ui")
    }
}
