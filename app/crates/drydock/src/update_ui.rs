// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The app's side of updating: the three-choice setting, the update channel, the Updates
//! window (Help > Check for Updates), the "ready to install" prompt and the restart guard.
//! Behaviour: `docs/specs/m13-update-ui.md`. The skeleton below is on main; fill in the bodies,
//! keep every signature, and do not edit `tests/update_ui.rs`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use drydock_update::{
    Background, Channel, Config, Mode, State, Transport, Updater, Version, hex_decode_32,
};
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

static FORCED_SUPPORTED: AtomicBool = AtomicBool::new(false);

/// Turn updating on for tests on any OS (a process-wide flag read by [`supported`]).
pub fn force_supported(on: bool) {
    FORCED_SUPPORTED.store(on, Ordering::SeqCst);
}

/// Updating is offered: the OS is Windows, or a test forced it with [`force_supported`]. The
/// menu item, the window and the checks all gate on this one function.
pub fn supported() -> bool {
    cfg!(windows) || FORCED_SUPPORTED.load(Ordering::SeqCst)
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
        match self {
            UpdateSetting::Auto => "Check automatically",
            UpdateSetting::NotifyOnly => "Notify only",
            UpdateSetting::Off => "Off",
        }
    }

    /// The updater's mode for this choice.
    pub fn mode(self) -> Mode {
        match self {
            UpdateSetting::Auto => Mode::Auto,
            UpdateSetting::NotifyOnly => Mode::NotifyOnly,
            UpdateSetting::Off => Mode::Off,
        }
    }

    /// One record per line (`\r\n` also ends a line): `check auto`, `check notify` or
    /// `check off`. Unknown lines and values are ignored, the last good line wins, and
    /// anything else (empty text) gives the default.
    pub fn parse(text: &str) -> UpdateSetting {
        let mut setting = UpdateSetting::default();
        for line in text.lines() {
            match line.trim() {
                "check auto" => setting = UpdateSetting::Auto,
                "check notify" => setting = UpdateSetting::NotifyOnly,
                "check off" => setting = UpdateSetting::Off,
                _ => {}
            }
        }
        setting
    }

    /// `check auto\n`, `check notify\n` or `check off\n`.
    pub fn to_text(self) -> String {
        let value = match self {
            UpdateSetting::Auto => "auto",
            UpdateSetting::NotifyOnly => "notify",
            UpdateSetting::Off => "off",
        };
        format!("check {value}\n")
    }

    /// Read `file`; a missing or unreadable file gives the default.
    pub fn load(file: &Path) -> UpdateSetting {
        std::fs::read_to_string(file)
            .map(|text| UpdateSetting::parse(&text))
            .unwrap_or_default()
    }

    /// Write `file` (creating its folder) through a temporary file and a rename, as
    /// `PrintSettings::save` does.
    pub fn save(self, file: &Path) -> std::io::Result<()> {
        let folder = file.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(folder)?;
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "updates".to_owned());
        let temp = folder.join(format!(".{name}.tmp"));
        std::fs::write(&temp, self.to_text())?;
        std::fs::rename(&temp, file)
    }
}

/// `updates` in `recent::config_dir`.
pub fn settings_file(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    crate::recent::config_dir(var).map(|dir| dir.join("updates"))
}

/// `update-channel` in `recent::config_dir`: a channel definition that overrides the built-in one.
pub fn channel_file(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    crate::recent::config_dir(var).map(|dir| dir.join("update-channel"))
}

/// `update-downloads` in `recent::config_dir`: where installers are downloaded to.
pub fn download_dir(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    crate::recent::config_dir(var).map(|dir| dir.join("update-downloads"))
}

/// The public key from a key file's text: the first line that is not blank and does not start
/// with `#`, 64 hex digits (`packaging/update-key.pub` has a comment line first).
pub fn parse_key_file(text: &str) -> Option<[u8; 32]> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .and_then(hex_decode_32)
}

