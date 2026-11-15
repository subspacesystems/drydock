// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::{Channel, Manifest, Transport, TransportError, Version, verify, verify_file_sha256};

/// The user's choice (the setting in `m13-update-ui`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Check at startup (after a delay) and then daily; download in the background.
    Auto,
    /// Check the same way and report Available; download only when asked ([`Updater::download`]).
    NotifyOnly,
    /// No network at all from the schedule. A manual check still works.
    Off,
}

/// What the UI shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Idle,
    Checking,
    UpToDate,
    Available(Manifest),
    Downloading {
        manifest: Manifest,
        received: u64,
    },
    /// The installer is downloaded and its size and SHA-256 match.
    Ready {
        path: PathBuf,
        manifest: Manifest,
    },
    /// Only after a manual action failed; the reason is a short non-empty sentence.
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct Config {
    pub channel: Channel,
    /// The running version (the app's own).
    pub running: Version,
    pub mode: Mode,
    /// Per-user folder for downloads; created when first needed.
    pub download_dir: PathBuf,
    /// Wait before the first automatic check.
    pub startup_delay: Duration,
    /// Wait between automatic checks (a day in the app).
    pub interval: Duration,
}

/// The updater's state machine. All methods take `&self`, so share it in an `Arc`. `check` and
/// `download` block for as long as the network takes: call them from a background thread
/// (see [`Updater::spawn`]), never from the UI thread. `state`, `cancel` and `set_mode` never block on I/O.
pub struct Updater {
    config: Config,
    transport: Arc<dyn Transport>,
    state: Mutex<State>,
    mode: Mutex<Mode>,
    cancel: Arc<AtomicBool>,
}

