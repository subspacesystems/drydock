// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `m13-update-check`. A fake transport serves signed manifests made with
//! a throwaway key (fixed seed; never a real release key).

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use drydock_update::{
    Channel, Config, Mode, State, Transport, TransportError, Updater, Version, hex_encode,
};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

const MANIFEST_URL: &str = "https://example.invalid/channel/latest.txt";
const SIG_URL: &str = "https://example.invalid/channel/latest.txt.sig";

fn key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn installer_url(version: &str) -> String {
    format!("https://example.invalid/dl/drydock-{version}-setup.exe")
}

/// A signed release: manifest bytes, signature bytes and the installer bytes the manifest describes.
struct Release {
    manifest: Vec<u8>,
    signature: Vec<u8>,
    installer: Vec<u8>,
}

fn release_with(seed: u8, version: &str, installer: Vec<u8>, claimed: &[u8]) -> Release {
    let manifest = format!(
        "version = {version}\nnotes_url = https://example.invalid/notes/{version}\ninstaller_url = {}\nsize = {}\nsha256 = {}\n",
        installer_url(version),
        claimed.len(),
        hex_encode(&Sha256::digest(claimed)),
    )
    .into_bytes();
    let signature = key(seed).sign(&manifest).to_bytes().to_vec();
    Release {
        manifest,
        signature,
        installer,
    }
}

/// Installer bytes 40 bytes long, distinct per version.
fn release(version: &str) -> Release {
    let data: Vec<u8> = (0..40u8)
        .map(|i| i.wrapping_mul(7) ^ version.len() as u8 ^ version.bytes().last().unwrap())
        .collect();
    release_with(7, version, data.clone(), &data)
}

/// (url, headers, max_bytes)
type GetCall = (String, Vec<(String, String)>, u64);

#[derive(Default)]
struct Fake {
    gets: Mutex<HashMap<String, Result<Vec<u8>, TransportError>>>,
    files: Mutex<HashMap<String, Vec<u8>>>,
    offline: AtomicBool,
    /// Download fails with `Other` after two chunks.
    break_download: AtomicBool,
    after_first_chunk: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    get_calls: Mutex<Vec<GetCall>>,
    download_calls: AtomicUsize,
}

impl Fake {
    fn serve(&self, rel: &Release) {
        let mut g = self.gets.lock().unwrap();
        g.insert(MANIFEST_URL.into(), Ok(rel.manifest.clone()));
        g.insert(SIG_URL.into(), Ok(rel.signature.clone()));
        let text = String::from_utf8(rel.manifest.clone()).unwrap();
        let url = text
            .lines()
            .find_map(|l| l.strip_prefix("installer_url = "))
            .unwrap()
            .to_string();
        self.files
            .lock()
            .unwrap()
            .insert(url, rel.installer.clone());
    }
    fn total_calls(&self) -> usize {
        self.get_calls.lock().unwrap().len() + self.download_calls.load(Ordering::SeqCst)
    }
}

impl Transport for Fake {
    fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
        max_bytes: u64,
    ) -> Result<Vec<u8>, TransportError> {
        self.get_calls
            .lock()
            .unwrap()
            .push((url.to_string(), headers.to_vec(), max_bytes));
        if self.offline.load(Ordering::SeqCst) {
            return Err(TransportError::Offline);
        }
        match self.gets.lock().unwrap().get(url) {
            Some(r) => r.clone(),
            None => Err(TransportError::Http(404)),
        }
    }

    fn download(
        &self,
        url: &str,
        _headers: &[(String, String)],
        max_bytes: u64,
        dest: &Path,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), TransportError> {
        self.download_calls.fetch_add(1, Ordering::SeqCst);
        if self.offline.load(Ordering::SeqCst) {
            return Err(TransportError::Offline);
        }
        let data = self
            .files
            .lock()
            .unwrap()
            .get(url)
            .cloned()
            .ok_or(TransportError::Http(404))?;
        if data.len() as u64 > max_bytes {
            return Err(TransportError::TooLarge);
        }
        let mut file = std::fs::File::create(dest).map_err(|e| TransportError::Io(e.kind()))?;
        let mut written = 0u64;
        for (i, chunk) in data.chunks(data.len() / 4 + 1).enumerate() {
            file.write_all(chunk)
                .map_err(|e| TransportError::Io(e.kind()))?;
            written += chunk.len() as u64;
            progress(written);
            if i == 0
                && let Some(hook) = self.after_first_chunk.lock().unwrap().as_ref()
            {
                hook();
            }
            if cancel.load(Ordering::SeqCst) {
                return Err(TransportError::Cancelled);
            }
            if i == 1 && self.break_download.load(Ordering::SeqCst) {
                return Err(TransportError::Other("connection reset".into()));
            }
        }
        Ok(())
    }
}

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