/// The built-in channel: name "built-in", `url` as the manifest URL (must be `https://`; else
/// `None`, as for `None`), `key_hex` the public key as 64 hex digits (else `None`), no headers.
pub fn built_in_channel(url: Option<&str>, key_hex: &str) -> Option<Channel> {
    let url = url?;
    if !url.starts_with("https://") {
        return None;
    }
    Some(Channel {
        name: "built-in".to_owned(),
        manifest_url: url.to_owned(),
        public_key: hex_decode_32(key_hex)?,
        headers: Vec::new(),
    })
}

/// The channel to use: `file` (the channel file's bytes) when it parses (`Channel::parse`),
/// else `built_in`. A file that does not parse is ignored with one `eprintln!` line starting
/// `drydock:` (never shown to the user); `None` for both gives `None`: no checking.
pub fn resolve_channel(built_in: Option<Channel>, file: Option<&[u8]>) -> Option<Channel> {
    let from_file = file.and_then(|bytes| match Channel::parse(bytes) {
        Ok(channel) => Some(channel),
        Err(error) => {
            eprintln!("drydock: ignoring update-channel file: {error:?}");
            None
        }
    });
    from_file.or(built_in)
}

/// Starts the downloaded installer. The app's launcher runs the file with [`INSTALLER_ARGS`]
/// and does not wait; tests use a recording one.
pub trait Launcher: Send {
    fn launch(&mut self, installer: &Path) -> Result<(), String>;
}

/// The real launcher. On Windows: `drydock_update::run_elevated(installer, INSTALLER_ARGS)`
/// (the shell shows the elevation prompt). Elsewhere: `std::process::Command::new(installer)
/// .args(INSTALLER_ARGS).spawn()`, not waited for. A declined prompt is
/// `Err(drydock_update::LAUNCH_CANCELLED.to_owned())`.
pub struct ProcessLauncher;

