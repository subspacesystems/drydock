// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Embeds the git commit the binary was built from as `DRYDOCK_GIT_HASH` (Help > About).
//! A `DRYDOCK_GIT_HASH` set in the build environment wins (builds from `git archive` have no
//! `.git`); otherwise `git rev-parse --short HEAD`, else "unknown".

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=DRYDOCK_GIT_HASH");
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
}
