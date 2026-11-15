// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The release side of updates: make the update key pair and sign the manifest for a built
//! installer. Behaviour: `docs/specs/m13-release-manifest.md`.

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};

use drydock_update::Manifest;

/// File names written next to each other by [`sign`] in the output folder.
pub const MANIFEST_FILE: &str = "manifest";
pub const SIGNATURE_FILE: &str = "manifest.sig";

/// Why a command failed. `Display` gives one clear line; it never contains key material.
#[derive(Debug)]
pub enum ReleaseError {
    /// Command line misuse (unknown command or option, missing option or argument).
    Usage(String),
    /// The environment variable named for the key is not set (or empty).
    KeyEnvMissing(String),
    /// The variable is set but does not hold 64 hex digits.
    KeyInvalid(String),
    /// The version, a URL or another manifest field is not acceptable.
    BadField(String),
    /// A file could not be read, created or written.
    Io(String, std::io::ErrorKind),
}

impl fmt::Display for ReleaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = f;
        unimplemented!("m13-release-manifest")
    }
}

impl std::error::Error for ReleaseError {}

/// What [`sign`] needs.
#[derive(Clone, Debug)]
pub struct SignArgs {
    /// Name of the environment variable that holds the private key.
    pub key_env: String,
    pub version: String,
    pub notes_url: String,
    pub installer_url: String,
    pub installer: PathBuf,
    pub out_dir: PathBuf,
}

/// Creates a new private key at `out` and returns the matching public key as 64 lower-case
/// hex digits.
pub fn keygen(out: &Path) -> Result<String, ReleaseError> {
    let _ = out;
    unimplemented!("m13-release-manifest")
}

/// Signs the manifest for `args.installer`, writes [`MANIFEST_FILE`] and [`SIGNATURE_FILE`]
/// into `args.out_dir` (created if missing) and returns the manifest.
pub fn sign(
    args: &SignArgs,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Manifest, ReleaseError> {
    let _ = (args, env);
    unimplemented!("m13-release-manifest")
}

/// The command line (without the program name): `keygen OUT_PRIVATE` or
/// `sign --key-env NAME --version X.Y.Z --notes-url U --installer-url U INSTALLER OUT_DIR`.
/// Normal output goes to `stdout`.
pub fn run(
    args: &[String],
    env: &dyn Fn(&str) -> Option<String>,
    stdout: &mut dyn Write,
) -> Result<(), ReleaseError> {
    let _ = (args, env, stdout);
    unimplemented!("m13-release-manifest")
}
