# m9-linux-desktop-entry: desktop entry and icons so the dock shows Drydock

GNOME matches a window's app id (Wayland `app_id`, X11 `WM_CLASS`) to `<app_id>.desktop`. Without
one the dock shows a gear. Linux has no installer (people run cargo builds), so the app keeps the
files up to date itself at startup. Builds on `m9-linux-titlebar` (`APP_ID`, `ICON_PNG`).

## Module `drydock::desktop_entry` (skeleton on main, fill in the bodies)

- **`data_dir_from(xdg, home)`**: `xdg` if set, non-empty and absolute; else `home/.local/share` if `home` is set and non-empty; else `None`.
- **`files_for(data_dir, exe)`**: list of `DesktopFile { path, bytes }`, in this order:
  1. `data_dir/applications/<APP_ID>.desktop`
  2. `data_dir/icons/hicolor/<n>x<n>/apps/<APP_ID>.png` for n in `ICON_SIZES` (48, 128, 256).
  The 256 px file is `ICON_PNG` byte for byte. Smaller ones are `ICON_PNG` decoded, scaled with
  `image::imageops::resize` (Lanczos3) and encoded as PNG (the `image` crate is already a dependency).
- **`desktop_entry_text(exe)`**: first line `[Desktop Entry]`, then one `Key=Value` per line, ending with a newline:
  `Type=Application`, `Name=Drydock`, `Comment=` (one short line of your own, e.g. "3D modeler for architecture and woodworking"), `Exec=<exec_line>`, `Icon=<APP_ID>`, `Terminal=false`, `Categories=Graphics;3DGraphics;`, `StartupWMClass=<APP_ID>`. No `MimeType` (skip).
- **`exec_line(exe)`**: the path as ONE double-quoted argument, then a space and `%f`. Escaping, in two layers:
  1. Inside the quotes put a backslash before `"`, `` ` ``, `$` and `\`.
  2. The file format reads values as strings, so every backslash is doubled again; a literal `%` is written `%%`.
  So `/home/a b/drydock` gives `"/home/a b/drydock" %f` and `/x/a"b` gives `"/x/a\\"b" %f` (two backslashes in the file). Convert the path with `to_string_lossy`.
- **`install(data_dir, exe) -> Report`**: for each file of `files_for`: read the existing file; if it is missing or its bytes differ, create the parent folders and write it (write to a temp name next to it and rename, so a crash never leaves half a file). Put the path in `written` when written, or `(path, error text)` in `failed` when any step fails, and go on with the next file. Unchanged files appear in neither list and are not touched (modification time stays). Never touch any other file, never panic.
- **`install_for_current_user()`**: on Linux only (`cfg(target_os = "linux")`; empty body elsewhere): `exe = std::env::current_exe()`, dir from `data_dir_from(env XDG_DATA_HOME, env HOME)`; if either is missing, print one line to stderr and return. Run `install`; print one stderr line per failure (`Drydock: could not write <path>: <error>`), nothing on success. If anything was written, best-effort run `update-desktop-database <data_dir>/applications` and `gtk-update-icon-cache -f -t <data_dir>/icons/hicolor` (`std::process::Command`, stdout/stderr null, ignore a missing program or a bad exit status; do not wait longer than the commands take).

## Wiring

In `main.rs` call `drydock::desktop_entry::install_for_current_user()` once, just before building `NativeOptions` for the normal window run; the `--convert` and `--screenshot` paths return earlier and must not call it. (`main.rs` is also touched by `m9-linux-titlebar`; build on main.)

## User guide

Add a short section in our own words to the guide (a new page `docs/guide/linux.md` titled "Drydock on Linux", linked from `docs/guide/SUMMARY.md`; keep `guide_coverage` and the guide build tests green). It says: there is no installer on Linux; at start Drydock writes a desktop entry and icons so the dock and launcher show its icon; where (`$XDG_DATA_HOME`, default `~/.local/share`: `applications/` and `icons/hicolor/`); that it rewrites them only when missing or when you start Drydock from a different place; that it touches no other files and carries on if the folder is read-only; and how to remove them (delete those files). The test checks the page mentions `.desktop`, `XDG_DATA_HOME` and `hicolor`.

## Tests

`app/crates/drydock/tests/desktop_entry.rs` (temp dirs only, never the real home): data dir rules, Exec quoting cases, entry keys, file names and order, icon sizes, first install, no-op second run (mtime unchanged), moved binary rewrites only the entry, bad/deleted icon restored, other files untouched, blocked folder reported while the rest is written, guide mentions the files.

## Done when

`run-tests -p drydock` passes the new tests; fmt and clippy clean. The orchestrator then runs the real window with `HOME` and `XDG_DATA_HOME` pointed at a temp dir and checks the files appear (Tyrel checks the dock on Ubuntu).

## Where to look

- `show APP_ID`, `show ICON_PNG`, `show window_icon` (`src/window_icon.rs`)
- `show desktop_entry` skeleton (`src/desktop_entry.rs`): every signature is there
- `show main` in `src/main.rs`: where the normal run starts (`NativeOptions`)
- `image::imageops::resize`, `image::DynamicImage::write_to` / `ImageFormat::Png`
- `docs/guide/SUMMARY.md`, `docs/guide/files.md` (style), `tests/guide_coverage.rs` (what the guide must contain)
