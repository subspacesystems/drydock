# m8-installer-notice: say so when an older Drydock is installed

Tyrel: setup over an older version upgraded fine but said nothing about it. Setup now
tells the user which version is installed and that it will replace it, and lets them cancel.

## Change (only `app/packaging/windows/drydock.nsi`, `Function .onInit`)

After the existing `SetRegView 64` / `SetShellVarContext all`:

1. `ReadRegStr` the uninstall key's `InstallLocation` (same key as `RemoveOldVersion`) into `$R1`
   (the variable `RemoveOldVersion` uses: `installer_script.rs` takes the first `InstallLocation` read as the folder variable). If empty, skip everything below (fresh install: no dialog).
2. `ReadRegStr` its `DisplayVersion` into `$R2`.
3. `IfSilent` -> jump past the dialog (silent installs never show it; `/S` runs and the sign-off
   scripts depend on that).
4. One `MessageBox MB_OKCANCEL|MB_ICONINFORMATION`, text by case (compare with `StrCmp`):
   - old version empty: `Subspace Drydock is installed in <dir>. Setup will remove it and install ${VERSION}.`
   - old version equals `${VERSION}`: `Subspace Drydock ${VERSION} is installed in <dir>. Setup will remove it and reinstall it.` (contains the word "reinstall" and `${VERSION}`)
   - otherwise: `Subspace Drydock <old> is installed in <dir>. Setup will remove it and install ${VERSION}.`
   Use the variables for `<old>` and `<dir>`; `$\n` is fine for a line break.
5. Cancel -> `Abort` right after the dialog (`IDOK` jump over it). Nothing is removed yet.

Do not move the removal: `RemoveOldVersion` still runs from `Section "Install"`, and `.onInit`
must not call `ExecWait`. Do not touch other functions or sections. Labels in a Function are
local to it; use unique names. Keep `Unicode true` quoting rules in mind (`$` in text must be
a variable or `$$`).

## Tests

`app/crates/drydock/tests/installer_notice.rs` (text checks, any OS; they fail until the script
is changed). The existing `installer_script.rs` tests must keep passing, and the oracle's nsis
gate compiles the script. If `makensis` is installed, also run `makensis -DVERSION=1.2.3
-DSTAGE=<dir with a dummy drydock.exe> app/packaging/windows/drydock.nsi`.

## Where to look

- `show script_path`, `show lines`, `show find` in `installer_script.rs` (helpers, same rules).
- `show RemoveOldVersion` is in the .nsi itself: `harness/bin/outline app/packaging/windows/drydock.nsi`.
- Test `old_uninstaller_is_found_by_the_install_folder` (it scans every `IfFileExists` and
  `ReadRegStr`, so your new reads must not break it).
