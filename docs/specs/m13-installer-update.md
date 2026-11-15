# m13-installer-update: silent update mode, safe install order

Windows auto-update (M13) runs the existing installer silently and expects Drydock to come back.
Two problems in `app/packaging/windows/drydock.nsi` today: `Section "Install"` removes the old
version (`Call RemoveOldVersion`) before copying anything, so a failed copy leaves no working
Drydock; and there is no way to restart the program afterwards. Change only `drydock.nsi`.

## Command line (document it in a comment block at the top, before `!ifndef VERSION`)

- `setup.exe` interactive, as now.
- `setup.exe /S` silent install; nothing is started.
- `setup.exe /S /UPDATE` silent update: installs into the folder of the installed version and starts
  `drydock.exe` again, not elevated, when done.

## Install order

1. Copy the new program next to the old one under another name: `File /oname=drydock.exe.new "${STAGE}\drydock.exe"`
   (after `SetOutPath "$INSTDIR"`), then move it over `drydock.exe` in one step with
   `System::Call 'kernel32::MoveFileExW(w "$INSTDIR\drydock.exe.new", w "$INSTDIR\drydock.exe", i 1) i .r0'`
   (flag 1 = replace existing). A running old program makes the move fail: retry every 500 ms, 20 times
   (`IntCmp`, `Sleep`, `Goto`). On giving up: `Delete "$INSTDIR\drydock.exe.new"` and `Abort "..."` with
   a message that the installed version was not changed. Nothing else has been changed at that point.
2. Then the guide (`File /nonfatal /r "${STAGE}\guide"`, unchanged).
3. Then `Call RemoveOldVersion`. It must skip the folder being installed into (`StrCmp $R1 "$INSTDIR" done`
   before the `IfFileExists`): that uninstaller would delete the files just copied. An older version in
   another folder is still removed by its own uninstaller (`ExecWait ... /S _?=...`), as now.
4. Only after that: `WriteUninstaller`, the Start menu shortcut, all registry values (the old
   uninstaller deletes the shortcut and keys, so they must be written after it). No `WriteReg*` or
   `CreateShortcut` before `Call RemoveOldVersion`.

## Update mode

- Top of the file: `!include "FileFunc.nsh"` (ships with NSIS, zlib license; no other include, no
  plugin directory; the built-in `System::Call` is the only plugin call), then
  `!insertmacro GetParameters` and `!insertmacro GetOptions`, and `Var UpdateMode`.
- `Function .onInit`, right after `SetShellVarContext all`: `StrCpy $UpdateMode "0"`, `${GetParameters} $R0`,
  `ClearErrors`, `${GetOptions} $R0 "/UPDATE" $R3`, and `StrCpy $UpdateMode "1"` when there was no error
  (`IfErrors +2 0` skips it). Do not add a `StrCmp ... ""` before the existing one: `installer_notice.rs`
  takes the first as the "no install" check.
- At `notice_done:` (end of `.onInit`): in update mode with a non-empty `$R1`, `StrCpy $INSTDIR $R1`.
  The registry read there uses the 64-bit view already.
- `Function .onInstSuccess` (runs only on success): when silent and `$UpdateMode` is `"1"`,
  `Exec '"$WINDIR\explorer.exe" "$INSTDIR\drydock.exe"'`. Launching through explorer gives the program
  the user's normal rights, not this installer's administrator rights. No `ExecShell`, no `runas`,
  no direct `Exec` of drydock.exe.
- Settings in `%APPDATA%\Drydock` and the user's models are never read or touched (no `appdata`,
  `$PROFILE` or `*.drydock` in the script).

## Tests

`app/crates/drydock/tests/installer_script.rs` (text checks, any OS; already written, replacing the old
"removes the old version before installing" test) and `installer_notice.rs` (unchanged, must keep
passing). `builds_when_makensis_is_available` and the oracle's `nsis` gate build the script with an
empty-ish stage; NSIS label names are local to a section or function. The sign-off checks the real
behavior under Wine.

## Where to look

- `app/packaging/windows/drydock.nsi`: `Function .onInit`, `Function RemoveOldVersion`, `Section "Install"`.
- `installer_script.rs`: `lines`, `find_from`, `must`, `enclosing`, and the new tests
  `old_version_is_removed_only_after_the_new_files_are_in_place`,
  `new_program_is_swapped_in_so_a_failed_copy_keeps_the_old_one`, `update_*`.
- `installer_notice.rs`: `on_init` (the `.onInit` lines it inspects).
