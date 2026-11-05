// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Embeds the git commit the binary was built from as `DRYDOCK_GIT_HASH` (Help > About).
//! A `DRYDOCK_GIT_HASH` set in the build environment wins (builds from `git archive` have no
//! `.git`); otherwise `git rev-parse --short HEAD`, else "unknown".

use std::path::Path;
use std::process::Command;

#[path = "src/winmeta.rs"]
mod winmeta;

fn main() {
    println!("cargo:rerun-if-env-changed=DRYDOCK_GIT_HASH");
    println!("cargo:rerun-if-env-changed=DRYDOCK_VERSION");
    let version = winmeta::build_version(
        std::env::var("DRYDOCK_VERSION").ok().as_deref(),
        env!("CARGO_PKG_VERSION"),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    println!("cargo:rustc-env=DRYDOCK_VERSION={version}");
    let guide_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/guide");
    if guide_src.join("SUMMARY.md").exists() {
        let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("guide");
        if out.exists()
            && let Err(error) = std::fs::remove_dir_all(&out)
        {
            panic!("failed to clear {}: {error}", out.display());
        }
        if let Err(error) = drydock_guide::build(&guide_src, &out) {
            panic!("{error}");
        }
        println!("cargo:rustc-env=DRYDOCK_DEV_GUIDE={}", out.display());
        println!("cargo:rerun-if-changed={}", guide_src.display());
    }
    let git_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../.git");
    for file in ["HEAD", "logs/HEAD"] {
        let path = git_dir.join(file);
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    let hash = std::env::var("DRYDOCK_GIT_HASH")
        .ok()
        .filter(|h| !h.trim().is_empty())
        .or_else(|| {
            let out = Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .output()
                .ok()?;
            let hash = String::from_utf8(out.stdout).ok()?.trim().to_owned();
            (out.status.success() && !hash.is_empty()).then_some(hash)
        })
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=DRYDOCK_GIT_HASH={}", hash.trim());

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=assets/drydock.ico");
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/drydock.ico");
        let parts = winmeta::file_version(&version).unwrap_or([0, 0, 0, 0]);
        let packed = winmeta::packed_version(parts);
        resource.set_version_info(winresource::VersionInfo::FILEVERSION, packed);
        resource.set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed);
        for (key, value) in winmeta::version_strings(&version) {
            resource.set(key, &value);
        }
        if let Err(error) = resource.compile() {
            panic!("failed to embed Windows resources: {error}");
        }
    }
}