/// Keeps the schedule thread alive; dropping it stops the thread (and cancels a download in progress).
pub struct Background {
    tx: mpsc::Sender<Command>,
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

enum Command {
    Check,
    Download,
    Stop,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Updater {
    pub fn new(config: Config, transport: Arc<dyn Transport>) -> Updater {
        let mode = config.mode;
        Updater {
            config,
            transport,
            state: Mutex::new(State::Idle),
            mode: Mutex::new(mode),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn state(&self) -> State {
        lock(&self.state).clone()
    }

    pub fn mode(&self) -> Mode {
        *lock(&self.mode)
    }

    pub fn set_mode(&self, mode: Mode) {
        *lock(&self.mode) = mode;
    }

    /// Fetches and verifies the manifest, then decides. See the spec, "check".
    /// `manual` is true when the user asked.
    pub fn check(&self, manual: bool) {
        if !manual && self.mode() == Mode::Off {
            return;
        }
        let previous = {
            let mut state = lock(&self.state);
            if matches!(*state, State::Downloading { .. }) {
                return;
            }
            let previous = state.clone();
            *state = State::Checking;
            previous
        };
        let manifest = match self.fetch_manifest() {
            Ok(manifest) => manifest,
            Err(reason) => {
                eprintln!("drydock-update: {reason}");
                self.finish_failed_check(manual, previous, reason);
                return;
            }
        };
        let newer = manifest.version.is_newer_than(&self.config.running);
        let next = if newer {
            match &previous {
                State::Ready {
                    path,
                    manifest: ready_manifest,
                } if ready_manifest.version == manifest.version => State::Ready {
                    path: path.clone(),
                    manifest: ready_manifest.clone(),
                },
                _ => State::Available(manifest.clone()),
            }
        } else {
            State::UpToDate
        };
        self.cleanup_downloads(if newer { Some(&manifest.version) } else { None });
        *lock(&self.state) = next;
    }

    /// Downloads the installer of the Available manifest. See the spec, "download".
    /// `manual` is true when the user asked.
    pub fn download(&self, manual: bool) {
        let manifest = match &*lock(&self.state) {
            State::Available(manifest) => manifest.clone(),
            _ => return,
        };
        if let Err(error) = fs::create_dir_all(&self.config.download_dir) {
            self.finish_failed_download(
                manual,
                format!("download failed: cannot create folder: {error:?}"),
            );
            return;
        }
        let final_path = self
            .config
            .download_dir
            .join(format!("drydock-{}-setup.exe", manifest.version));
        if final_path.exists() {
            if verify_file_sha256(&final_path, &manifest.sha256, manifest.size).is_ok() {
                *lock(&self.state) = State::Ready {
                    path: final_path,
                    manifest,
                };
                return;
            }
            let _ = fs::remove_file(&final_path);
        }
        let mut part_name = final_path.as_os_str().to_owned();
        part_name.push(".part");
        let part_path = PathBuf::from(part_name);
        self.cancel.store(false, Ordering::SeqCst);
        *lock(&self.state) = State::Downloading {
            manifest: manifest.clone(),
            received: 0,
        };
        let result = {
            let mut progress = |received: u64| {
                let mut state = lock(&self.state);
                if let State::Downloading {
                    received: current, ..
                } = &mut *state
                {
                    *current = received;
                }
            };
            self.transport.download(
                &manifest.installer_url,
                &self.config.channel.headers,
                manifest.size,
                &part_path,
                self.cancel.as_ref(),
                &mut progress,
            )
        };
        let cancelled =
            self.cancel.load(Ordering::SeqCst) || matches!(result, Err(TransportError::Cancelled));
        if cancelled {
            let _ = fs::remove_file(&part_path);
            *lock(&self.state) = State::Available(manifest);
            return;
        }
        match result {
            Ok(()) => match verify_file_sha256(&part_path, &manifest.sha256, manifest.size) {
                Ok(()) => match fs::rename(&part_path, &final_path) {
                    Ok(()) => {
                        *lock(&self.state) = State::Ready {
                            path: final_path,
                            manifest,
                        };
                    }
                    Err(error) => {
                        let _ = fs::remove_file(&part_path);
                        let _ = fs::remove_file(&final_path);
                        self.finish_failed_download(
                            manual,
                            format!("download failed: cannot move into place: {error:?}"),
                        );
                    }
                },
                Err(error) => {
                    let _ = fs::remove_file(&part_path);
                    let _ = fs::remove_file(&final_path);
                    self.finish_failed_download(
                        manual,
                        format!("download failed: file does not match the manifest: {error:?}"),
                    );
                }
            },
            Err(error) => {
                let _ = fs::remove_file(&part_path);
                if final_path.exists()
                    && verify_file_sha256(&final_path, &manifest.sha256, manifest.size).is_err()
                {
                    let _ = fs::remove_file(&final_path);
                }
                self.finish_failed_download(manual, format!("download failed: {error:?}"));
            }
        }
    }

    /// Asks a running download to stop; safe from any thread and when nothing is downloading.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Starts the schedule thread. See the spec, "schedule".
    pub fn spawn(self: &Arc<Self>) -> Background {
        let (tx, rx) = mpsc::channel();
        let updater = Arc::clone(self);
        let cancel = Arc::clone(&self.cancel);
        let handle = thread::spawn(move || {
            let mut deadline = Instant::now() + updater.config.startup_delay;
            loop {
                let now = Instant::now();
                if now >= deadline {
                    let mode = updater.mode();
                    if mode != Mode::Off {
                        updater.check(false);
                    }
                    if updater.mode() == Mode::Auto
                        && matches!(updater.state(), State::Available(_))
                    {
                        updater.download(false);
                    }
                    deadline = Instant::now() + updater.config.interval;
                    continue;
                }
                match rx.recv_timeout(deadline - now) {
                    Ok(Command::Check) => updater.check(true),
                    Ok(Command::Download) => updater.download(true),
                    Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => {}
                }
            }
        });
        Background {
            tx,
            cancel,
            handle: Some(handle),
        }
    }

    fn fetch_manifest(&self) -> Result<Manifest, String> {
        let headers = &self.config.channel.headers;
        let bytes = self
            .transport
            .get(&self.config.channel.manifest_url, headers, 64 * 1024)
            .map_err(|error| format!("check failed: manifest request: {error:?}"))?;
        let signature_url = format!("{}.sig", self.config.channel.manifest_url);
        let signature = self
            .transport
            .get(&signature_url, headers, 4 * 1024)
            .map_err(|error| format!("check failed: signature request: {error:?}"))?;
        verify(&bytes, &signature, &self.config.channel.public_key)
            .map_err(|error| format!("check failed: {error:?}"))
    }

    fn finish_failed_check(&self, manual: bool, previous: State, reason: String) {
        let state = if manual {
            State::Failed(reason)
        } else {
            match previous {
                State::Failed(_) | State::Checking => State::Idle,
                other => other,
            }
        };
        *lock(&self.state) = state;
    }

    fn finish_failed_download(&self, manual: bool, reason: String) {
        eprintln!("drydock-update: {reason}");
        *lock(&self.state) = if manual {
            State::Failed(reason)
        } else {
            State::Idle
        };
    }

    fn cleanup_downloads(&self, keep_version: Option<&Version>) {
        let Ok(entries) = fs::read_dir(&self.config.download_dir) else {
            return;
        };
        let keep = keep_version.map(|version| format!("drydock-{version}-setup.exe"));
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let Some(name) = file_name.to_str() else {
                continue;
            };
            let base = name.strip_suffix(".part").unwrap_or(name);
            if !(name.starts_with("drydock-") && base.ends_with("-setup.exe")) {
                continue;
            }
            if let Some(keep) = &keep
                && (name == keep || name == format!("{keep}.part"))
            {
                continue;
            }
            let _ = fs::remove_file(entry.path());
        }
    }
}

impl Background {
    /// Runs a manual check on the schedule thread; returns at once.
    pub fn check_now(&self) {
        let _ = self.tx.send(Command::Check);
    }

    /// Runs a manual download of the Available manifest on the schedule thread; returns at once.
    pub fn download_now(&self) {
        let _ = self.tx.send(Command::Download);
    }
}

impl Drop for Background {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Stop);
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
