// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `m13-update-ui` (`docs/specs/m13-update-ui.md`). A fake transport serves
//! manifests signed with a throwaway key (fixed seed; never a real release key); the launcher
//! records instead of starting anything. Every test forces `supported()` on first.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use drydock::chrome::{MenuState, menu_bar};
use drydock::input::Action;
use drydock::prompts::UnsavedChoice;
use drydock::update_ui::{
    BUILT_IN_URL, INSTALLER_ARGS, Launcher, Parts, ProcessLauncher, Step, UpdateEvent,
    UpdateSetting, UpdateUi, built_in_channel, channel_file, download_dir, force_supported,
    parse_key_file, resolve_channel, settings_file, supported,
};
use drydock_update::{Channel, Mode, State, Transport, TransportError, Version, hex_encode};
use ed25519_dalek::{Signer, SigningKey};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use sha2::{Digest, Sha256};

const MANIFEST_URL: &str = "https://example.invalid/channel/latest.txt";
const SIG_URL: &str = "https://example.invalid/channel/latest.txt.sig";
const NEW_VERSION: &str = "9.9.9";

fn key() -> SigningKey {
    SigningKey::from_bytes(&[7; 32])
}

fn channel() -> Channel {
    Channel {
        name: "test".into(),
        manifest_url: MANIFEST_URL.into(),
        public_key: key().verifying_key().to_bytes(),
        headers: Vec::new(),
    }
}

const INSTALLER: &[u8] = b"pretend installer bytes, forty bytes....";

/// Serves a signed manifest for `NEW_VERSION` (or `old` when `newer` is false).
struct Fake {
    gets: Mutex<HashMap<String, Vec<u8>>>,
    offline: AtomicBool,
    calls: AtomicUsize,
}

impl Fake {
    fn new(version: &str) -> Arc<Fake> {
        let manifest = format!(
            "version = {version}\nnotes_url = https://example.invalid/notes/{version}\ninstaller_url = https://example.invalid/dl/drydock-{version}-setup.exe\nsize = {}\nsha256 = {}\n",
            INSTALLER.len(),
            hex_encode(&Sha256::digest(INSTALLER)),
        )
        .into_bytes();
        let signature = key().sign(&manifest).to_bytes().to_vec();
        let fake = Fake {
            gets: Mutex::new(HashMap::new()),
            offline: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
        };
        fake.gets
            .lock()
            .unwrap()
            .insert(MANIFEST_URL.into(), manifest);
        fake.gets.lock().unwrap().insert(SIG_URL.into(), signature);
        Arc::new(fake)
    }

    fn offline() -> Arc<Fake> {
        let fake = Fake::new(NEW_VERSION);
        fake.offline.store(true, Ordering::SeqCst);
        fake
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Transport for Fake {
    fn get(
        &self,
        url: &str,
        _headers: &[(String, String)],
        _max: u64,
    ) -> Result<Vec<u8>, TransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.offline.load(Ordering::SeqCst) {
            return Err(TransportError::Offline);
        }
        self.gets
            .lock()
            .unwrap()
            .get(url)
            .cloned()
            .ok_or(TransportError::Http(404))
    }

    fn download(
        &self,
        _url: &str,
        _headers: &[(String, String)],
        _max: u64,
        dest: &Path,
        _cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), TransportError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.offline.load(Ordering::SeqCst) {
            return Err(TransportError::Offline);
        }
        std::fs::write(dest, INSTALLER).map_err(|e| TransportError::Io(e.kind()))?;
        progress(INSTALLER.len() as u64);
        Ok(())
    }
}

#[derive(Clone, Default)]
struct Rec {
    launched: Arc<Mutex<Vec<PathBuf>>>,
    fail: Arc<AtomicBool>,
}

impl Launcher for Rec {
    fn launch(&mut self, installer: &Path) -> Result<(), String> {
        if self.fail.load(Ordering::SeqCst) {
            return Err("access denied".into());
        }
        self.launched.lock().unwrap().push(installer.to_path_buf());
        Ok(())
    }
}

fn fresh_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("drydock-update-ui-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Rig {
    ui: UpdateUi,
    fake: Arc<Fake>,
    rec: Rec,
    dir: PathBuf,
}

