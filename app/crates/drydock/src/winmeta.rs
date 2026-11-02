// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The text and numbers `build.rs` puts in the Windows .exe's version resource. Pure
//! functions, so they are tested on any OS; `build.rs` includes this file with `#[path]`
//! and so it must not use anything from the rest of the crate.
//!
//! Behaviour: `docs/specs/m8-win-exe.md`.

/// The product name shown in file properties.
pub const PRODUCT_NAME: &str = "Subspace Drydock";
/// The copyright holder.
pub const COMPANY_NAME: &str = "Subspace Systems";
/// The program's file name.
pub const FILE_NAME: &str = "drydock.exe";

/// Four-part Windows file version from a Cargo version: `major.minor.patch` plus an optional
/// fourth number; a `-pre` or `+build` suffix on the last part is ignored. `None` when there
/// are fewer than three parts, more than four, or a part is not a number that fits `u16`.
pub fn file_version(version: &str) -> Option<[u16; 4]> {
    let _ = version;
    unimplemented!("m8-win-exe")
}

/// The four parts as the 64-bit number the version resource stores: the first part in the
/// top 16 bits, the fourth in the bottom 16.
pub fn packed_version(parts: [u16; 4]) -> u64 {
    let _ = parts;
    unimplemented!("m8-win-exe")
}

/// The string entries of the version resource, in this order: `CompanyName`,
/// `FileDescription`, `FileVersion`, `InternalName`, `LegalCopyright`, `OriginalFilename`,
/// `ProductName`, `ProductVersion`. See the spec for the values.
pub fn version_strings(version: &str) -> Vec<(&'static str, String)> {
    let _ = version;
    unimplemented!("m8-win-exe")
}