impl Launcher for ProcessLauncher {
    fn launch(&mut self, installer: &Path) -> Result<(), String> {
        std::process::Command::new(installer)
            .args(INSTALLER_ARGS)
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Restart {
    None,
    Asking,
    Saving,
}

/// The updater, the setting, the windows and the restart guard.
pub struct UpdateUi {
    updater: Option<Arc<Updater>>,
    background: Option<Background>,
    setting: UpdateSetting,
    settings_file: Option<PathBuf>,
    launcher: Box<dyn Launcher>,
    dialog_open: bool,
    /// A manual check is in flight or has produced the status text.
    manual: bool,
    /// The manual check was asked for but the updater has not left Idle yet.
    check_requested: bool,
    launcher_error: Option<String>,
    requested_download: Option<Version>,
    later: Option<Version>,
    /// The ready prompt was seen for a frame; draw it from the next one.
    prompt_armed: bool,
    restart: Restart,
}

impl UpdateUi {
    /// With a channel: build the `Updater` (mode from `parts.setting`) and `spawn` its schedule.
    /// Without: nothing is created and nothing touches the network.
    pub fn new(parts: Parts) -> UpdateUi {
        let Parts {
            channel,
            transport,
            launcher,
            settings_file,
            download_dir,
            running,
            setting,
            startup_delay,
            interval,
        } = parts;
        let (updater, background) = match channel {
            Some(channel) => {
                let updater = Arc::new(Updater::new(
                    Config {
                        channel,
                        running,
                        mode: setting.mode(),
                        download_dir,
                        startup_delay,
                        interval,
                    },
                    transport,
                ));
                let background = updater.spawn();
                (Some(updater), Some(background))
            }
            None => (None, None),
        };
        UpdateUi {
            updater,
            background,
            setting,
            settings_file,
            launcher,
            dialog_open: false,
            manual: false,
            check_requested: false,
            launcher_error: None,
            requested_download: None,
            later: None,
            prompt_armed: false,
            restart: Restart::None,
        }
    }

    /// A channel is configured.
    pub fn has_channel(&self) -> bool {
        self.updater.is_some()
    }

    pub fn setting(&self) -> UpdateSetting {
        self.setting
    }

    /// Change the setting: tell the updater (`set_mode`) and save the file (an error is
    /// ignored; the choice still holds for this run).
    pub fn set_setting(&mut self, setting: UpdateSetting) {
        self.setting = setting;
        if let Some(updater) = &self.updater {
            updater.set_mode(setting.mode());
        }
        if let Some(file) = &self.settings_file {
            let _ = setting.save(file);
        }
    }

    /// Help > Check for Updates: open the Updates window and start a manual check
    /// ([`UpdateUi::check_now`]).
    pub fn open_dialog(&mut self) {
        self.dialog_open = true;
        self.check_now();
    }

    pub fn dialog_open(&self) -> bool {
        self.dialog_open
    }

    /// A manual check (works in every mode); does nothing without a channel.
    pub fn check_now(&mut self) {
        if let Some(background) = &self.background {
            self.manual = true;
            self.launcher_error = None;
            self.check_requested = true;
            background.check_now();
        }
    }

    /// The updater's state; `None` without a channel.
    pub fn state(&self) -> Option<State> {
        self.updater.as_ref().map(|updater| updater.state())
    }

    /// The line for the status bar after a manual check: `None` until the user has asked
    /// (automatic checks never show anything, not even a failure) and after
    /// [`UpdateUi::clear_status`]. The texts are in the spec ("Status texts").
    pub fn status_text(&self) -> Option<String> {
        if let Some(error) = &self.launcher_error {
            return Some(error.clone());
        }
        if !self.manual {
            return None;
        }
        match self.state()? {
            State::Idle => None,
            State::Checking => Some("Checking for updates...".to_owned()),
            State::UpToDate => Some("Drydock is up to date.".to_owned()),
            State::Failed(_) => Some("Couldn't reach the update server.".to_owned()),
            State::Available(manifest) => {
                Some(format!("Drydock {} is available.", manifest.version))
            }
            State::Downloading { manifest, .. } => {
                Some(format!("Downloading Drydock {}...", manifest.version))
            }
            State::Ready { manifest, .. } => {
                Some(format!("Drydock {} is ready to install.", manifest.version))
            }
        }
    }

    /// Forget the manual-check report (the app calls it where it clears `export_note`).
    pub fn clear_status(&mut self) {
        self.manual = false;
        self.launcher_error = None;
    }

    /// Per frame: with the setting `Auto` and a manual check that found an update, ask for the
    /// download once per version (`Background::download_now`); keep the UI repainting while a
    /// check or download runs. Then draw the Updates window (while open) and the "ready"
    /// prompt (while [`UpdateUi::prompt_showing`]). Returns what was clicked.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<UpdateEvent> {
        if !supported() {
            return None;
        }
        let state = self.state();
        let showing = self.prompt_showing();
        if showing && !self.prompt_armed {
            self.prompt_armed = true;
            ctx.request_repaint_after(Duration::from_millis(250));
        } else if !showing {
            self.prompt_armed = false;
        }
        let draw_prompt = showing && self.prompt_armed;
        if self.setting == UpdateSetting::Auto
            && let Some(State::Available(manifest)) = &state
            && self.requested_download.as_ref() != Some(&manifest.version)
            && let Some(background) = &self.background
        {
            background.download_now();
            self.requested_download = Some(manifest.version);
        }
        let working = matches!(
            state,
            Some(State::Checking) | Some(State::Downloading { .. })
        );
        let asked = self.check_requested && matches!(state, Some(State::Idle) | None);
        if working || asked {
            // egui subtracts the predicted frame time from the delay; add it back so the
            // request stays in the future even when a test steps in large increments.
            let frame = ctx.input(|input| input.predicted_dt);
            ctx.request_repaint_after(Duration::from_millis(250) + Duration::from_secs_f32(frame));
        }
        if self.check_requested && !matches!(state, Some(State::Idle) | None) {
            self.check_requested = false;
        }

        let has_channel = self.has_channel();
        let status = self.status_text();
        let mut chosen = self.setting;
        let mut check_clicked = false;
        let mut download_clicked = false;
        if self.dialog_open {
            let mut open = self.dialog_open;
            crate::dialog::dialog_window(
                ctx,
                &crate::dialog::DialogWindow {
                    title: "Updates",
                    size: egui::vec2(360.0, 260.0),
                },
                &mut open,
                |ui| {
                    if !has_channel {
                        ui.label("Updates are not set up in this build.");
                        return;
                    }
                    ui.label(format!("Drydock {}", crate::chrome::VERSION));
                    for option in UpdateSetting::ALL {
                        ui.radio_value(&mut chosen, option, option.label());
                    }
                    if ui.button("Check now").clicked() {
                        check_clicked = true;
                    }
                    if let Some(text) = &status {
                        ui.label(text);
                    }
                    if matches!(state, Some(State::Available(_)))
                        && self.setting == UpdateSetting::NotifyOnly
                        && ui.button("Download").clicked()
                    {
                        download_clicked = true;
                    }
                },
            );
            self.dialog_open = open;
        }
        if chosen != self.setting {
            self.set_setting(chosen);
        }
        if check_clicked {
            self.check_now();
        }
        if download_clicked && let Some(background) = &self.background {
            background.download_now();
        }

        let mut event = None;
        let mut later = false;
        if draw_prompt && let Some(State::Ready { manifest, .. }) = self.state() {
            let mut open = true;
            crate::dialog::dialog_window(
                ctx,
                &crate::dialog::DialogWindow {
                    title: "Update ready",
                    size: egui::vec2(360.0, 180.0),
                },
                &mut open,
                |ui| {
                    ui.label(format!("Drydock {} is ready to install", manifest.version));
                    let link = ui.hyperlink_to("Release notes", &manifest.notes_url);
                    link.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Link, true, "Release notes")
                    });
                    if crate::look::primary_button(ui, "Restart and update").clicked() {
                        event = Some(UpdateEvent::Restart);
                    }
                    if ui.button("Later").clicked() {
                        later = true;
                    }
                },
            );
            if later || !open {
                self.later = Some(manifest.version);
                self.clear_status();
            }
        }
        event
    }

    /// The state is `Ready` and the user has not answered "Later" for that version this run.
    pub fn prompt_showing(&self) -> bool {
        matches!(
            self.state(),
            Some(State::Ready { manifest, .. }) if self.later != Some(manifest.version)
        )
    }

    /// "Restart and update": with the state not `Ready` -> `Stay`. With `modified` (unsaved
    /// changes) -> `Ask`, nothing launched. Otherwise start the installer: `Quit` when the
    /// launcher succeeds; on failure the status text says so ("Could not start the installer:
    /// {reason}") and the step is `Stay`.
    pub fn begin_restart(&mut self, modified: bool) -> Step {
        if !matches!(self.state(), Some(State::Ready { .. })) {
            return Step::Stay;
        }
        if modified {
            self.restart = Restart::Asking;
            Step::Ask
        } else {
            self.restart = Restart::None;
            self.launch()
        }
    }

    /// The unsaved-changes answer: `Save` -> `Save`; `Discard` -> start the installer as in
    /// `begin_restart`; `Cancel` -> `Stay` and the installer is not started.
    pub fn answer_unsaved(&mut self, choice: UnsavedChoice) -> Step {
        match choice {
            UnsavedChoice::Save => {
                self.restart = Restart::Saving;
                Step::Save
            }
            UnsavedChoice::Discard => {
                self.restart = Restart::None;
                self.launch()
            }
            UnsavedChoice::Cancel => {
                self.restart = Restart::None;
                Step::Stay
            }
        }
    }

    /// The save asked for by `Step::Save` finished: `true` starts the installer (as above),
    /// `false` (the save failed or was cancelled) is `Stay`.
    pub fn after_saved(&mut self, saved: bool) -> Step {
        self.restart = Restart::None;
        if saved { self.launch() } else { Step::Stay }
    }

    /// The restart flow was abandoned (the save dialog was cancelled): nothing is launched.
    pub fn cancel_restart(&mut self) {
        self.restart = Restart::None;
    }

    /// A restart is waiting for the unsaved-changes answer or for the save.
    pub fn restart_pending(&self) -> bool {
        matches!(self.restart, Restart::Asking | Restart::Saving)
    }

    fn launch(&mut self) -> Step {
        let Some(State::Ready { path, .. }) = self.state() else {
            return Step::Stay;
        };
        match self.launcher.launch(&path) {
            Ok(()) => {
                self.launcher_error = None;
                Step::Quit
            }
            Err(reason) => {
                self.launcher_error = Some(format!("Could not start the installer: {reason}"));
                Step::Stay
            }
        }
    }
}