fn rig_with(
    name: &str,
    fake: Arc<Fake>,
    setting: UpdateSetting,
    delay: Duration,
    with_channel: bool,
) -> Rig {
    force_supported(true);
    let dir = fresh_dir(name);
    let rec = Rec::default();
    let ui = UpdateUi::new(Parts {
        channel: with_channel.then(channel),
        transport: fake.clone(),
        launcher: Box::new(rec.clone()),
        settings_file: Some(dir.join("updates")),
        download_dir: dir.join("dl"),
        running: Version::parse("0.1.0").unwrap(),
        setting,
        startup_delay: delay,
        interval: Duration::from_secs(86_400),
    });
    Rig { ui, fake, rec, dir }
}

/// Nothing happens by itself: the first automatic check is a day away.
fn rig(name: &str, setting: UpdateSetting) -> Rig {
    rig_with(
        name,
        Fake::new(NEW_VERSION),
        setting,
        Duration::from_secs(86_400),
        true,
    )
}

struct St {
    ui: UpdateUi,
    events: Vec<UpdateEvent>,
}

fn harness(ui: UpdateUi) -> Harness<'static, St> {
    Harness::builder()
        .with_size(eframe::egui::vec2(800.0, 600.0))
        .build_ui_state(
            |ui, st: &mut St| {
                if let Some(event) = st.ui.show(ui.ctx()) {
                    st.events.push(event);
                }
            },
            St {
                ui,
                events: Vec::new(),
            },
        )
}