/// A fresh, not yet created, download folder.
fn fresh_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "drydock-update-check-{}-{}",
        std::process::id(),
        NEXT_DIR.fetch_add(1, Ordering::SeqCst)
    ))
}

fn config(dir: &Path, mode: Mode) -> Config {
    Config {
        channel: Channel {
            name: "test".into(),
            manifest_url: MANIFEST_URL.into(),
            public_key: key(7).verifying_key().to_bytes(),
            headers: vec![("X-Channel".into(), "one".into())],
        },
        running: Version::parse("1.0.0").unwrap(),
        mode,
        download_dir: dir.to_path_buf(),
        startup_delay: Duration::from_millis(10),
        interval: Duration::from_secs(3600),
    }
}

fn setup(mode: Mode, rel: Option<&Release>) -> (Arc<Fake>, Arc<Updater>, PathBuf) {
    let fake = Arc::new(Fake::default());
    if let Some(r) = rel {
        fake.serve(r);
    }
    let dir = fresh_dir();
    let up = Arc::new(Updater::new(config(&dir, mode), fake.clone()));
    (fake, up, dir)
}

/// Names of the files in `dir` (empty when it does not exist).
fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn wait_for(up: &Updater, what: &str, ok: impl Fn(&State) -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        if ok(&up.state()) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out waiting for {what}; state is {:?}", up.state());
}

fn is_failed(s: &State) -> bool {
    matches!(s, State::Failed(r) if !r.is_empty())
}

// ---- check: decisions ----

#[test]
fn starts_idle() {
    let (fake, up, dir) = setup(Mode::Auto, None);
    assert_eq!(up.state(), State::Idle);
    assert_eq!(up.mode(), Mode::Auto);
    assert_eq!(fake.total_calls(), 0);
    assert!(names(&dir).is_empty());
}

#[test]
fn newer_version_is_available() {
    let rel = release("1.4.2");
    let (_f, up, _d) = setup(Mode::Auto, Some(&rel));
    up.check(true);
    match up.state() {
        State::Available(m) => {
            assert_eq!(m.version, Version::parse("1.4.2").unwrap());
            assert_eq!(m.size, 40);
            assert_eq!(m.installer_url, installer_url("1.4.2"));
        }
        s => panic!("expected Available, got {s:?}"),
    }
}

#[test]
fn same_version_is_up_to_date() {
    let rel = release("1.0.0");
    let (_f, up, _d) = setup(Mode::Auto, Some(&rel));
    up.check(true);
    assert_eq!(up.state(), State::UpToDate);
}

#[test]
fn older_version_is_up_to_date() {
    let rel = release("0.9.9");
    let (_f, up, _d) = setup(Mode::Auto, Some(&rel));
    up.check(false);
    assert_eq!(up.state(), State::UpToDate);
}

#[test]
fn request_carries_only_channel_headers_and_plain_urls() {
    let rel = release("1.4.2");
    let (fake, up, _d) = setup(Mode::Auto, Some(&rel));
    up.check(false);
    let calls = fake.get_calls.lock().unwrap().clone();
    let urls: Vec<&str> = calls.iter().map(|c| c.0.as_str()).collect();
    assert_eq!(urls, [MANIFEST_URL, SIG_URL]);
    for (url, headers, max) in &calls {
        assert!(!url.contains('?'), "no query string: {url}");
        assert_eq!(headers, &[("X-Channel".to_string(), "one".to_string())]);
        assert!(*max >= 64 && *max <= 1 << 20, "sensible size limit: {max}");
    }
    // The manifest's limit leaves room for a real manifest; the signature's is small.
    assert!(calls[0].2 >= 1024);
    assert!(calls[1].2 >= 64 && calls[1].2 <= 4096);
}

// ---- check: failures are quiet ----

#[test]
fn offline_automatic_check_stays_idle_and_quiet() {
    let (fake, up, dir) = setup(Mode::Auto, None);
    fake.offline.store(true, Ordering::SeqCst);
    up.check(false);
    assert_eq!(up.state(), State::Idle);
    assert!(names(&dir).is_empty());
}

