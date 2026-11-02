# m8-installer: NSIS installer with in-place upgrade and uninstaller

Write one file, `app/packaging/windows/drydock.nsi`, an NSIS script (NSIS is zlib-licensed
and a build tool only; nothing of it ships). It builds on Linux with `makensis`, though the
tests below do not need it: they read the script as text, and one test builds it if `makensis`
is on the PATH. Acceptance tests: `crates/drydock/tests/installer_script.rs` (don't edit).
No Rust changes. The script's first two lines are `; SPDX-License-Identifier: GPL-3.0-or-later`
and `; Copyright (C) 2026 Subspace Systems`.

The product is **Subspace Drydock**, by **Subspace Systems**. The script must not contain the
codename (`modeler`) or any other product's name, even in comments.

## Build inputs (passed with `-D`)

- `VERSION` (required): `!ifndef VERSION` / `!error "pass -DVERSION=x.y.z"` / `!endif` at the top.
- `STAGE` (required): folder with `drydock.exe` and optionally a `guide` folder (the user
  guide, built by a later task). Use `File "${STAGE}\drydock.exe"` and
  `File /nonfatal /r "${STAGE}\guide"`.
- `OUTFILE` (optional): defaults to `drydock-setup-${VERSION}.exe` (`!ifndef OUTFILE` / `!define`).
  `OutFile "${OUTFILE}"`.

## Settings

`Unicode true`, `Name "Subspace Drydock"`, `InstallDir "$PROGRAMFILES64\Subspace Drydock"`,
`RequestExecutionLevel admin`, `SetRegView 64`, `SetShellVarContext all` (so shortcuts and
file types are for all users). LZMA compression is fine (`SetCompressor /SOLID lzma`).
Pages: `Page directory`, `Page instfiles`, `UninstPage uninstConfirm`, `UninstPage instfiles`.
Silent installs (`/S`) work for free with these.

## Install section, in this order

1. **Remove the old version.** Let `UNKEY` be
   `Software\Microsoft\Windows\CurrentVersion\Uninstall\SubspaceDrydock`. `ReadRegStr` from
   `HKLM` the `UninstallString` and the `InstallLocation` values of `UNKEY`. If an
   uninstaller is recorded and the file exists, run it silently and wait:
   `ExecWait '"$R0" /S _?=$R1'` (`$R0` the uninstaller path, `$R1` the old folder; `_?=` makes
   the uninstaller run in place so `ExecWait` really waits). Afterwards `Delete` the old
   `uninstall.exe` and `RMDir` the old folder (non-recursive). If no registry entry exists,
   do nothing. Use a `Function` or inline code, but it must come before any `File` line.
2. `SetOutPath "$INSTDIR"`, install `drydock.exe` and the optional guide folder (above).
3. `WriteUninstaller "$INSTDIR\uninstall.exe"`.
4. `CreateShortcut "$SMPROGRAMS\Subspace Drydock.lnk" "$INSTDIR\drydock.exe"`.
5. **Add/Remove Programs.** Under `HKLM` and `UNKEY`, `WriteRegStr`: `DisplayName`
   `"Subspace Drydock"`, `Publisher` `"Subspace Systems"`, `DisplayVersion` `"${VERSION}"`,
   `UninstallString` `"$INSTDIR\uninstall.exe"` (quoted), `InstallLocation` `"$INSTDIR"`,
   `DisplayIcon` `"$INSTDIR\drydock.exe"`; `WriteRegDWORD` `NoModify` 1 and `NoRepair` 1.
6. **File type, `.drydock` only.** Under `HKLM`: `Software\Classes\.drydock` default value
   `SubspaceDrydock.Model`; `Software\Classes\SubspaceDrydock.Model` default
   `"Drydock model"`; `...\SubspaceDrydock.Model\DefaultIcon` default `"$INSTDIR\drydock.exe",0`;
   `...\SubspaceDrydock.Model\shell\open\command` default `"$INSTDIR\drydock.exe" "%1"`.
   Register no other extension (the program still opens older files by itself).

## Uninstall section (`Section "Uninstall"`)

- `Delete` `$INSTDIR\drydock.exe` and `$INSTDIR\uninstall.exe`, `RMDir /r "$INSTDIR\guide"`,
  then `RMDir "$INSTDIR"` (non-recursive: leaves the folder if anything else is in it).
- `Delete "$SMPROGRAMS\Subspace Drydock.lnk"`.
- `DeleteRegKey HKLM` for `UNKEY`, `Software\Classes\.drydock` and
  `Software\Classes\SubspaceDrydock.Model`.
- Never touches user settings or models: no mention of `APPDATA`, `$DOCUMENTS`, `$PROFILE`,
  `$DESKTOP` or `*.drydock`, and no recursive delete of `$INSTDIR` or `$PROGRAMFILES`.

Optionally call `System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'` after
the file-type keys, in both sections, so Explorer refreshes.

## Where to look

- Test helpers in `crates/drydock/tests/installer_script.rs`: `lines`, `must`, `find_from`
  (read how each test matches lines), and the constant `UNINSTALL_KEY`.
- `crates/drydock/src/winmeta.rs` (`PRODUCT_NAME`, `COMPANY_NAME`, `FILE_NAME`): the names to match.
- `crates/drydock/assets/drydock.ico` exists; the installer needs no other asset.
- No existing `.nsi`; `app/packaging/windows/` is new (create the folder with the file).
