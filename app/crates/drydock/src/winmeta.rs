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
    let core = version.split(['-', '+']).next().unwrap_or(version);
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() < 3 || parts.len() > 4 {
        return None;
    }
    let mut numbers = [0u16; 4];
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        numbers[index] = part.parse().ok()?;
    }
    Some(numbers)
}

/// The four parts as the 64-bit number the version resource stores: the first part in the
/// top 16 bits, the fourth in the bottom 16.
pub fn packed_version(parts: [u16; 4]) -> u64 {
    (u64::from(parts[0]) << 48)
        | (u64::from(parts[1]) << 32)
        | (u64::from(parts[2]) << 16)
        | u64::from(parts[3])
}

/// The string entries of the version resource, in this order: `CompanyName`,
/// `FileDescription`, `FileVersion`, `InternalName`, `LegalCopyright`, `OriginalFilename`,
/// `ProductName`, `ProductVersion`. See the spec for the values.
pub fn version_strings(version: &str) -> Vec<(&'static str, String)> {
    let file_version = file_version(version)
        .map(|parts| {
            parts
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(".")
        })
        .unwrap_or_else(|| "0.0.0.0".to_owned());
    vec![
        ("CompanyName", COMPANY_NAME.to_owned()),
        ("FileDescription", PRODUCT_NAME.to_owned()),
        ("FileVersion", file_version),
        ("InternalName", "drydock".to_owned()),
        (
            "LegalCopyright",
            "Copyright (C) 2026 Subspace Systems".to_owned(),
        ),
        ("OriginalFilename", FILE_NAME.to_owned()),
        ("ProductName", PRODUCT_NAME.to_owned()),
        ("ProductVersion", version.to_owned()),
    ]
}
