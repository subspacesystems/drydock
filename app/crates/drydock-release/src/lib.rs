// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The release side of updates: make the update key pair and sign the manifest for a built
//! installer. Behaviour: `docs/specs/m13-release-manifest.md`.
//!
//! `keygen` reads its randomness from `/dev/urandom` (the tool runs on the Linux build host),
//! so it needs no random-number crate. Private keys are never written by `sign`: it reads one
//! from the named environment variable and drops the signing key when done.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use drydock_update::{Manifest, ParseError, Version, hex_decode_32, hex_encode};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

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
        match self {
            ReleaseError::Usage(message) => write!(f, "{message}"),
            ReleaseError::KeyEnvMissing(name) => {
                write!(f, "update key environment variable {name} is not set")
            }
            ReleaseError::KeyInvalid(name) => write!(
                f,
                "update key environment variable {name} does not hold 64 hex digits"
            ),
            ReleaseError::BadField(field) => write!(f, "invalid manifest field: {field}"),
            ReleaseError::Io(path, kind) => write!(f, "{path}: {kind:?}"),
        }
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
    let mut seed = [0u8; 32];
    let mut random = File::open("/dev/urandom")
        .map_err(|e| ReleaseError::Io("/dev/urandom".to_string(), e.kind()))?;
    random
        .read_exact(&mut seed)
        .map_err(|e| ReleaseError::Io("/dev/urandom".to_string(), e.kind()))?;

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(out)
        .map_err(|e| ReleaseError::Io(path_text(out), e.kind()))?;

    let line = format!("{}\n", hex_encode(&seed));
    if let Err(e) = file.write_all(line.as_bytes()).and_then(|()| file.flush()) {
        drop(file);
        let _ = fs::remove_file(out);
        return Err(ReleaseError::Io(path_text(out), e.kind()));
    }

    let signing = SigningKey::from_bytes(&seed);
    Ok(hex_encode(&signing.verifying_key().to_bytes()))
}

/// Signs the manifest for `args.installer`, writes [`MANIFEST_FILE`] and [`SIGNATURE_FILE`]
/// into `args.out_dir` (created if missing) and returns the manifest.
pub fn sign(
    args: &SignArgs,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Manifest, ReleaseError> {
    let key_text = env(&args.key_env)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .ok_or_else(|| ReleaseError::KeyEnvMissing(args.key_env.clone()))?;
    let seed =
        hex_decode_32(&key_text).ok_or_else(|| ReleaseError::KeyInvalid(args.key_env.clone()))?;
    let signing = SigningKey::from_bytes(&seed);

    let (size, sha256) = hash_file(&args.installer)?;
    let version = Version::parse(&args.version)
        .ok_or_else(|| ReleaseError::BadField("version".to_string()))?;
    let manifest = Manifest {
        version,
        notes_url: args.notes_url.clone(),
        installer_url: args.installer_url.clone(),
        size,
        sha256,
    };
    let text = manifest.to_text();
    let checked = Manifest::parse(text.as_bytes()).map_err(bad_field)?;
    let text = checked.to_text();

    let signature = signing.sign(text.as_bytes()).to_bytes();
    let public_key = signing.verifying_key().to_bytes();
    drop(signing);
    drydock_update::verify(text.as_bytes(), &signature, &public_key)
        .map_err(|_| ReleaseError::BadField("signature".to_string()))?;

    write_pair(&args.out_dir, text.as_bytes(), &signature)?;
    Ok(checked)
}

/// The command line (without the program name): `keygen OUT_PRIVATE` or
/// `sign --key-env NAME --version X.Y.Z --notes-url U --installer-url U INSTALLER OUT_DIR`.
/// Normal output goes to `stdout`.
pub fn run(
    args: &[String],
    env: &dyn Fn(&str) -> Option<String>,
    stdout: &mut dyn Write,
) -> Result<(), ReleaseError> {
    let Some(command) = args.first() else {
        return Err(ReleaseError::Usage(usage()));
    };
    match command.as_str() {
        "keygen" => {
            let rest = &args[1..];
            if rest.len() != 1 || rest[0].starts_with('-') {
                return Err(ReleaseError::Usage(usage()));
            }
            let out = Path::new(&rest[0]);
            let public = keygen(out)?;
            writeln!(stdout, "{public}")
                .map_err(|e| ReleaseError::Io("stdout".to_string(), e.kind()))
        }
        "sign" => {
            let sign_args = parse_sign(&args[1..])?;
            let manifest = sign(&sign_args, env)?;
            let manifest_path = sign_args.out_dir.join(MANIFEST_FILE);
            writeln!(
                stdout,
                "signed manifest for {}: {}",
                manifest.version,
                manifest_path.display()
            )
            .map_err(|e| ReleaseError::Io("stdout".to_string(), e.kind()))
        }
        _ => Err(ReleaseError::Usage(usage())),
    }
}

fn usage() -> String {
    "usage: drydock-release keygen OUT_PRIVATE\n       drydock-release sign --key-env NAME --version X.Y.Z --notes-url U --installer-url U INSTALLER OUT_DIR".to_string()
}

fn parse_sign(args: &[String]) -> Result<SignArgs, ReleaseError> {
    const OPTIONS: [&str; 4] = ["--key-env", "--version", "--notes-url", "--installer-url"];
    let mut key_env = None;
    let mut version = None;
    let mut notes_url = None;
    let mut installer_url = None;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if positional.is_empty() && arg.starts_with("--") {
            let slot = match arg.as_str() {
                "--key-env" => &mut key_env,
                "--version" => &mut version,
                "--notes-url" => &mut notes_url,
                "--installer-url" => &mut installer_url,
                _ => return Err(ReleaseError::Usage(usage())),
            };
            if slot.is_some() {
                return Err(ReleaseError::Usage(usage()));
            }
            let value = args
                .get(index + 1)
                .ok_or_else(|| ReleaseError::Usage(usage()))?;
            if OPTIONS.contains(&value.as_str()) {
                return Err(ReleaseError::Usage(usage()));
            }
            *slot = Some(value.clone());
            index += 2;
        } else if arg.starts_with("--") {
            return Err(ReleaseError::Usage(usage()));
        } else {
            positional.push(arg.clone());
            index += 1;
        }
    }
    if positional.len() != 2 {
        return Err(ReleaseError::Usage(usage()));
    }
    Ok(SignArgs {
        key_env: key_env.ok_or_else(|| ReleaseError::Usage(usage()))?,
        version: version.ok_or_else(|| ReleaseError::Usage(usage()))?,
        notes_url: notes_url.ok_or_else(|| ReleaseError::Usage(usage()))?,
        installer_url: installer_url.ok_or_else(|| ReleaseError::Usage(usage()))?,
        installer: PathBuf::from(&positional[0]),
        out_dir: PathBuf::from(&positional[1]),
    })
}

