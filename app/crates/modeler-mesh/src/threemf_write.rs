// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! 3MF writer (`docs/specs/m7-export-3mf.md`).

use std::fmt;
use std::path::Path;

use crate::print_mesh::PrintObject;

/// Why nothing was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreeMfWriteError {
    /// No object has a triangle.
    NothingToWrite,
    /// The file couldn't be written; the system's message.
    Io(String),
}

impl fmt::Display for ThreeMfWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThreeMfWriteError::NothingToWrite => write!(f, "nothing to export"),
            ThreeMfWriteError::Io(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ThreeMfWriteError {}

/// CRC-32 (IEEE 802.3, polynomial 0xEDB88320, as ZIP uses). `crc32(b"123456789")` is
/// `0xCBF43926`.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, entry) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *entry = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc = table[((crc ^ b as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

/// A ZIP archive holding `entries` (name, bytes) in order, every entry stored without
/// compression, with correct CRC-32 and sizes, no extra fields, no data descriptors, a fixed
/// modification time (1980-01-01 00:00) so equal input gives equal bytes.
pub fn zip_stored(entries: &[(&str, &[u8])]) -> Vec<u8> {
    const DATE: u16 = 0x0021; // 1980-01-01
    let mut out = Vec::new();
    let mut cdir = Vec::new();
    for (name, data) in entries {
        let name = name.as_bytes();
        let crc = crc32(data);
        let off = out.len() as u32;
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // stored
        out.extend_from_slice(&0u16.to_le_bytes()); // time
        out.extend_from_slice(&DATE.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // no extra
        out.extend_from_slice(name);
        out.extend_from_slice(data);
        let cd = cdir.len() as u32;
        cdir.extend_from_slice(b"PK\x01\x02");
        cdir.extend_from_slice(&20u16.to_le_bytes()); // made by
        cdir.extend_from_slice(&20u16.to_le_bytes()); // version needed
        cdir.extend_from_slice(&0u16.to_le_bytes()); // flags
        cdir.extend_from_slice(&0u16.to_le_bytes()); // stored
        cdir.extend_from_slice(&0u16.to_le_bytes()); // time
        cdir.extend_from_slice(&DATE.to_le_bytes());
        cdir.extend_from_slice(&crc.to_le_bytes());
        cdir.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cdir.extend_from_slice(&(data.len() as u32).to_le_bytes());
        cdir.extend_from_slice(&(name.len() as u16).to_le_bytes());
        cdir.extend_from_slice(&0u16.to_le_bytes()); // no extra
        cdir.extend_from_slice(&0u16.to_le_bytes()); // no comment
        cdir.extend_from_slice(&0u16.to_le_bytes()); // disk
        cdir.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        cdir.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        cdir.extend_from_slice(&off.to_le_bytes());
        cdir.extend_from_slice(name);
    }
    let cdir_off = out.len() as u32;
    out.extend_from_slice(&cdir);
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&0u16.to_le_bytes()); // disk
    out.extend_from_slice(&0u16.to_le_bytes()); // cdir disk
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(cdir.len() as u32).to_le_bytes());
    out.extend_from_slice(&cdir_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // no comment
    out
}

/// The bytes of a 3MF package for `objects` (millimeters). `NothingToWrite` when no object
/// has a usable triangle.
pub fn encode(objects: &[PrintObject]) -> Result<Vec<u8>, ThreeMfWriteError> {
    let _ = objects;
    unimplemented!("m7-export-3mf")
}

/// `encode`, written to `path` (the parent folder is not created). No file when
/// `NothingToWrite`.
pub fn write(objects: &[PrintObject], path: &Path) -> Result<(), ThreeMfWriteError> {
    let _ = (objects, path);
    unimplemented!("m7-export-3mf")
}
