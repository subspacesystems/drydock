# m10-brand-app-icon: the app icon from the new Drydock logo

The brand kit (`design/brand/logo/`) has the new logo. Replace the old icon (a cube drawn by
`assets/make-icon.py`) with renders of it, made by resvg (a dev-dependency, added on main).
Runtime code does not change: `window_icon::ICON_PNG` keeps its path and the Linux hicolor icons
are still made from it by `desktop_entry::files_for`, so they pick up the new artwork by themselves.

## What to do

1. Run the writer in `tests/app_icon.rs` (do not edit that file):
   `cargo test -p drydock --test app_icon -- --ignored write_icon_files`.
   It overwrites two files:
   - `app/crates/drydock/assets/drydock-256.png`: `drydock-app-icon.svg` (1000x1000) at 256 px.
   - `app/crates/drydock/assets/drydock.ico`: four PNG frames in this order: 16 and 32 px from
     `drydock-favicon.svg`, 48 and 256 px from `drydock-app-icon.svg`. `build.rs` embeds the file
     unchanged.
2. Commit both regenerated files.
3. Delete `assets/make-icon.py`.
4. Rewrite the icon lines of `assets/README.md` in our own words: the icon files are our own artwork
   from `design/brand/logo` (app icon and favicon SVGs), written by the ignored test
   `write_icon_files` in `tests/app_icon.rs` (give the command), `drydock-256.png` is the Linux window
   icon and the source of the hicolor icons, the ico holds frames 16, 32, 48 and 256. Do not mention
   `make-icon`. Keep the `land_110m.bin` line.
5. Do not change `window_icon.rs`, `desktop_entry.rs` or `build.rs`. Rust code under `src/` stays as is.

## Tests (`tests/app_icon.rs`, already written)

The committed PNG and every ico frame equal a fresh render of the matching SVG (within 2 per channel),
the PNG has transparent rounded corners, `ICON_PNG` is the asset and `window_icon()` returns its pixels,
the hicolor files from `files_for` are the app icon at 48, 128 and 256 px (256 byte for byte `ICON_PNG`),
`make-icon.py` is gone and the README names the logo folder and the writer test.

The hicolor and `window_icon` tests need `m9-linux-titlebar` and `m9-linux-desktop-entry` merged; this
task starts after them.

## Done when

`run-tests -p drydock` passes; fmt and clippy clean. The orchestrator then looks at the PNG and, in the
real window, the title bar and dock icon (Tyrel checks the Windows `.ico`).

## Where to look

- `show ICON_PNG`, `show window_icon` (`src/window_icon.rs`)
- `show files_for`, `show ICON_SIZES` (`src/desktop_entry.rs`)
- `tests/app_icon.rs`: `render`, `build_ico`, `write_icon_files`, `ico_frames` (read only)
- `assets/README.md` for the lines to rewrite; `build.rs` shows how the ico is embedded
