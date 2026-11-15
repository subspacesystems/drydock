# m13-update-ui: the setting, Help > Check for Updates, the restart prompt

Windows auto-update, last piece (M13). `drydock-update` (`Updater`, `Background`, `State`, `Mode`,
`Channel`, `Transport`) and the installer's `/S /UPDATE` mode are done. This task is the app's side.
The skeleton is on main: `app/crates/drydock/src/update_ui.rs` (all bodies `unimplemented!`), plus
`Action::CheckForUpdates` / `Effect::CheckForUpdates`, `MenuState::update_channel`, a no-op arm in
`app.rs`, and the guide page `docs/guide/updates.md` (already written; do not change it). Keep every
signature in `update_ui.rs`; do not edit `tests/update_ui.rs`, `tests/update_ui_hidden.rs` or
`tests/guide_coverage.rs`. No new dependencies. No network code here: the transport is injected.

## Where it is offered: `supported()`

True on Windows, or after `force_supported(true)` (a process-wide `AtomicBool`; tests use it). Menu,
window, checks and the channel all gate on this one function. Where it is false (Linux): no menu item, and
`app.rs` creates no `UpdateUi` at all (no thread, no network).

## The setting

`UpdateSetting` (Check automatically = default, Notify only, Off). File `updates` in the config folder
(`recent::config_dir`), format and atomic save as `PrintSettings` (`parse`/`to_text`/`load`/`save`,
`print_setup.rs`); the same folder is kept across updates, because the installer never touches it
(`m13-installer-update`). Helpers `settings_file`, `channel_file` (`update-channel`) and `download_dir`
(`update-downloads`) mirror `print_setup::settings_file`.

## The channel

- `built_in_channel(url, key_hex)`: name `built-in`, no headers; `None` unless the URL is `https://`
  and the key is 64 hex digits (`drydock_update::hex_decode_32`).
- `BUILT_IN_URL` / `BUILT_IN_KEY` are `option_env!("DRYDOCK_UPDATE_URL")` / `("DRYDOCK_UPDATE_KEY")`.
  App start: `built_in_channel(BUILT_IN_URL, key)` where `key` is `BUILT_IN_KEY`, else the text of
  `app/packaging/update-key.pub` (`include_str!`, read with `parse_key_file`: the throwaway key, so a
  development build can only be fed test releases). Never write a real URL or key in the repo.
- `resolve_channel(built_in, file_bytes)`: a `update-channel` file that parses (`Channel::parse`) wins
  (that is how a token header is added); one that does not is ignored with one `eprintln!` line.
  Neither: `None`, so no `Updater` is built. The Updates window says "Updates are not set up in this
  build." and the Help item reads "Check for Updates... (not set up)" and is greyed.

## UpdateUi

`UpdateUi::new(Parts)`: with a channel, build `Updater` (`Config { channel, running, mode:
setting.mode(), download_dir, startup_delay, interval }`) and `Updater::spawn`; keep the `Background`
(dropping `UpdateUi` stops the thread). `app.rs` builds `Parts` with `WinHttpTransport` (Windows) on the
real path, `ProcessLauncher`, `Version::parse(chrome::VERSION)`, `startup_delay` 20 s, `interval` 24 h.
The Windows transport type is behind `#[cfg(windows)]`; make a small cfg function that returns
`Arc<dyn Transport>`; on non-Windows (reachable only when forced) use a transport that always returns
`TransportError::Offline`.

Per frame `show(ctx)`: in `Auto`, when the state is `Available` and the download for that version was not
yet requested, `Background::download_now` once (the schedule only downloads at its own pace; a manual
check should not wait a day). Call `ctx.request_repaint_after(250 ms)` while the state is `Checking` or
`Downloading` or a check was asked for, so the UI picks up results. Then draw:

1. **Updates window** while open (`dialog::dialog_window`, title "Updates", about 360 x 260; follow
   `print_setup_window`). Contents: "Drydock {chrome::VERSION}"; the three choices as radio buttons
   (`ui.radio_value`) labelled exactly "Check automatically", "Notify only", "Off"; a "Check now" button;
   the status line; when `Available` in Notify only, a "Download" button (`Background::download_now`).
   Without a channel: only the text "Updates are not set up in this build.". Choosing a radio calls
   `set_setting`.