#[test]
fn offline_manual_check_reports_a_reason() {
    let (fake, up, _d) = setup(Mode::Auto, None);
    fake.offline.store(true, Ordering::SeqCst);
    up.check(true);
    assert!(is_failed(&up.state()), "{:?}", up.state());
}

#[test]
fn bad_signature_is_rejected() {
    let mut rel = release("1.4.2");
    rel.signature = key(8).sign(&rel.manifest).to_bytes().to_vec(); // another key
    let (_f, up, _d) = setup(Mode::Auto, Some(&rel));
    up.check(true);
    assert!(is_failed(&up.state()), "{:?}", up.state());
    up.check(false);
    assert_eq!(up.state(), State::Idle, "automatic: quiet");
}

#[test]
fn manifest_changed_after_signing_is_rejected() {
    let mut rel = release("1.4.2");
    rel.manifest.extend_from_slice(b"\n");
    let (_f, up, _d) = setup(Mode::Auto, Some(&rel));
    up.check(true);
    assert!(is_failed(&up.state()), "{:?}", up.state());
}

#[test]
fn unsigned_manifest_is_rejected() {
    // Signature file missing (404), empty and truncated.
    for sig in [None, Some(vec![]), Some(vec![1u8; 63])] {
        let rel = release("1.4.2");
        let (fake, up, _d) = setup(Mode::Auto, Some(&rel));
        match sig {
            None => {
                fake.gets.lock().unwrap().remove(SIG_URL);
            }
            Some(s) => {
                fake.gets.lock().unwrap().insert(SIG_URL.into(), Ok(s));
            }
        }
        up.check(true);
        assert!(is_failed(&up.state()), "{:?}", up.state());
        up.check(false);
        assert_eq!(up.state(), State::Idle);
    }
}

#[test]
fn garbage_manifest_is_rejected() {
    let rel = release_with(7, "1.4.2", vec![1; 40], &[1; 40]);
    let (fake, up, _d) = setup(Mode::Auto, Some(&rel));
    let junk = b"not a manifest\n".to_vec();
    let sig = key(7).sign(&junk).to_bytes().to_vec();
    {
        let mut g = fake.gets.lock().unwrap();
        g.insert(MANIFEST_URL.into(), Ok(junk));
        g.insert(SIG_URL.into(), Ok(sig));
    }
    up.check(true);
    assert!(is_failed(&up.state()), "{:?}", up.state());
}

#[test]
fn failed_manual_check_recovers_on_the_next_good_one() {
    let rel = release("1.0.0");
    let (fake, up, _d) = setup(Mode::Auto, Some(&rel));
    fake.offline.store(true, Ordering::SeqCst);
    up.check(true);
    assert!(is_failed(&up.state()));
    fake.offline.store(false, Ordering::SeqCst);
    up.check(true);
    assert_eq!(up.state(), State::UpToDate);
}

#[test]
fn automatic_failure_keeps_the_earlier_state() {
    let rel = release("1.4.2");
    let (fake, up, _d) = setup(Mode::Auto, Some(&rel));
    up.check(false);
    let before = up.state();
    assert!(matches!(before, State::Available(_)));
    fake.offline.store(true, Ordering::SeqCst);
    up.check(false);
    assert_eq!(up.state(), before);
}

// ---- modes ----

#[test]
fn off_automatic_check_and_scheduler_never_touch_the_transport() {
    let rel = release("1.4.2");
    let (fake, up, dir) = setup(Mode::Off, Some(&rel));
    up.check(false);
    assert_eq!(up.state(), State::Idle);
    let bg = up.spawn();
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(fake.total_calls(), 0);
    assert_eq!(up.state(), State::Idle);
    drop(bg);
    assert_eq!(fake.total_calls(), 0);
    assert!(names(&dir).is_empty());
}

#[test]
fn manual_check_works_when_off() {
    let rel = release("1.4.2");
    let (_f, up, _d) = setup(Mode::Off, Some(&rel));
    up.check(true);
    assert!(matches!(up.state(), State::Available(_)));
    assert_eq!(up.mode(), Mode::Off);
}

