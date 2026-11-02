# m8-installer-fix: the installer script compiles and an upgrade removes the old version

Milestone M8. File: `app/packaging/windows/drydock.nsi` only. Tests:
`crates/drydock/tests/installer_script.rs` (text checks, plus `builds_when_makensis_is_available`,
which runs `makensis` when it is installed and skips otherwise).

## Two bugs

1. `SetRegView 64` and `SetShellVarContext all` sit at the top level of the script. `makensis`
   stops with "command SetRegView not valid outside Section or Function", so no installer is built.
2. `RemoveOldVersion` tests `IfFileExists "$R0"` where `$R0` is the `UninstallString` value. That
   value is stored with quotes around the path, so the test is never true and an upgrade never runs
   the old uninstaller (the old folder stays).

## Changes

- Delete both top-level lines. Add

  ```
  Function .onInit
    SetRegView 64
    SetShellVarContext all
  FunctionEnd

  Function un.onInit
    SetRegView 64
    SetShellVarContext all
  FunctionEnd
  ```

  The uninstaller needs the 64-bit registry view and the all-users Start Menu as much as the
  installer. Place both before `RemoveOldVersion`.
- In `RemoveOldVersion` read only what is needed: drop the `UninstallString` read (or leave it
  unused; tests don't need it). Use the install folder (`InstallLocation`, in `$R1`):
  - `StrCmp $R1 "" done` (nothing known to remove),
  - `IfFileExists "$R1\uninstall.exe" 0 done`,
  - `ExecWait '"$R1\uninstall.exe" /S _?=$R1'`,
  - then `Delete "$R1\uninstall.exe"` and `RMDir "$R1"` as now.
- Nothing else changes: sections, registry keys, pages and names stay as they are.

## Done when

- `harness/bin/run-tests -p drydock` passes, including the three tests that fail now:
  `removes_the_old_version_before_installing`, `old_uninstaller_is_found_by_the_install_folder`,
  `registry_view_and_shell_context_are_set_inside_functions`.
- Where `makensis` is installed, `builds_when_makensis_is_available` passes (it fails on the
  current script).

## Where to look

- `app/packaging/windows/drydock.nsi`: `RemoveOldVersion`, the top-level settings block.
- Tests: `enclosing`, `lines`, `must`, `find`, `find_from`, `UNINSTALL_KEY` in
  `crates/drydock/tests/installer_script.rs` (read how they match lines).