fn path_text(path: &Path) -> String {
    path.display().to_string()
}

fn hash_file(path: &Path) -> Result<(u64, [u8; 32]), ReleaseError> {
    let mut file = File::open(path).map_err(|e| ReleaseError::Io(path_text(path), e.kind()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut size: u64 = 0;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| ReleaseError::Io(path_text(path), e.kind()))?;
        if read == 0 {
            break;
        }
        size = size
            .checked_add(read as u64)
            .ok_or_else(|| ReleaseError::BadField("size".to_string()))?;
        hasher.update(&buffer[..read]);
    }
    Ok((size, hasher.finalize().into()))
}

fn bad_field(error: ParseError) -> ReleaseError {
    match error {
        ParseError::Invalid(field) | ParseError::Missing(field) | ParseError::Duplicate(field) => {
            ReleaseError::BadField(field.to_string())
        }
        ParseError::NotUtf8 | ParseError::Syntax { .. } => {
            ReleaseError::BadField("manifest".to_string())
        }
    }
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_path(out_dir: &Path, name: &str) -> PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    out_dir.join(format!(".{name}.{}.{}.tmp", std::process::id(), n))
}

fn write_pair(out_dir: &Path, manifest: &[u8], signature: &[u8]) -> Result<(), ReleaseError> {
    fs::create_dir_all(out_dir).map_err(|e| ReleaseError::Io(path_text(out_dir), e.kind()))?;
    let manifest_tmp = temp_path(out_dir, MANIFEST_FILE);
    let signature_tmp = temp_path(out_dir, SIGNATURE_FILE);
    let result = (|| {
        write_temp(&manifest_tmp, manifest)?;
        write_temp(&signature_tmp, signature)?;
        fs::rename(&manifest_tmp, out_dir.join(MANIFEST_FILE))
            .map_err(|e| ReleaseError::Io(path_text(&manifest_tmp), e.kind()))?;
        fs::rename(&signature_tmp, out_dir.join(SIGNATURE_FILE))
            .map_err(|e| ReleaseError::Io(path_text(&signature_tmp), e.kind()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&manifest_tmp);
        let _ = fs::remove_file(&signature_tmp);
        let _ = fs::remove_file(out_dir.join(MANIFEST_FILE));
        let _ = fs::remove_file(out_dir.join(SIGNATURE_FILE));
    }
    result
}

fn write_temp(path: &Path, bytes: &[u8]) -> Result<(), ReleaseError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| ReleaseError::Io(path_text(path), e.kind()))?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .map_err(|e| ReleaseError::Io(path_text(path), e.kind()))
}
