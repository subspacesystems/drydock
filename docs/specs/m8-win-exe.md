# m8-win-exe: the program builds for Windows

The app must build for `x86_64-pc-windows-gnu` as `drydock.exe`: no console window in release
builds, our own icon and version info inside the .exe, and settings folders that follow
Windows conventions. A gate (added by the orchestrator, not part of this task) runs
`cargo check` and clippy `-D warnings` for that target, so nothing here may break it: no
Unix-only code outside `#[cfg(unix)]`.

Acceptance tests: `crates/drydock/tests/win_exe.rs` (pure; they run on Linux). Don't edit them.
Already on main: the icon `crates/drydock/assets/drydock.ico`, the `winresource` build
dependency (MIT) in `Cargo.toml`, and skeletons `src/winmeta.rs` and `src/winpaths.rs`.

## 1. No console window (`src/main.rs`)

Add this exact line as an inner attribute after the `//!` doc comment:

```rust
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
```

Debug builds keep the console, so `println!` output and panics stay visible while developing.

## 2. Icon and version info (`build.rs`)

Keep the existing git-hash step. Add, after it, a step that runs only when the **target** is
Windows: read `CARGO_CFG_TARGET_OS` (the build script runs on the host, so `cfg!(windows)`
would be wrong when cross-compiling) and act only when it equals `"windows"`.

- `println!("cargo:rerun-if-changed=assets/drydock.ico")`.
- Include the helpers with `#[path = "src/winmeta.rs"] mod winmeta;` (a build script can't
  use the crate itself). `winmeta.rs` therefore uses nothing else from the crate.
- With `winresource::WindowsResource::new()`: `set_icon("assets/drydock.ico")`; set the
  numeric file and product versions with
  `set_version_info(winresource::VersionInfo::FILEVERSION, ...)` and `PRODUCTVERSION`, both from
  `winmeta::packed_version(winmeta::file_version(env!("CARGO_PKG_VERSION")))`
  (`[0, 0, 0, 0]` when it doesn't parse); and each pair from `winmeta::version_strings(...)` via
  `set(key, value)`. Then `compile()`; on error, `panic!` with the message (a Windows build
  without its resources must fail loudly).
- Nothing in this step runs for Linux targets, so Linux builds need no `windres`.

## 3. `winmeta` helpers (`src/winmeta.rs`)

- `file_version(v)`: cut `v` at the first `-` or `+` (pre-release or build suffix), split the
  rest at `.`; 3 or 4 parts, each a plain decimal number that fits `u16` (no sign, not empty), else `None`.
  A missing fourth part is `0`.
- `packed_version(parts)`: `parts[0] << 48 | parts[1] << 32 | parts[2] << 16 | parts[3]`.
- `version_strings(v)`, in this order:
  `CompanyName` = `COMPANY_NAME`; `FileDescription` = `PRODUCT_NAME`; `FileVersion` = the four
  parts joined with `.` (`"0.0.0.0"` when `file_version` is `None`); `InternalName` = `drydock`;
  `LegalCopyright` = `Copyright (C) 2026 Subspace Systems`; `OriginalFilename` = `FILE_NAME`;
  `ProductName` = `PRODUCT_NAME`; `ProductVersion` = `v` exactly as given.

## 4. Config folders (`src/winpaths.rs`, `src/recent.rs`)

`config_dir_on(os, var)` returns the app's settings folder, reading variables through `var`;
a variable that is unset or empty counts as missing. First match wins:

1. `DRYDOCK_CONFIG_DIR`, used as is, on every OS.
2. Windows only: `APPDATA` joined with `Drydock`; else `USERPROFILE` joined with
   `AppData`, `Roaming`, `Drydock` (use `PathBuf::join` for each part, not a string with
   separators). Windows never looks at `XDG_CONFIG_HOME` or `HOME`.
3. Linux and Mac: `XDG_CONFIG_HOME` joined with `drydock`, only when it is an absolute path;
   else `HOME` joined with `.config`, `drydock`. These never look at `APPDATA` or `USERPROFILE`.
4. `None`.

`MODELER_CONFIG_DIR` and the old `modeler` folder are not read (m8-rename-config settled that).
In `recent.rs`, `config_dir` becomes `config_dir_on(Os::current(), var)` (`look::Os`), and its
own `cfg!(windows)` branch goes away. Keep `recent_file` and the existing recent-files tests
passing; if an existing test relied on the old inline logic, it keeps its meaning through the
new function.

## Out of scope

- The installer, signing, release scripts and the CI gate for the Windows target.
- A window icon set at run time (the .exe icon covers Explorer and the taskbar).
- Texture and font paths: `document::texture_path_for_file` already writes `/` separators and
  `look::font_candidates(Os::Windows)` already lists the Windows fonts.

## Where to look

- `show file_version`, `show packed_version`, `show version_strings` (skeletons in `winmeta.rs`).
- `show config_dir_on` (skeleton in `winpaths.rs`), `show config_dir`, `show recent_file` in `recent.rs`.
- `show Os` and `Os::current` in `look.rs`.
- `build.rs`: the existing `main` (git-hash step) to extend.
- `tests/win_exe.rs`: helpers `crate_file` and `env` show how the tests read sources and fake variables.
- `Cargo.toml` of the crate: `[build-dependencies]` already lists `winresource`.
