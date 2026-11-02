# m8-release: release script for the Windows installer

Write `app/packaging/release.sh` (bash) and `app/packaging/INSTALL.md`. The script builds the
Windows program, packs it into the installer, optionally signs, and writes the files a beta
tester downloads. Acceptance tests: `crates/drydock/tests/release_script.rs` (don't edit). They
run the script against stub `cargo`, `makensis` and `osslsigncode` programs, so they need only
bash and `sha256sum`. No Rust changes.

Needs `m8-installer` (it provides `app/packaging/windows/drydock.nsi`, which takes `-DVERSION`,
`-DSTAGE` and `-DOUTFILE`) and `m8-win-exe` (the `drydock` program builds for Windows).

The CI gate, publishing the files and the signing certificate are not part of this task.

## The script

Lines 1-3: `#!/usr/bin/env bash`, `# SPDX-License-Identifier: GPL-3.0-or-later`,
`# Copyright (C) 2026 Subspace Systems`. Then `set -euo pipefail`. Executable bit set. Never
mention the codename. Paths come from the script's own location (`$(dirname "${BASH_SOURCE[0]}")`),
so it works from any directory; `app/` is the folder above `packaging/`.

Usage: `release.sh [--version X.Y.Z] [--out DIR] [--guide DIR] [--dry-run]`

- `--version`: must match `^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$`; otherwise print a
  message containing the word "version" to stderr and exit non-zero before running anything
  (an empty value is bad too). Without the option, use the first `version = "..."` after
  `[workspace.package]` in `app/Cargo.toml`.
- `--out`: output folder (default `app/dist`), created if missing, then made absolute.
- `--guide`: a folder (the built user guide) copied to `<stage>/guide`. If it isn't a folder,
  fail before building.
- `--dry-run`: print each step, run nothing, write nothing.

Environment: `CARGO`, `MAKENSIS`, `OSSLSIGNCODE` name the programs (defaults `cargo`,
`makensis`, `osslsigncode`). `CARGO_TARGET_DIR` (default `app/target`; export it so cargo sees
it). `DRYDOCK_SIGN_PFX` (path to a PKCS#12 certificate) and `DRYDOCK_SIGN_PASS_FILE` (file
holding its password); an unset or empty `DRYDOCK_SIGN_PFX` means "no certificate".

## Steps, in order

1. Check arguments, the guide folder, and (when configured) that the certificate file exists:
   message names the path, exit non-zero, nothing built yet. Without a certificate print
   a line containing `signing skipped`.
2. From `app/`: `$CARGO build --release --locked --target x86_64-pc-windows-gnu -p drydock`.
3. `drydock.exe` must be at `$CARGO_TARGET_DIR/x86_64-pc-windows-gnu/release/drydock.exe`;
   if not, say so (naming `drydock.exe`) and exit non-zero. Make `<out>/stage` empty, copy the
   program in, and the guide folder to `<out>/stage/guide` when given.
4. Sign `stage/drydock.exe` (see below).
5. `$MAKENSIS -DVERSION=<v> -DSTAGE=<abs stage> -DOUTFILE=<abs out>/drydock-<v>-setup.exe
   <abs path of packaging/windows/drydock.nsi>`; the `.nsi` path is the last argument. If it
   fails, or the installer file is missing afterwards, exit non-zero.
6. Sign the installer.
7. Copy `INSTALL.md` to `<out>/INSTALL.txt`.
8. From inside `<out>`: `sha256sum INSTALL.txt drydock-<v>-setup.exe > SHA256SUMS` (lowercase hex,
   two spaces, plain names), so `sha256sum -c SHA256SUMS` passes there and rerunning
   overwrites the file.

Signing a file F (only when `DRYDOCK_SIGN_PFX` is set):
`$OSSLSIGNCODE sign -pkcs12 $PFX -readpass $DRYDOCK_SIGN_PASS_FILE -h sha256 -in F -out F.signed`,
then move `F.signed` over `F`. The password is read from the file and never appears on a
command line. (A timestamp server option can come when a certificate exists.)

Dry run prints every command it would run as a line starting `+ ` (the cargo, signing and
makensis steps, signing ones only when a certificate is configured), in order. The checks in
step 1 still run.

## INSTALL.md

Plain, short text for beta testers, product named "Subspace Drydock", never "modeler" or
another product's name. It covers, each as its own short section or bullet:

- How to install: run `drydock-<version>-setup.exe`; installing over an older version upgrades
  it in place; uninstall from Windows Settings > Apps.
- Checking the download: `SHA256SUMS`, with the PowerShell line `Get-FileHash` to compare.
- The build is **unsigned** for now. **SmartScreen** may say "Windows protected your PC":
  click **More info**, then **Run anyway**.
- **Smart App Control** (Windows 11) blocks unsigned programs with no way to click through:
  say so, and that turning it off in Windows Security is the only way to run the beta (it
  can't be turned back on without a reset, so testers should weigh that).
- Where settings live (`%APPDATA%\Drydock`) and that uninstalling keeps models and settings.

## Where to look

- Test helpers in `tests/release_script.rs`: `Rig` (stubs, log, `run`), `CARGO_STUB`,
  `MAKENSIS_STUB`, `SIGN_STUB`, `checksum_names`; read each test for the exact strings checked.
- `app/Cargo.toml` `[workspace.package]` for the version.
- On the `task/m8-installer` branch (merged before you start): `app/packaging/windows/drydock.nsi`
  and its spec `docs/specs/m8-installer.md` for the defines.
