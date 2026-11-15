# m13-release-manifest: release tool (key pair, manifest, signature)

The release side of M13. A small program makes the update key pair and signs the manifest of a
built installer; `release.sh` can run it after building. The app side (`drydock-update`,
`m13-update-verify`) already verifies; this task produces what it verifies. Nothing is
published anywhere here.

The crate `drydock-release` (library + binary `drydock-release`) is on main as a skeleton
(`unimplemented!()` bodies), with its `deny.toml` row. Fill it in. Tests: `crates/drydock-release/tests/release.rs`
and the new section at the end of `crates/drydock/tests/release_script.rs`. They must pass
unchanged.

## Formats (fixed)

- **Private key file / environment variable**: the 32-byte Ed25519 seed as 64 lower-case hex
  digits. The file has a final newline. The variable holds the same text; trim whitespace
  around it.
- **Public key**: 64 lower-case hex digits (the form in channel files and `update-key.pub`).
- **`manifest`**: exactly `Manifest::to_text()` of the manifest being signed. **`manifest.sig`**:
  the 64 raw signature bytes over the manifest file's exact bytes (see `drydock-update`'s crate
  docs). Ed25519 is deterministic: the same inputs give the same bytes.

## `keygen OUT_PRIVATE`

- Makes a new random seed. Read 32 bytes from `/dev/urandom` (the tool runs on the Linux build
  host; no new dependency, so no `rand`). Say this in the crate docs.
- Creates `OUT_PRIVATE` with `create_new` (an existing file is an error and stays untouched) and
  mode 0600 on Unix, set when the file is created (not chmod afterwards). Writes the hex seed
  and `\n`.
- Prints the public key hex and `\n` to stdout, nothing else. Never prints the seed.
  On any error nothing is printed to stdout and a half-written key file is removed.
- `keygen(out) -> Result<String, ReleaseError>` returns the public key hex; `run` prints it.

## `sign --key-env NAME --version X.Y.Z --notes-url U --installer-url U INSTALLER OUT_DIR`

- Reads the key only from the environment variable `NAME` (through the `env` closure in the
  library; `main` passes `std::env::var`). Missing, empty or only-whitespace is
  `KeyEnvMissing(NAME)`; text that is not exactly 64 hex digits is `KeyInvalid(NAME)`. The
  messages name the variable, **never contain any part of the value**.
- Reads `INSTALLER` once: size in bytes and SHA-256 (stream it with a fixed buffer, as
  `verify_file_sha256` does; installers are tens of MB). A missing file is `Io(path, kind)`.
- Builds a `drydock_update::Manifest` (version via `Version::parse`, the two URLs, size, hash),
  checks it by `Manifest::parse(text.as_bytes())` so an invalid version or a non-`https://` URL
  is `BadField` (naming the field) before anything is written.
- Signs `text.as_bytes()`, then verifies the result with `drydock_update::verify` and the
  derived public key (a belt-and-braces check; failure is an internal error, not written).
- Writes `OUT_DIR/manifest` and `OUT_DIR/manifest.sig` (create `OUT_DIR` if missing). Writes
  nothing on any error. The key is never written anywhere (no temp files, no logs); drop the
  `SigningKey` after use (the `zeroize` feature is enabled for that). Prints one line to
  stdout, e.g. `signed manifest for 1.4.0: OUT_DIR/manifest`; nothing about the key.
- Returns the `Manifest`.

## Command line (`run`)

`keygen` and `sign` as above; options of `sign` may come in any order, each exactly once, then
exactly two positional arguments. `--name value` form only. Anything else (no command, unknown
command, unknown or repeated option, missing value, wrong positional count) is
`Usage(message)`; `main` prints `drydock-release: <message>` to stderr and exits 1. Usage
messages list the two commands. Add `ed25519-dalek` signing as needed (`SigningKey::from_bytes`,
`Signer`); it is already a workspace dependency with the `std` and `zeroize` features set in the
crate's `Cargo.toml`. Keep `cargo deny` green (no new crates beyond what `Cargo.lock` has; the
lock file may only gain what these features pull in).

## `app/packaging/release.sh`

New options; all three are given together or not at all (any other mix is exit 2 with a
message, before anything is built):

- `--sign-key-env NAME`: after the installer is built (and Authenticode-signed if configured),
  run the sign step with `"$cargo" run --release --locked -p drydock-release -- sign --key-env NAME --version V --notes-url NOTES --installer-url BASE/drydock-V-setup.exe INSTALLER OUT`
  (same style as the guide build; `OUT` is the output folder, so `manifest` and `manifest.sig`
  land next to the installer). Run it before `SHA256SUMS` is written; `SHA256SUMS` stays as it is.
- `--url-base URL`: where the installer will be served. Strip trailing `/` characters before
  joining. Never a default and never a host in the script: the repository is public, so the
  script must contain no `http://` or `https://` text at all.
- `--notes-url URL`: the release notes page.
- Unless `--dry-run`, check before building anything that the variable `NAME` is set and non-empty;
  if not, print a message naming `NAME` and exit 1. In `--dry-run`, print the sign step with
  `print_cmd` in its place after the installer steps (the variable need not be set). Only the
  variable's name ever appears in output or logs.
- A failing sign step fails the release (`set -e` already does this).
- Add the options to `usage`.

## Public key the app trusts

`app/packaging/update-key.pub` is on main already: comment lines starting with `#`, then one line
with the key. Until Tyrel provides the real public key it holds the throwaway test key
(`crates/drydock-release/tests/data/test-key` is its private half, committed on purpose: it
protects nothing). `m13-update-ui` embeds the file in the build. Do not change the file here.

`app/packaging/INSTALL.md` gets a short section "Updates" in plain words: the program can check
for updates (Windows), checks are signed, and builds made from source or by developers carry a
**test key**, so they do not accept the project's releases and the project's installers do not
accept theirs. Keep the existing wording the other tests check.

## Not in scope

Publishing, the app's embedding of the key, GitHub Releases (`m13-release-publish`, on hold),
Authenticode.

## Where to look

- `drydock_release`: `keygen`, `sign`, `run`, `SignArgs`, `ReleaseError` (skeleton in `crates/drydock-release/src/lib.rs`); `main.rs` calls `run`.
- `drydock_update::Manifest::to_text`, `Manifest::parse`, `Version::parse`, `verify`, `verify_file_sha256`, `hex_encode`, `hex_decode_32` (`show <name> drydock-update`).
- `crates/drydock-update/src/verify.rs` for how the file is streamed and hashed.
- `app/packaging/release.sh`: `sign_file`, `print_cmd`, the option loop and the `dry_run` block.
- Test helpers: `Rig` (`run`, `release`, `pos`, `log_lines`) and `CARGO_STUB` in `crates/drydock/tests/release_script.rs`; `tool`, `signed`, `sign_args` in `crates/drydock-release/tests/release.rs`.