2. **Ready prompt** while `prompt_showing()`: a non-modal `dialog_window` titled "Update ready" with
   "Drydock X.Y.Z is ready to install" (X.Y.Z from `Manifest::version`), a "Release notes" hyperlink to
   `notes_url` (`ui.hyperlink_to`), a primary button "Restart and update" (`look::primary_button`) and a
   button "Later". "Restart and update" returns `Some(UpdateEvent::Restart)`. "Later" (or closing the
   window) hides the prompt for that version until the next run; a newer `Ready` version shows it again.

## Status texts (exact)

Shown in the window and returned by `status_text()` (the app puts it in the status bar next to
`export_note`, and calls `clear_status()` where it clears `export_note`):

- `Checking` -> "Checking for updates..."
- `UpToDate` -> "Drydock is up to date."
- `Failed(_)` -> "Couldn't reach the update server." (whatever the reason; no dialog, never blocking)
- `Available(m)` -> "Drydock {version} is available."
- `Downloading` -> "Downloading Drydock {version}..."
- `Ready` -> "Drydock {version} is ready to install."
- a launcher failure -> "Could not start the installer: {reason}"

`status_text()` is `None` until the user started a check (`check_now` / `open_dialog`) and after
`clear_status()`; automatic checks never produce text (the updater already keeps `Failed` for manual
checks only). The window shows the same text for the current state at any time.

## Restart flow (the guard)

`begin_restart(modified)`, `answer_unsaved(choice)`, `after_saved(saved)`, `cancel_restart()`,
`restart_pending()` are the state machine; the tests drive it directly. Launching reads `State::Ready {
path, .. }` and calls `Launcher::launch(path)` once. Wiring in `app.rs` (read `request`, `perform`,
`save_before`, the `self.confirm` block near the end of `update`, `SaveOutcome::Cancel`):

- `UpdateEvent::Restart` -> `self.updates.begin_restart(modified)` with `modified` as in `request`.
- `Step::Ask` -> show `prompts::unsaved_prompt` every frame while `restart_pending()` and no save is
  running; its answer goes to `answer_unsaved`.
- `Step::Save` -> start the usual save (`self.save`); when it finishes (where `after_save` is taken in
  `saved_now`) call `after_saved(true)`; when the save dialog is cancelled call `cancel_restart()`.
  Use a new `PendingAction::InstallUpdate` for the after-save hook if that is the least change.
- `Step::Quit` -> `self.allow_close = true` and `ViewportCommand::Close` (as `PendingAction::Close`
  does); the unsaved question is not asked again. `Step::Stay` -> nothing.

## Menu

`chrome.rs` Help menu: User Guide, then "Check for Updates..." (`Action::CheckForUpdates`), then About,
the new item only `if update_ui::supported()`. Enabled when `MenuState::update_channel`; otherwise greyed
(`add_enabled_ui`) with the label "Check for Updates... (not set up)". `app.rs`: `Effect::CheckForUpdates`
-> `self.updates.open_dialog()`; `menu_state` sets `update_channel`.

## Tests

`tests/update_ui.rs` (written): setting parse/save/load/persist, paths, channel building and override,
menu item (shown, order, click, greyed, hidden), the window (choices, saving, check now, up to date,
offline, notify-only download), no channel, automatic mode (downloads quietly, never shows errors),
Off (no traffic, also after switching), the prompt (Restart / Later), the unsaved-changes guard (clean,
Cancel, Save then start, failed save, Don't save, launcher failure, not ready), the real launcher's flags.
`tests/update_ui_hidden.rs`: not supported means no menu item. `tests/guide_coverage.rs` needs
"Help > Check for Updates" in the guide (done). Run the kittest tests without `DRYDOCK_GUI_TESTS`.

## Where to look

- `update_ui.rs`: everything above. `drydock_update`: `Updater`, `Background::{check_now,download_now}`,
  `State`, `Mode`, `Channel::parse`, `hex_decode_32`, `Version`.
- `print_setup.rs`: `PrintSettings::{parse,to_text,load,save}`, `settings_file`, `print_setup_window`.
- `dialog.rs`: `dialog_window`, `DialogWindow`; `look::primary_button`; `prompts::unsaved_prompt`.
- `chrome.rs`: Help menu in `menu_bar_inner`, `MenuState`; `viewstate.rs` `Effect`.
- `app.rs`: `request`, `perform`, `save_before`, `saved_now`, `after_save`, `allow_close`, `menu_state`,
  the status bar near `export_note`, `PendingAction`. `recent::config_dir`.
- Test helpers in `tests/update_ui.rs`: `Fake`, `Rec`, `rig`, `rig_with`, `harness`, `wait`, `ready`.