fn wait(h: &mut Harness<'_, St>, what: &str, cond: impl Fn(&UpdateUi) -> bool) {
    for _ in 0..600 {
        h.step();
        if cond(&h.state().ui) {
            h.run();
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("timed out waiting for {what}");
}

fn is_ready(ui: &UpdateUi) -> bool {
    matches!(ui.state(), Some(State::Ready { .. }))
}

fn has(h: &Harness<'_, St>, text: &str) -> bool {
    h.query_all_by_label_contains(text).next().is_some()
}

/// A Ready updater with the prompt showing.
fn ready(name: &str) -> (Harness<'static, St>, Fake2) {
    let r = rig(name, UpdateSetting::Auto);
    let Rig { ui, fake, rec, dir } = r;
    let mut h = harness(ui);
    h.state_mut().ui.open_dialog();
    wait(&mut h, "ready", is_ready);
    (
        h,
        Fake2 {
            fake,
            rec,
            _dir: dir,
        },
    )
}

struct Fake2 {
    fake: Arc<Fake>,
    rec: Rec,
    _dir: PathBuf,
}

// ---- setting ----

#[test]
fn setting_defaults_to_check_automatically_and_maps_to_modes() {
    assert_eq!(UpdateSetting::default(), UpdateSetting::Auto);
    assert_eq!(
        UpdateSetting::ALL.map(UpdateSetting::label),
        ["Check automatically", "Notify only", "Off"]
    );
    assert_eq!(UpdateSetting::Auto.mode(), Mode::Auto);
    assert_eq!(UpdateSetting::NotifyOnly.mode(), Mode::NotifyOnly);
    assert_eq!(UpdateSetting::Off.mode(), Mode::Off);
}

#[test]
fn setting_text_round_trips_and_ignores_junk() {
    for s in UpdateSetting::ALL {
        assert_eq!(UpdateSetting::parse(&s.to_text()), s);
    }
    assert_eq!(UpdateSetting::Off.to_text(), "check off\n");
    assert_eq!(UpdateSetting::parse(""), UpdateSetting::Auto);
    assert_eq!(
        UpdateSetting::parse("check sometimes\nwhat\n"),
        UpdateSetting::Auto
    );
    assert_eq!(
        UpdateSetting::parse("check off\r\ncheck bogus\r\n"),
        UpdateSetting::Off
    );
    assert_eq!(
        UpdateSetting::parse("check off\ncheck notify\n"),
        UpdateSetting::NotifyOnly
    );
}

#[test]
fn setting_is_kept_in_a_file_and_survives_a_new_run() {
    let dir = fresh_dir("persist");
    let file = dir.join("sub").join("updates");
    assert_eq!(
        UpdateSetting::load(&file),
        UpdateSetting::Auto,
        "missing file"
    );
    UpdateSetting::NotifyOnly.save(&file).unwrap();
    assert_eq!(UpdateSetting::load(&file), UpdateSetting::NotifyOnly);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "check notify\n");
    let leftovers: Vec<_> = std::fs::read_dir(file.parent().unwrap()).unwrap().collect();
    assert_eq!(leftovers.len(), 1, "no temporary file left");
}

#[test]
fn choosing_in_the_window_saves_and_reaches_the_updater() {
    let r = rig("choose", UpdateSetting::Auto);
    let file = r.dir.join("updates");
    let mut h = harness(r.ui);
    h.state_mut().ui.open_dialog();
    h.run();
    h.get_by_label("Notify only").click();
    h.run();
    assert_eq!(h.state().ui.setting(), UpdateSetting::NotifyOnly);
    assert_eq!(UpdateSetting::load(&file), UpdateSetting::NotifyOnly);
    h.get_by_label("Off").click();
    h.run();
    assert_eq!(h.state().ui.setting(), UpdateSetting::Off);
    assert_eq!(UpdateSetting::load(&file), UpdateSetting::Off);
    // A new run starts from the saved choice.
    let again = rig_with(
        "choose",
        Fake::new(NEW_VERSION),
        UpdateSetting::load(&file),
        Duration::from_secs(86_400),
        true,
    );
    assert_eq!(again.ui.setting(), UpdateSetting::Off);
}

#[test]
fn files_live_in_the_config_folder() {
    let var = |name: &str| (name == "DRYDOCK_CONFIG_DIR").then(|| OsString::from("/cfg/d"));
    assert_eq!(settings_file(var), Some(PathBuf::from("/cfg/d/updates")));
    assert_eq!(
        channel_file(var),
        Some(PathBuf::from("/cfg/d/update-channel"))
    );
    assert_eq!(
        download_dir(var),
        Some(PathBuf::from("/cfg/d/update-downloads"))
    );
}

// ---- channel ----

const KEY_HEX: &str = "dbaa4aadfb200e6bcc75ffed7e65e68d974e1437fed70438a9d364e2baca82ad";

#[test]
fn built_in_channel_needs_an_https_url_and_a_good_key() {
    let c = built_in_channel(Some("https://example.invalid/m"), KEY_HEX).unwrap();
    assert_eq!(c.manifest_url, "https://example.invalid/m");
    assert!(c.headers.is_empty());
    assert_eq!(c.public_key[0], 0xdb);
    assert!(built_in_channel(None, KEY_HEX).is_none());
    assert!(built_in_channel(Some("http://example.invalid/m"), KEY_HEX).is_none());
    assert!(built_in_channel(Some("https://example.invalid/m"), "abcd").is_none());
}

#[test]
fn key_file_skips_comments() {
    let text = format!("# Throwaway key\n\n{KEY_HEX}\n");
    assert_eq!(parse_key_file(&text).unwrap()[31], 0xad);
    assert!(parse_key_file("# only a comment\n").is_none());
    assert!(parse_key_file("zz\n").is_none());
}

#[test]
fn channel_file_overrides_the_built_in_one_and_may_add_headers() {
    let built_in = built_in_channel(Some("https://built-in.invalid/m"), KEY_HEX);
    let file = format!(
        "name = private\nmanifest_url = https://private.invalid/m\npublic_key = {KEY_HEX}\nheader = Authorization: Bearer abc\n"
    );
    let c = resolve_channel(built_in.clone(), Some(file.as_bytes())).unwrap();
    assert_eq!(c.name, "private");
    assert_eq!(
        c.headers,
        vec![("Authorization".to_owned(), "Bearer abc".to_owned())]
    );
    assert_eq!(
        resolve_channel(built_in.clone(), Some(b"garbage")),
        built_in,
        "bad file ignored"
    );
    assert_eq!(resolve_channel(built_in.clone(), None), built_in);
    assert!(
        resolve_channel(None, None).is_none(),
        "no channel: no checking"
    );
    assert!(resolve_channel(None, Some(b"garbage")).is_none());
}

#[test]
fn ordinary_builds_have_no_built_in_url() {
    // No real URL is ever committed; builds get one from the environment of the release script.
    if std::env::var_os("DRYDOCK_UPDATE_URL").is_none() {
        assert!(BUILT_IN_URL.is_none());
    }
}

// ---- menu ----

fn menu(state: MenuState<'static>) -> (Harness<'static>, Arc<Mutex<Vec<Action>>>) {
    force_supported(true);
    let seen: Arc<Mutex<Vec<Action>>> = Arc::default();
    let sink = seen.clone();
    let h = Harness::builder()
        .with_size(eframe::egui::vec2(1000.0, 800.0))
        .build_ui(move |ui| {
            if let Some(action) = menu_bar(ui, &state) {
                sink.lock().unwrap().push(action);
            }
        });
    (h, seen)
}

#[test]
fn help_menu_has_check_for_updates_between_the_guide_and_about() {
    let (mut h, seen) = menu(MenuState {
        update_channel: true,
        ..MenuState::default()
    });
    h.run();
    h.get_by_label("Help").click();
    h.run();
    let guide = h.get_by_label("User Guide").rect().top();
    let check = h.get_by_label("Check for Updates...").rect().top();
    let about = h.get_by_label("About Drydock...").rect().top();
    assert!(guide < check && check < about);
    h.get_by_label("Check for Updates...").click();
    h.run();
    assert_eq!(*seen.lock().unwrap(), vec![Action::CheckForUpdates]);
}

#[test]
fn without_a_channel_the_item_is_greyed_and_says_so() {
    let (mut h, seen) = menu(MenuState::default());
    h.run();
    h.get_by_label("Help").click();
    h.run();
    assert!(h.query_by_label("Check for Updates...").is_none());
    h.get_by_label("Check for Updates... (not set up)").click();
    h.run();
    assert!(
        seen.lock().unwrap().is_empty(),
        "a greyed item does nothing"
    );
}

// ---- the Updates window ----

#[test]
fn window_shows_the_three_choices_and_the_current_one() {
    let r = rig("window", UpdateSetting::Off);
    let mut h = harness(r.ui);
    h.run();
    assert!(!has(&h, "Check automatically"), "closed until asked for");
    h.state_mut().ui.open_dialog();
    h.run();
    for label in ["Check automatically", "Notify only", "Off", "Check now"] {
        assert!(h.query_by_label(label).is_some(), "{label}");
    }
}

#[test]
fn manual_check_reports_up_to_date() {
    let r = rig_with(
        "uptodate",
        Fake::new("0.1.0"),
        UpdateSetting::Auto,
        Duration::from_secs(86_400),
        true,
    );
    let mut h = harness(r.ui);
    h.state_mut().ui.open_dialog();
    wait(&mut h, "up to date", |ui| {
        ui.state() == Some(State::UpToDate)
    });
    assert!(has(&h, "Drydock is up to date."));
    assert_eq!(
        h.state().ui.status_text().as_deref(),
        Some("Drydock is up to date.")
    );
    h.state_mut().ui.clear_status();
    assert!(h.state().ui.status_text().is_none());
}

#[test]
fn manual_check_offline_is_a_quiet_message() {
    let r = rig_with(
        "manual-offline",
        Fake::offline(),
        UpdateSetting::Auto,
        Duration::from_secs(86_400),
        true,
    );
    let mut h = harness(r.ui);
    h.state_mut().ui.open_dialog();
    wait(&mut h, "failure", |ui| {
        matches!(ui.state(), Some(State::Failed(_)))
    });
    assert!(has(&h, "Couldn't reach the update server."));
    assert_eq!(
        h.state().ui.status_text().as_deref(),
        Some("Couldn't reach the update server.")
    );
}

#[test]
fn check_now_button_checks_again() {
    let r = rig("check-button", UpdateSetting::NotifyOnly);
    let fake = r.fake.clone();
    let mut h = harness(r.ui);
    h.state_mut().ui.open_dialog();
    wait(&mut h, "available", |ui| {
        matches!(ui.state(), Some(State::Available(_)))
    });
    let before = fake.calls();
    h.get_by_label("Check now").click();
    h.run();
    wait(&mut h, "second check", |_| fake.calls() > before);
}

#[test]
fn notify_only_offers_the_update_and_downloads_when_asked() {
    let r = rig("notify", UpdateSetting::NotifyOnly);
    let mut h = harness(r.ui);
    h.state_mut().ui.open_dialog();
    wait(&mut h, "available", |ui| {
        matches!(ui.state(), Some(State::Available(_)))
    });
    assert!(has(&h, "Drydock 9.9.9 is available."));
    std::thread::sleep(Duration::from_millis(200));
    h.run();
    assert!(
        matches!(h.state().ui.state(), Some(State::Available(_))),
        "no download without being asked"
    );
    h.get_by_label("Download").click();
    h.run();
    wait(&mut h, "ready", is_ready);
}

#[test]
fn no_channel_means_no_checking_and_the_window_says_so() {
    let r = rig_with(
        "nochannel",
        Fake::new(NEW_VERSION),
        UpdateSetting::Auto,
        Duration::ZERO,
        false,
    );
    let fake = r.fake.clone();
    let mut ui = r.ui;
    assert!(!ui.has_channel());
    assert!(ui.state().is_none());
    ui.open_dialog();
    ui.check_now();
    let mut h = harness(ui);
    h.run();
    assert!(has(&h, "Updates are not set up in this build."));
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(fake.calls(), 0);
    assert!(h.state().ui.status_text().is_none());
}

// ---- schedule and mode ----

#[test]
fn auto_downloads_in_the_background_without_any_message() {
    let r = rig_with(
        "auto",
        Fake::new(NEW_VERSION),
        UpdateSetting::Auto,
        Duration::ZERO,
        true,
    );
    let mut h = harness(r.ui);
    wait(&mut h, "ready", is_ready);
    assert!(
        h.state().ui.status_text().is_none(),
        "automatic work shows no status text"
    );
    assert!(
        !has(&h, "Check automatically"),
        "the Updates window stays closed"
    );
}

#[test]
fn off_never_touches_the_network() {
    let r = rig_with(
        "off",
        Fake::new(NEW_VERSION),
        UpdateSetting::Off,
        Duration::ZERO,
        true,
    );
    let fake = r.fake.clone();
    let mut h = harness(r.ui);
    std::thread::sleep(Duration::from_millis(300));
    h.run();
    assert_eq!(fake.calls(), 0);
}

#[test]
fn automatic_failures_are_never_shown() {
    let r = rig_with(
        "auto-offline",
        Fake::offline(),
        UpdateSetting::Auto,
        Duration::ZERO,
        true,
    );
    let fake = r.fake.clone();
    let mut h = harness(r.ui);
    wait(&mut h, "an attempt", |_| fake.calls() > 0);
    std::thread::sleep(Duration::from_millis(100));
    h.run();
    assert!(h.state().ui.status_text().is_none());
    assert!(!has(&h, "reach"));
    assert!(!matches!(h.state().ui.state(), Some(State::Failed(_))));
}

#[test]
fn switching_to_off_stops_later_automatic_traffic() {
    let r = rig_with(
        "switch-off",
        Fake::new("0.1.0"),
        UpdateSetting::Auto,
        Duration::from_millis(400),
        true,
    );
    let fake = r.fake.clone();
    let mut ui = r.ui;
    ui.set_setting(UpdateSetting::Off);
    std::thread::sleep(Duration::from_millis(900));
    assert_eq!(fake.calls(), 0);
    let _ = &ui;
}

// ---- the ready prompt ----

#[test]
fn ready_prompt_offers_restart_and_later() {
    let (h, _f) = ready("prompt");
    assert!(h.state().ui.prompt_showing());
    assert!(has(&h, "Drydock 9.9.9 is ready to install"));
    assert!(h.query_by_label("Release notes").is_some());
    assert!(h.query_by_label("Restart and update").is_some());
    assert!(h.query_by_label("Later").is_some());
    assert!(h.state().events.is_empty());
}

#[test]
fn later_hides_the_prompt_and_starts_nothing() {
    let (mut h, f) = ready("later");
    h.get_by_label("Later").click();
    h.run();
    assert!(!h.state().ui.prompt_showing());
    assert!(!has(&h, "is ready to install"));
    assert!(f.rec.launched.lock().unwrap().is_empty());
    assert!(h.state().events.is_empty());
    let _ = &f.fake;
}

#[test]
fn restart_button_reports_the_event_and_launches_nothing_yet() {
    let (mut h, f) = ready("restart-click");
    h.get_by_label("Restart and update").click();
    h.run();
    assert_eq!(h.state().events, vec![UpdateEvent::Restart]);
    assert!(
        f.rec.launched.lock().unwrap().is_empty(),
        "the guard comes first"
    );
}

// ---- the unsaved-changes guard ----

fn ready_ui(name: &str) -> (UpdateUi, Fake2) {
    let (h, f) = ready(name);
    let St { ui, .. } = state_of(h);
    (ui, f)
}

fn state_of(h: Harness<'static, St>) -> St {
    // Take the state out of the harness by value.
    h.into_state()
}

fn launched(f: &Fake2) -> Vec<PathBuf> {
    f.rec.launched.lock().unwrap().clone()
}

#[test]
fn clean_document_starts_the_installer_and_quits() {
    let (mut ui, f) = ready_ui("guard-clean");
    assert_eq!(ui.begin_restart(false), Step::Quit);
    let calls = launched(&f);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].file_name().unwrap(), "drydock-9.9.9-setup.exe");
    assert!(calls[0].is_file(), "the downloaded installer");
    assert!(!ui.restart_pending());
}

