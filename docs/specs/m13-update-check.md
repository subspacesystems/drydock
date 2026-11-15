# m13-update-check: background updater with a transport trait

New in `drydock-update` (after `m13-update-verify`, which supplies `verify`, `verify_file_sha256`,
`Manifest`, `Channel`, `Version`). The skeleton is on main (`transport.rs`, `updater.rs`); do not
change signatures, variants or the test file `tests/update_check.rs`. No new dependencies
(std threads, `mpsc` and `Mutex` are enough). No telemetry, no platform code: Windows gets its
own transport in `m13-update-winhttp`.

## Transport

`Transport::get` returns a body (limit `max_bytes`); `Transport::download` streams into a file,
reports bytes written, and watches a cancel flag (see the trait docs). The updater passes the
channel's `headers` unchanged to every call and never adds a header, query string or ID.

## check(manual)

`manual` is true when the user asked. In every case, a failure is a short log line (`eprintln!`
with a `drydock-update:` prefix) and nothing more.

1. `Mode::Off` and not manual: return at once; the transport is not touched. A manual check
   works in every mode. If a download is in progress, return.
2. State becomes `Checking`. GET `channel.manifest_url` (limit 64 KiB), then GET the same URL with
   `.sig` appended (limit 4 KiB). The signature file is the 64 raw bytes. These are the only two
   requests; both carry the channel headers.
3. `verify(manifest_bytes, signature, &channel.public_key)`. A missing signature (404, empty
   body, wrong length), a bad one, or an invalid manifest is a failure.
4. `manifest.version.is_newer_than(&running)`: if not, the state is `UpToDate`; if so, `Available(manifest)`.
   Exception: when the state before the check was `Ready` for exactly this version, it stays
   `Ready` (same path, same manifest).
5. After a successful check (either outcome) clean up the download folder: delete every file
   named `drydock-<anything>-setup.exe` or `drydock-<anything>-setup.exe.part` except the
   one for the newer manifest's version (`drydock-<version>-setup.exe`, version in `Display` form). Other files are
   left alone. A missing folder is fine. A failed check deletes nothing.
6. Failure: if manual, state `Failed(reason)` (a non-empty sentence, no need to match the error
   exactly); if automatic, the state goes back to what it was before the check (so a failed
   automatic check never replaces `Available` or `Ready`, and never shows `Failed`).

## download(manual)

Only acts when the state is `Available(manifest)`; in any other state it returns without
touching the transport. It does not look at the mode (the user asked, or the schedule decided
under `Auto`).

1. Create `download_dir` if missing. Final name `drydock-<version>-setup.exe`; while downloading
   the transport writes to the same name with `.part` appended.
2. If the final file already exists and passes `verify_file_sha256` for the manifest, go straight
   to `Ready` with no download. A final file that fails is deleted first.
3. Clear the cancel flag, state `Downloading { manifest, received: 0 }`; the progress callback
   updates `received`. The transport is given `manifest.installer_url`, the channel headers and
   `max_bytes = manifest.size`. `state()` must work (and not block) while a download runs, and
   `cancel()` may be called from inside the transport call.
4. On `Ok`: `verify_file_sha256` on the `.part` file against the manifest's hash and size. If
   it passes, rename to the final name and go to `Ready { path, manifest }`.
5. Cancelled (the flag was set; check it yourself too, whatever the transport returned): delete
   the `.part` file and return to `Available(manifest)`. Not a failure.
6. Any other failure (transport error, size or hash mismatch, rename error): delete the `.part`
   file (and a mismatching final file) so nothing is left behind; if manual, `Failed(reason)`;
   if automatic, `Idle`.

## cancel, set_mode, mode, state

`cancel()` sets an `Arc<AtomicBool>` shared with the running download; with nothing running it
has no lasting effect (step 3 clears it). `set_mode` takes effect for the next schedule decision.

## schedule: `spawn(self: &Arc<Self>) -> Background`

One thread, which holds an `Arc<Updater>`. It waits `startup_delay`, then loops: when the mode
is not `Off`, `check(false)`; when the mode is `Auto` and the state is `Available`, `download(false)`;
then wait `interval`. The waits are interruptible (use `mpsc::Receiver::recv_timeout`) and the
mode is read fresh at each decision, so `Off` is never network traffic even after a mode change.
`Background::check_now` sends a command that runs `check(true)` at once, skipping the wait
(and any startup delay); `download_now` runs `download(true)`. Dropping `Background` tells the
thread to stop, sets the cancel flag, and joins it: after the drop returns no transport call is
made. A manual command while the mode is `Off` still runs. No panics from a poisoned lock.

## Tests

`tests/update_check.rs` (written): decisions (newer, same, older), request shape (URLs, headers,
no query, size limits), offline automatic and manual, bad, missing, truncated signatures,
tampered and garbage manifests, `Off` never calls the transport (check, schedule, drop), manual
check in `Off`, `Auto` downloads to `Ready`, `NotifyOnly` waits for `download_now`, interval,
startup delay, drop stops the schedule, progress, hash mismatch, size mismatch, mid-download
error, cancel mid-download, stale-file cleanup, reuse of a verified earlier file, replacement of
a damaged one, a newer release replacing `Ready`.

## Where to look

- Skeletons: `Updater`, `Background`, `Config`, `State`, `Mode` in `updater.rs`;
  `Transport`, `TransportError` in `transport.rs`.
- Dependencies from `m13-update-verify`: `verify`, `verify_file_sha256`, `FileCheckError`,
  `Version::is_newer_than`, `Version` `Display`, `Channel`.
- Test helpers in `tests/update_check.rs`: `Fake` (the fake transport), `release`, `release_with`,
  `setup`, `fresh_dir`, `names`, `wait_for`.
