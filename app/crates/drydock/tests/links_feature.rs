// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Regression: hyperlinks (Release notes, About's sponsor link) did nothing because egui-winit
//! was built without its `links` feature, so `open_url_in_browser` only logged. The resolved
//! feature list of the egui-winit that the app uses must include `links`
//! (`docs/specs/m13-update-launch-fix.md`).

use std::path::Path;
use std::process::Command;

#[test]
fn egui_winit_is_built_with_its_links_feature() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let out = Command::new(cargo)
        .args([
            "metadata",
            "--offline",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(&manifest)
        .output()
        .expect("run cargo metadata");
    assert!(
        out.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json = String::from_utf8(out.stdout).expect("utf-8 metadata");
    // The resolve graph has one node per package: {"id":"...egui-winit#0.36.2","dependencies":
    // [...],"deps":[...],"features":["a","b"]}. Read the feature list of that node.
    let nodes = json.find("\"resolve\":").expect("resolve section");
    let rest = &json[nodes..];
    let id = rest
        .find("egui-winit#")
        .or_else(|| rest.find("egui-winit@"))
        .expect("egui-winit in the resolved graph");
    let rest = &rest[id..];
    let start = rest.find("\"features\":[").expect("features list") + "\"features\":[".len();
    let list = &rest[start..start + rest[start..].find(']').expect("end of list")];
    assert!(
        list.split(',').any(|f| f.trim() == "\"links\""),
        "egui-winit resolved features are [{list}]: `links` is off, so hyperlinks do nothing"
    );
}