#[test]
fn unsaved_changes_ask_first_and_cancel_keeps_the_app() {
    let (mut ui, f) = ready_ui("guard-cancel");
    assert_eq!(ui.begin_restart(true), Step::Ask);
    assert!(ui.restart_pending());
    assert!(launched(&f).is_empty());
    assert_eq!(ui.answer_unsaved(UnsavedChoice::Cancel), Step::Stay);
    assert!(!ui.restart_pending());
    assert!(launched(&f).is_empty(), "Cancel aborts the update");
    // The update is still there for later.
    assert!(matches!(ui.state(), Some(State::Ready { .. })));
}

#[test]
fn save_then_start_the_installer() {
    let (mut ui, f) = ready_ui("guard-save");
    assert_eq!(ui.begin_restart(true), Step::Ask);
    assert_eq!(ui.answer_unsaved(UnsavedChoice::Save), Step::Save);
    assert!(ui.restart_pending());
    assert!(launched(&f).is_empty(), "not before the save is done");
    assert_eq!(ui.after_saved(true), Step::Quit);
    assert_eq!(launched(&f).len(), 1);
    assert!(!ui.restart_pending());
}

#[test]
fn a_failed_or_cancelled_save_starts_nothing() {
    let (mut ui, f) = ready_ui("guard-save-fails");
    ui.begin_restart(true);
    ui.answer_unsaved(UnsavedChoice::Save);
    assert_eq!(ui.after_saved(false), Step::Stay);
    assert!(launched(&f).is_empty());
    ui.begin_restart(true);
    ui.answer_unsaved(UnsavedChoice::Save);
    ui.cancel_restart();
    assert!(!ui.restart_pending());
    assert!(launched(&f).is_empty());
}