#[test]
fn set_mode_changes_what_the_scheduler_does() {
    let rel = release("1.4.2");
    let (fake, up, _d) = setup(Mode::Off, Some(&rel));
    let bg = up.spawn();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(fake.total_calls(), 0);
    up.set_mode(Mode::NotifyOnly);
    assert_eq!(up.mode(), Mode::NotifyOnly);
    bg.check_now();
    wait_for(&up, "Available", |s| matches!(s, State::Available(_)));
    drop(bg);
}

#[test]
fn auto_mode_checks_then_downloads_to_ready() {
    let rel = release("1.4.2");
    let (_f, up, dir) = setup(Mode::Auto, Some(&rel));
    let bg = up.spawn();
    wait_for(&up, "Ready", |s| matches!(s, State::Ready { .. }));
    match up.state() {
        State::Ready { path, manifest } => {
            assert_eq!(manifest.version, Version::parse("1.4.2").unwrap());
            assert!(path.starts_with(&dir));
            assert_eq!(std::fs::read(&path).unwrap(), rel.installer);
        }
        s => panic!("{s:?}"),
    }
    assert_eq!(
        names(&dir).len(),
        1,
        "only the installer: {:?}",
        names(&dir)
    );
    drop(bg);
}

#[test]
fn notify_only_reports_and_waits_for_the_user() {
    let rel = release("1.4.2");
    let (fake, up, dir) = setup(Mode::NotifyOnly, Some(&rel));
    let bg = up.spawn();
    wait_for(&up, "Available", |s| matches!(s, State::Available(_)));
    std::thread::sleep(Duration::from_millis(150));
    assert!(matches!(up.state(), State::Available(_)));
    assert_eq!(fake.download_calls.load(Ordering::SeqCst), 0);
    assert!(names(&dir).is_empty());
    bg.download_now();
    wait_for(&up, "Ready", |s| matches!(s, State::Ready { .. }));
    assert_eq!(fake.download_calls.load(Ordering::SeqCst), 1);
    drop(bg);
}

#[test]
fn scheduler_repeats_at_the_interval() {
    let rel = release("1.0.0");
    let fake = Arc::new(Fake::default());
    fake.serve(&rel);
    let dir = fresh_dir();
    let mut cfg = config(&dir, Mode::Auto);
    cfg.interval = Duration::from_millis(60);
    let up = Arc::new(Updater::new(cfg, fake.clone()));
    let bg = up.spawn();
    let end = Instant::now() + Duration::from_secs(10);
    while fake.get_calls.lock().unwrap().len() < 6 && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(fake.get_calls.lock().unwrap().len() >= 6, "three checks");
    drop(bg);
}

#[test]
fn dropping_background_stops_the_schedule() {
    let rel = release("1.0.0");
    let fake = Arc::new(Fake::default());
    fake.serve(&rel);
    let dir = fresh_dir();
    let mut cfg = config(&dir, Mode::Auto);
    cfg.interval = Duration::from_millis(30);
    let up = Arc::new(Updater::new(cfg, fake.clone()));
    let bg = up.spawn();
    wait_for(&up, "UpToDate", |s| *s == State::UpToDate);
    drop(bg);
    let n = fake.total_calls();
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(fake.total_calls(), n);
}

#[test]
fn startup_delay_is_respected() {
    let rel = release("1.0.0");
    let fake = Arc::new(Fake::default());
    fake.serve(&rel);
    let dir = fresh_dir();
    let mut cfg = config(&dir, Mode::Auto);
    cfg.startup_delay = Duration::from_millis(400);
    let up = Arc::new(Updater::new(cfg, fake.clone()));
    let bg = up.spawn();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(fake.total_calls(), 0, "nothing before the delay");
    wait_for(&up, "UpToDate", |s| *s == State::UpToDate);
    drop(bg);
}

#[test]
fn manual_check_on_the_thread_skips_the_delay() {
    let rel = release("1.4.2");
    let fake = Arc::new(Fake::default());
    fake.serve(&rel);
    let dir = fresh_dir();
    let mut cfg = config(&dir, Mode::NotifyOnly);
    cfg.startup_delay = Duration::from_secs(3600);
    let up = Arc::new(Updater::new(cfg, fake.clone()));
    let bg = up.spawn();
    bg.check_now();
    wait_for(&up, "Available", |s| matches!(s, State::Available(_)));
    drop(bg);
}

// ---- download ----

