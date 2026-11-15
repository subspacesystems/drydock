// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use std::fs::File;
use std::io::Read;
use std::path::Path;

use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::{Manifest, ParseError};

/// Why a manifest and its signature were refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// No signature at all (empty bytes).
    Missing,
    /// Not 64 bytes long (truncated or padded).
    Malformed,
    /// The signature does not match the manifest bytes under this key, or the key itself is
    /// not a valid Ed25519 public key.
    BadSignature,
    /// The signature passed but the manifest text is not valid.
    Parse(ParseError),
}

/// Checks the detached signature over `manifest_bytes` and only then parses them. Order of
/// checks: empty signature is Missing; length other than 64 is Malformed; then the signature
/// (strict verification) is BadSignature on failure; then [`Manifest::parse`], whose error is
/// wrapped in Parse. The manifest is never parsed when the signature fails.
pub fn verify(
    manifest_bytes: &[u8],
    signature: &[u8],
    public_key: &[u8; 32],
) -> Result<Manifest, VerifyError> {
    if signature.is_empty() {
        return Err(VerifyError::Missing);
    }
    let signature: [u8; 64] = signature.try_into().map_err(|_| VerifyError::Malformed)?;
    let key = VerifyingKey::from_bytes(public_key).map_err(|_| VerifyError::BadSignature)?;
    key.verify_strict(manifest_bytes, &Signature::from_bytes(&signature))
        .map_err(|_| VerifyError::BadSignature)?;
    Manifest::parse(manifest_bytes).map_err(VerifyError::Parse)
}

/// Why a downloaded file was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileCheckError {
    /// The file could not be opened or read (missing, permission, ...).
    Io(std::io::ErrorKind),
    /// The file has a different number of bytes than the manifest says (checked before
    /// the hash is reported; a file with both wrong reports SizeMismatch).
    SizeMismatch {
        expected: u64,
        actual: u64,
    },
    HashMismatch,
}

/// Streams the file once (fixed-size buffer, never the whole file in memory), counting bytes
/// and hashing with SHA-256. Ok only when the size and the hash both match. Stops reading
/// as soon as more than `expected_size` bytes have been seen (a huge file is not hashed to the end).
pub fn verify_file_sha256(
    path: &Path,
    expected_sha256: &[u8; 32],
    expected_size: u64,
) -> Result<(), FileCheckError> {
    let mut file = File::open(path).map_err(|e| FileCheckError::Io(e.kind()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut count: u64 = 0;

    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| FileCheckError::Io(e.kind()))?;
        if read == 0 {
            break;
        }
        count += read as u64;
        if count > expected_size {
            return Err(FileCheckError::SizeMismatch {
                expected: expected_size,
                actual: count,
            });
        }
        hasher.update(&buffer[..read]);
    }

    if count != expected_size {
        return Err(FileCheckError::SizeMismatch {
            expected: expected_size,
            actual: count,
        });
    }
    let actual: [u8; 32] = hasher.finalize().into();
    if &actual != expected_sha256 {
        return Err(FileCheckError::HashMismatch);
    }
    Ok(())
}