#[test]
fn dont_save_starts_the_installer() {
    let (mut ui, f) = ready_ui("guard-discard");
    ui.begin_restart(true);
    assert_eq!(ui.answer_unsaved(UnsavedChoice::Discard), Step::Quit);
    assert_eq!(launched(&f).len(), 1);
}

#[test]
fn a_launcher_failure_keeps_the_app_and_says_why() {
    let (mut ui, f) = ready_ui("guard-launch-fails");
    f.rec.fail.store(true, Ordering::SeqCst);
    assert_eq!(ui.begin_restart(false), Step::Stay);
    let text = ui.status_text().unwrap();
    assert!(text.starts_with("Could not start the installer"), "{text}");
    assert!(text.contains("access denied"), "{text}");
}

#[test]
fn restart_without_a_ready_update_does_nothing() {
    let r = rig("guard-not-ready", UpdateSetting::Auto);
    let mut ui = r.ui;
    assert_eq!(ui.begin_restart(false), Step::Stay);
    assert_eq!(ui.begin_restart(true), Step::Stay);
    assert!(r.rec.launched.lock().unwrap().is_empty());
}

#[test]
fn the_real_launcher_passes_the_update_flags() {
    assert_eq!(INSTALLER_ARGS, ["/S", "/UPDATE"]);
    let mut launcher = ProcessLauncher;
    assert!(
        launcher
            .launch(Path::new("/nonexistent/drydock-0.0.0-setup.exe"))
            .is_err()
    );
}

#[test]
fn updating_is_supported_when_forced() {
    force_supported(true);
    assert!(supported());
}