#[test]
fn download_without_an_available_update_does_nothing() {
    let rel = release("1.4.2");
    let (fake, up, dir) = setup(Mode::Auto, Some(&rel));
    up.download(true);
    assert_eq!(up.state(), State::Idle);
    assert_eq!(fake.total_calls(), 0);
    assert!(names(&dir).is_empty());
}

#[test]
fn download_creates_the_folder_and_verifies() {
    let rel = release("1.4.2");
    let (fake, up, dir) = setup(Mode::NotifyOnly, Some(&rel));
    assert!(!dir.exists());
    up.check(false);
    up.download(true);
    let State::Ready { path, manifest } = up.state() else {
        panic!("{:?}", up.state());
    };
    assert_eq!(manifest.version, Version::parse("1.4.2").unwrap());
    assert_eq!(path.parent().unwrap(), dir);
    assert_eq!(path.file_name().unwrap(), "drydock-1.4.2-setup.exe");
    assert_eq!(std::fs::read(&path).unwrap(), rel.installer);
    assert_eq!(names(&dir), ["drydock-1.4.2-setup.exe"]);
    assert_eq!(fake.download_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn progress_is_visible_while_downloading() {
    let rel = release("1.4.2");
    let (fake, up, _d) = setup(Mode::NotifyOnly, Some(&rel));
    up.check(false);
    let seen = Arc::new(Mutex::new(None));
    let (up2, seen2) = (up.clone(), seen.clone());
    *fake.after_first_chunk.lock().unwrap() = Some(Box::new(move || {
        *seen2.lock().unwrap() = Some(up2.state());
    }));
    up.download(true);
    match seen.lock().unwrap().clone() {
        Some(State::Downloading { manifest, received }) => {
            assert_eq!(manifest.size, 40);
            assert!(received > 0 && received < 40, "received {received}");
        }
        s => panic!("expected Downloading, got {s:?}"),
    }
    assert!(matches!(up.state(), State::Ready { .. }));
}

#[test]
fn hash_mismatch_deletes_the_file() {
    // The manifest describes different bytes than the server sends (same length).
    let good = vec![5u8; 40];
    let rel = release_with(7, "1.4.2", vec![6u8; 40], &good);
    let (_f, up, dir) = setup(Mode::NotifyOnly, Some(&rel));
    up.check(false);
    up.download(true);
    assert!(is_failed(&up.state()), "{:?}", up.state());
    assert!(names(&dir).is_empty(), "left behind: {:?}", names(&dir));
}

#[test]
fn size_mismatch_deletes_the_file() {
    let rel = release_with(7, "1.4.2", vec![5u8; 39], &[5u8; 40]);
    let (_f, up, dir) = setup(Mode::NotifyOnly, Some(&rel));
    up.check(false);
    up.download(true);
    assert!(is_failed(&up.state()), "{:?}", up.state());
    assert!(names(&dir).is_empty(), "left behind: {:?}", names(&dir));
}

#[test]
fn automatic_download_failure_is_quiet() {
    let rel = release_with(7, "1.4.2", vec![6u8; 40], &[5u8; 40]);
    let (_f, up, dir) = setup(Mode::Auto, Some(&rel));
    up.check(false);
    up.download(false);
    assert_eq!(up.state(), State::Idle);
    assert!(names(&dir).is_empty());
}

#[test]
fn transport_error_mid_download_deletes_the_partial_file() {
    let rel = release("1.4.2");
    let (fake, up, dir) = setup(Mode::NotifyOnly, Some(&rel));
    up.check(false);
    fake.break_download.store(true, Ordering::SeqCst);
    up.download(true);
    assert!(is_failed(&up.state()), "{:?}", up.state());
    assert!(names(&dir).is_empty(), "left behind: {:?}", names(&dir));
}

#[test]
fn cancel_mid_download_deletes_the_partial_file_and_stays_available() {
    let rel = release("1.4.2");
    let (fake, up, dir) = setup(Mode::NotifyOnly, Some(&rel));
    up.check(false);
    let up2 = up.clone();
    *fake.after_first_chunk.lock().unwrap() = Some(Box::new(move || up2.cancel()));
    up.download(true);
    assert!(
        matches!(up.state(), State::Available(_)),
        "cancelling is not a failure: {:?}",
        up.state()
    );
    assert!(names(&dir).is_empty(), "left behind: {:?}", names(&dir));
    // A later download works: the cancel flag does not stick.
    *fake.after_first_chunk.lock().unwrap() = None;
    up.download(true);
    assert!(matches!(up.state(), State::Ready { .. }));
}

#[test]
fn cancel_when_idle_is_harmless() {
    let rel = release("1.4.2");
    let (_f, up, _d) = setup(Mode::NotifyOnly, Some(&rel));
    up.cancel();
    up.check(false);
    up.download(true);
    assert!(matches!(up.state(), State::Ready { .. }));
}

#[test]
fn older_downloads_are_cleaned_up() {
    let rel = release("1.4.2");
    let (_f, up, dir) = setup(Mode::NotifyOnly, Some(&rel));
    std::fs::create_dir_all(&dir).unwrap();
    for n in [
        "drydock-0.9.0-setup.exe",
        "drydock-1.3.0-setup.exe.part",
        "keep.txt",
    ] {
        std::fs::write(dir.join(n), b"old").unwrap();
    }
    up.check(false);
    up.download(true);
    assert_eq!(names(&dir), ["drydock-1.4.2-setup.exe", "keep.txt"]);
}

#[test]
fn a_verified_file_from_an_earlier_run_is_reused() {
    let rel = release("1.4.2");
    let (fake, up, dir) = setup(Mode::NotifyOnly, Some(&rel));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("drydock-1.4.2-setup.exe"), &rel.installer).unwrap();
    up.check(false);
    up.download(false);
    assert!(matches!(up.state(), State::Ready { .. }));
    assert_eq!(fake.download_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn a_damaged_file_from_an_earlier_run_is_replaced() {
    let rel = release("1.4.2");
    let (fake, up, dir) = setup(Mode::NotifyOnly, Some(&rel));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("drydock-1.4.2-setup.exe"), b"truncat").unwrap();
    up.check(false);
    up.download(false);
    let State::Ready { path, .. } = up.state() else {
        panic!("{:?}", up.state());
    };
    assert_eq!(std::fs::read(path).unwrap(), rel.installer);
    assert_eq!(fake.download_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn recheck_keeps_ready_and_a_newer_release_replaces_it() {
    let rel = release("1.4.2");
    let (fake, up, dir) = setup(Mode::Auto, Some(&rel));
    up.check(false);
    up.download(false);
    let ready = up.state();
    assert!(matches!(ready, State::Ready { .. }));
    up.check(false);
    assert_eq!(up.state(), ready, "same release: still Ready");
    assert_eq!(names(&dir), ["drydock-1.4.2-setup.exe"]);
    // 1.5.0 is published.
    fake.serve(&release("1.5.0"));
    up.check(false);
    assert!(
        matches!(up.state(), State::Available(m) if m.version == Version::parse("1.5.0").unwrap())
    );
    up.download(false);
    assert_eq!(names(&dir), ["drydock-1.5.0-setup.exe"]);
}

#[test]
fn up_to_date_check_removes_stale_installers() {
    // The app was updated to 1.0.0; the earlier download of 0.9.0 is no longer needed.
    let rel = release("1.0.0");
    let (_f, up, dir) = setup(Mode::Auto, Some(&rel));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("drydock-1.0.0-setup.exe"), b"x").unwrap();
    std::fs::write(dir.join("drydock-0.9.0-setup.exe"), b"x").unwrap();
    std::fs::write(dir.join("keep.txt"), b"x").unwrap();
    up.check(false);
    assert_eq!(up.state(), State::UpToDate);
    assert_eq!(names(&dir), ["keep.txt"]);
}

#[test]
fn failed_check_leaves_files_alone() {
    let (fake, up, dir) = setup(Mode::Auto, None);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("drydock-0.9.0-setup.exe"), b"x").unwrap();
    fake.offline.store(true, Ordering::SeqCst);
    up.check(false);
    assert_eq!(names(&dir), ["drydock-0.9.0-setup.exe"]);
}

#[test]
fn download_never_exceeds_the_manifest_size() {
    let rel = release("1.4.2");
    let (fake, up, dir) = setup(Mode::NotifyOnly, Some(&rel));
    up.check(false);
    // The server now has a bigger file than the manifest says: the fake enforces max_bytes.
    fake.files
        .lock()
        .unwrap()
        .insert(installer_url("1.4.2"), vec![0u8; 400]);
    up.download(true);
    assert!(is_failed(&up.state()), "{:?}", up.state());
    assert!(names(&dir).is_empty());
}
