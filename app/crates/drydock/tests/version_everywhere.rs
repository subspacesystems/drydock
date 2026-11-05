// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m8-version: one version in Help > About, the .exe's file properties and the installer, and
//! the commit in About. See docs/specs/m8-version.md.

use drydock::chrome::{VERSION, about_text};
use drydock::winmeta::{build_version, file_version};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

// ---- build_version ----

#[test]
fn no_request_uses_the_cargo_version() {
    assert_eq!(build_version(None, "0.8.0"), Ok("0.8.0".to_owned()));
}

#[test]
fn blank_request_counts_as_none() {
    assert_eq!(build_version(Some(""), "0.8.0"), Ok("0.8.0".to_owned()));
    assert_eq!(build_version(Some("  \t"), "0.8.0"), Ok("0.8.0".to_owned()));
}

#[test]
fn a_request_wins_and_is_trimmed() {
    assert_eq!(
        build_version(Some("0.8.1"), "0.8.0"),
        Ok("0.8.1".to_owned())
    );
    assert_eq!(
        build_version(Some(" 1.2.3-beta.1 \n"), "0.8.0"),
        Ok("1.2.3-beta.1".to_owned())
    );
    assert_eq!(
        build_version(Some("1.2.3.4"), "0.8.0"),
        Ok("1.2.3.4".to_owned())
    );
}

#[test]
fn a_bad_request_is_an_error_naming_it() {
    for bad in ["abc", "1.2", "1.2.x", "1.2.3.4.5", "70000.0.0"] {
        let err = build_version(Some(bad), "0.8.0").expect_err(bad);
        assert!(err.contains(bad), "{err:?} should name {bad:?}");
    }
}

#[test]
fn the_cargo_version_is_used_as_it_is() {
    assert_eq!(build_version(None, "0.0.0-dev"), Ok("0.0.0-dev".to_owned()));
}

// ---- the version the program shows ----

#[test]
fn about_shows_the_build_version_and_commit() {
    let text = about_text();
    assert!(text.starts_with(&format!("Drydock {VERSION} (")), "{text}");
    assert!(text.contains(drydock::chrome::GIT_HASH), "{text}");
}

#[test]
fn the_banner_shows_the_build_version() {
    assert_eq!(drydock::banner(), format!("Drydock {VERSION}"));
}

#[test]
fn the_build_version_is_the_one_build_rs_emitted() {
    // `DRYDOCK_VERSION` is set for the crate's own build by build.rs.
    assert_eq!(Some(VERSION), option_env!("DRYDOCK_VERSION"));
    assert!(file_version(VERSION).is_some(), "{VERSION}");
}

#[test]
fn plain_builds_are_not_0_0_1() {
    let text = std::fs::read_to_string(crate_dir().join("../../Cargo.toml")).unwrap();
    assert!(
        text.contains("version = \"0.9."),
        "workspace version is 0.9.x"
    );
    assert!(!VERSION.starts_with("0.0."), "{VERSION}");
}

#[test]
fn build_rs_reads_the_variable_and_feeds_the_resource_from_it() {
    let src = std::fs::read_to_string(crate_dir().join("build.rs")).unwrap();
    assert!(
        src.contains("cargo:rerun-if-env-changed=DRYDOCK_VERSION"),
        "rerun when DRYDOCK_VERSION changes"
    );
    assert!(src.contains("cargo:rustc-env=DRYDOCK_VERSION="), "emits it");
    assert!(src.contains("build_version"), "uses winmeta::build_version");
    let resource = src
        .split("CARGO_CFG_TARGET_OS")
        .nth(1)
        .expect("windows block");
    assert!(
        !resource.contains("CARGO_PKG_VERSION"),
        "the version resource must use the build version, not the Cargo one"
    );
}

#[test]
fn no_source_shows_the_cargo_version_directly() {
    for file in ["src/chrome.rs", "src/lib.rs"] {
        let src = std::fs::read_to_string(crate_dir().join(file)).unwrap();
        let shown = src
            .lines()
            .filter(|l| l.contains("format!") && l.contains("CARGO_PKG_VERSION"))
            .count();
        assert_eq!(shown, 0, "{file} formats CARGO_PKG_VERSION");
    }
}

// ---- release.sh exports ----

const CARGO_STUB: &str = r#"#!/usr/bin/env bash
echo "cargo $*" >> "$LOG"
echo "env version=${DRYDOCK_VERSION:-} hash=${DRYDOCK_GIT_HASH:-}" >> "$LOG"
d="$CARGO_TARGET_DIR/x86_64-pc-windows-gnu/release"
mkdir -p "$d"
printf 'stub-exe' > "$d/drydock.exe"
"#;

