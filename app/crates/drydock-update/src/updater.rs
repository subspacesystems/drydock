// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::{Channel, Manifest, Transport, Version};

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
    _private: (),
}

/// Keeps the schedule thread alive; dropping it stops the thread (and cancels a download in progress).
pub struct Background {
    _private: (),
}

impl Updater {
    pub fn new(config: Config, transport: Arc<dyn Transport>) -> Updater {
        let _ = (config, transport);
        unimplemented!()
    }

    pub fn state(&self) -> State {
        unimplemented!()
    }

    pub fn mode(&self) -> Mode {
        unimplemented!()
    }

    pub fn set_mode(&self, mode: Mode) {
        let _ = mode;
        unimplemented!()
    }

    /// Fetches and verifies the manifest, then decides. See the spec, "check".
    /// `manual` is true when the user asked.
    pub fn check(&self, manual: bool) {
        let _ = manual;
        unimplemented!()
    }

    /// Downloads the installer of the Available manifest. See the spec, "download".
    /// `manual` is true when the user asked.
    pub fn download(&self, manual: bool) {
        let _ = manual;
        unimplemented!()
    }

    /// Asks a running download to stop; safe from any thread and when nothing is downloading.
    pub fn cancel(&self) {
        unimplemented!()
    }

    /// Starts the schedule thread. See the spec, "schedule".
    pub fn spawn(self: &Arc<Self>) -> Background {
        unimplemented!()
    }
}

impl Background {
    /// Runs a manual check on the schedule thread; returns at once.
    pub fn check_now(&self) {
        unimplemented!()
    }

    /// Runs a manual download of the Available manifest on the schedule thread; returns at once.
    pub fn download_now(&self) {
        unimplemented!()
    }
}

impl Drop for Background {
    fn drop(&mut self) {
        unimplemented!()
    }
}