const MAKENSIS_STUB: &str = r#"#!/usr/bin/env bash
echo "makensis $*" >> "$LOG"
for a in "$@"; do
  case $a in -DOUTFILE=*) out=${a#-DOUTFILE=};; esac
done
printf 'stub-installer' > "$out"
"#;

/// A scratch checkout: `app/packaging/release.sh` copied in, stub programs, optionally a git
/// repository at the top so the script can find a commit.
struct Rig {
    root: PathBuf,
}

impl Rig {
    fn new(name: &str, git: bool) -> Rig {
        let root =
            std::env::temp_dir().join(format!("drydock-version-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let pack = root.join("app/packaging");
        std::fs::create_dir_all(pack.join("windows")).unwrap();
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::create_dir_all(root.join("guide")).unwrap();
        std::fs::copy(
            crate_dir().join("../../packaging/release.sh"),
            pack.join("release.sh"),
        )
        .unwrap();
        std::fs::write(pack.join("INSTALL.md"), "notes").unwrap();
        std::fs::write(pack.join("windows/drydock.nsi"), "").unwrap();
        for (file, body) in [("cargo", CARGO_STUB), ("makensis", MAKENSIS_STUB)] {
            let p = root.join("bin").join(file);
            std::fs::write(&p, body).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        let rig = Rig { root };
        if git {
            rig.git(&["init", "-q"]);
            rig.git(&[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.com",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "x",
            ]);
        }
        rig
    }

    fn git(&self, args: &[&str]) -> String {
        let o = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CEILING_DIRECTORIES", self.root.parent().unwrap())
            .output()
            .expect("git runs");
        assert!(
            o.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8_lossy(&o.stdout).trim().to_owned()
    }

    fn head(&self) -> String {
        self.git(&["rev-parse", "--short", "HEAD"])
    }

    fn log(&self) -> Vec<String> {
        std::fs::read_to_string(self.root.join("log.txt"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn run(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let bin = |n: &str| self.root.join("bin").join(n).to_string_lossy().into_owned();
        let mut cmd = Command::new("bash");
        cmd.arg(self.root.join("app/packaging/release.sh"))
            .args(args)
            .arg("--guide")
            .arg(self.root.join("guide"))
            .arg("--out")
            .arg(self.root.join("out"))
            .env("LOG", self.root.join("log.txt"))
            .env("CARGO", bin("cargo"))
            .env("MAKENSIS", bin("makensis"))
            .env("CARGO_TARGET_DIR", self.root.join("target"))
            .env("GIT_CEILING_DIRECTORIES", self.root.parent().unwrap())
            .env_remove("DRYDOCK_GIT_HASH")
            .env_remove("DRYDOCK_VERSION")
            .env_remove("DRYDOCK_SIGN_PFX");
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.output().expect("bash runs")
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

#[test]
fn the_build_gets_the_release_version_and_the_commit() {
    let rig = Rig::new("build", true);
    let o = rig.run(&["--version", "0.8.1"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    let want = format!("env version=0.8.1 hash={}", rig.head());
    assert!(rig.log().contains(&want), "{want:?} not in {:?}", rig.log());
}

#[test]
fn a_hash_already_in_the_environment_is_kept() {
    let rig = Rig::new("keep", true);
    let o = rig.run(&["--version", "0.8.1"], &[("DRYDOCK_GIT_HASH", "abc1234")]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(
        rig.log()
            .contains(&"env version=0.8.1 hash=abc1234".to_owned()),
        "{:?}",
        rig.log()
    );
}

#[test]
fn without_a_repository_the_release_still_builds_and_leaves_the_hash_unset() {
    let rig = Rig::new("nogit", false);
    let o = rig.run(&["--version", "0.8.1"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(
        rig.log().contains(&"env version=0.8.1 hash=".to_owned()),
        "{:?}",
        rig.log()
    );
}

#[test]
fn the_script_trusts_the_checkout_when_asking_git() {
    let s = std::fs::read_to_string(crate_dir().join("../../packaging/release.sh")).unwrap();
    assert!(
        s.contains("safe.directory"),
        "git is called with -c safe.directory=<repo>"
    );
}

#[test]
fn dry_run_prints_the_version_and_hash_before_the_build() {
    let rig = Rig::new("dry", true);
    let o = rig.run(&["--version", "0.8.1", "--dry-run"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    let lines: Vec<&str> = out.lines().collect();
    let v = lines
        .iter()
        .position(|l| *l == "DRYDOCK_VERSION=0.8.1")
        .expect(&out);
    let h = lines
        .iter()
        .position(|l| *l == format!("DRYDOCK_GIT_HASH={}", rig.head()))
        .expect(&out);
    let b = lines
        .iter()
        .position(|l| l.contains("build --release"))
        .expect(&out);
    assert!(
        v < b && h < b,
        "printed before the cargo build line:\n{out}"
    );
    assert!(rig.log().is_empty(), "dry run runs nothing");
}

#[test]
fn the_default_version_is_the_workspace_one_and_reaches_the_build() {
    let rig = Rig::new("default", true);
    std::fs::write(
        rig.root.join("app/Cargo.toml"),
        "[workspace.package]\nversion = \"0.8.7\"\n",
    )
    .unwrap();
    let o = rig.run(&[], &[]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(
        rig.log()
            .contains(&format!("env version=0.8.7 hash={}", rig.head())),
        "{:?}",
        rig.log()
    );
}
