// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The release script `packaging/release.sh` and `packaging/INSTALL.md`
//! (`docs/specs/m8-release.md`). The script runs against stub `cargo`, `makensis` and
//! `osslsigncode` programs that log their arguments, so these tests need only bash and
//! coreutils (`sha256sum`), not a Windows toolchain.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn packaging() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging")
}

fn script() -> PathBuf {
    packaging().join("release.sh")
}

/// The workspace version in `app/Cargo.toml` (first `version = "..."` after `[workspace.package]`).
fn workspace_version() -> String {
    let text =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml"))
            .expect("app/Cargo.toml");
    let rest = text
        .split("[workspace.package]")
        .nth(1)
        .expect("workspace.package");
    let line = rest
        .lines()
        .find(|l| l.trim_start().starts_with("version"))
        .expect("version");
    line.split('"').nth(1).expect("quoted version").to_string()
}

const CARGO_STUB: &str = r#"#!/usr/bin/env bash
echo "cargo $*" >> "$LOG"
[ "${STUB_CARGO_NO_EXE:-}" = 1 ] && exit 0
d="$CARGO_TARGET_DIR/x86_64-pc-windows-gnu/release"
mkdir -p "$d"
printf 'stub-exe' > "$d/drydock.exe"
"#;

const MAKENSIS_STUB: &str = r#"#!/usr/bin/env bash
echo "makensis $*" >> "$LOG"
[ "${STUB_MAKENSIS_FAIL:-}" = 1 ] && exit 1
for a in "$@"; do
  case $a in
    -DOUTFILE=*) out=${a#-DOUTFILE=};;
    -DSTAGE=*) stage=${a#-DSTAGE=};;
    -DVERSION=*) v=${a#-DVERSION=};;
  esac
done
[ -f "$stage/drydock.exe" ] && echo "stage_has_exe" >> "$LOG"
[ -f "$stage/drydock.exe" ] && echo "stage_exe=$(cat "$stage/drydock.exe")" >> "$LOG"
[ -d "$stage/guide" ] && echo "stage_has_guide" >> "$LOG"
printf 'stub-installer %s' "$v" > "$out"
"#;

const SIGN_STUB: &str = r#"#!/usr/bin/env bash
echo "osslsigncode $*" >> "$LOG"
while [ $# -gt 0 ]; do
  case $1 in
    -in) in=$2; shift;;
    -out) out=$2; shift;;
  esac
  shift
done
tmp=$(mktemp)
{ cat "$in"; printf -- '-signed'; } > "$tmp"
mv "$tmp" "$out"
"#;

/// A scratch folder with the three stub programs, a log file and an output folder.
struct Rig {
    root: PathBuf,
}

impl Rig {
    fn new(name: &str) -> Rig {
        let root =
            std::env::temp_dir().join(format!("drydock-release-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("bin")).unwrap();
        for (file, body) in [
            ("cargo", CARGO_STUB),
            ("makensis", MAKENSIS_STUB),
            ("osslsigncode", SIGN_STUB),
        ] {
            let p = root.join("bin").join(file);
            std::fs::write(&p, body).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        Rig { root }
    }

    fn out(&self) -> PathBuf {
        self.root.join("out")
    }

    fn log(&self) -> PathBuf {
        self.root.join("log.txt")
    }

    fn log_lines(&self) -> Vec<String> {
        std::fs::read_to_string(self.log())
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn pos(&self, prefix: &str) -> Option<usize> {
        self.log_lines().iter().position(|l| l.starts_with(prefix))
    }

    /// Runs the script with the stubs; `env` adds or overrides variables.
    fn run(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let bin = |n: &str| self.root.join("bin").join(n).to_string_lossy().into_owned();
        let mut cmd = Command::new("bash");
        cmd.arg(script())
            .args(args)
            .env("LOG", self.log())
            .env("CARGO", bin("cargo"))
            .env("MAKENSIS", bin("makensis"))
            .env("OSSLSIGNCODE", bin("osslsigncode"))
            .env("CARGO_TARGET_DIR", self.root.join("target"))
            .env_remove("DRYDOCK_SIGN_PFX")
            .env_remove("DRYDOCK_SIGN_PASS_FILE");
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.output().expect("bash runs")
    }

    fn release(&self, version: &str) -> Output {
        let out = self.out();
        self.run(&["--version", version, "--out", out.to_str().unwrap()], &[])
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

fn ok(o: &Output) {
    assert!(o.status.success(), "script failed: {}", text(o));
}

fn checksum_names(out: &Path) -> Vec<String> {
    let sums = std::fs::read_to_string(out.join("SHA256SUMS")).expect("SHA256SUMS");
    let mut names: Vec<String> = sums
        .lines()
        .map(|l| {
            let (hash, name) = l
                .split_once("  ")
                .unwrap_or_else(|| panic!("bad line {l:?}"));
            assert_eq!(hash.len(), 64, "hash in {l:?}");
            assert!(
                hash.chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
                "hash in {l:?}"
            );
            name.to_string()
        })
        .collect();
    names.sort();
    names
}

// ---- the script file ----

#[test]
fn script_has_header_and_runs_strict() {
    let s = std::fs::read_to_string(script()).expect("app/packaging/release.sh exists");
    let lines: Vec<&str> = s.lines().take(4).collect();
    assert!(
        lines[0].starts_with("#!") && lines[0].contains("bash"),
        "shebang: {:?}",
        lines[0]
    );
    assert_eq!(lines[1], "# SPDX-License-Identifier: GPL-3.0-or-later");
    assert_eq!(lines[2], "# Copyright (C) 2026 Subspace Systems");
    assert!(s.contains("set -euo pipefail"), "strict mode");
    assert!(
        !s.to_lowercase().contains("modeler"),
        "no codename in the script"
    );
}

#[cfg(unix)]
#[test]
fn script_is_executable() {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(script())
        .expect("release.sh")
        .permissions()
        .mode();
    assert!(
        mode & 0o111 != 0,
        "release.sh must be executable (mode {mode:o})"
    );
}

// ---- build ----

#[test]
fn builds_the_windows_target_with_cargo() {
    let rig = Rig::new("cargo");
    ok(&rig.release("1.2.3"));
    let line = rig
        .log_lines()
        .into_iter()
        .find(|l| l.starts_with("cargo "))
        .expect("cargo was run");
    for want in [
        "build",
        "--release",
        "--locked",
        "--target x86_64-pc-windows-gnu",
        "-p drydock",
    ] {
        assert!(line.contains(want), "cargo line {line:?} lacks {want:?}");
    }
}

#[test]
fn missing_exe_after_build_fails() {
    let rig = Rig::new("noexe");
    let out = rig.out();
    let o = rig.run(
        &["--version", "1.2.3", "--out", out.to_str().unwrap()],
        &[("STUB_CARGO_NO_EXE", "1")],
    );
    assert!(!o.status.success());
    assert!(
        text(&o).contains("drydock.exe"),
        "message names the file: {}",
        text(&o)
    );
    assert!(
        rig.pos("makensis").is_none(),
        "no installer is built without the program"
    );
    assert!(!out.join("SHA256SUMS").exists());
}

// ---- installer ----

#[test]
fn makensis_gets_version_stage_and_outfile() {
    let rig = Rig::new("nsis");
    ok(&rig.release("1.2.3"));
    let line = rig
        .log_lines()
        .into_iter()
        .find(|l| l.starts_with("makensis "))
        .expect("makensis ran");
    let args: Vec<&str> = line.split(' ').skip(1).collect();
    assert!(args.contains(&"-DVERSION=1.2.3"), "{line}");
    let outfile = format!(
        "-DOUTFILE={}",
        rig.out().join("drydock-1.2.3-setup.exe").display()
    );
    assert!(args.contains(&outfile.as_str()), "{line} lacks {outfile}");
    let stage = args
        .iter()
        .find_map(|a| a.strip_prefix("-DSTAGE="))
        .expect("-DSTAGE");
    assert!(Path::new(stage).is_absolute(), "STAGE is absolute: {stage}");
    let last = args.last().unwrap();
    assert!(
        last.ends_with("packaging/windows/drydock.nsi"),
        "script path last: {last}"
    );
    assert!(Path::new(last).exists(), "installer script exists: {last}");
    assert!(
        rig.pos("stage_has_exe").is_some(),
        "stage held drydock.exe when makensis ran"
    );
    assert!(
        rig.pos("stage_has_guide").is_none(),
        "no guide folder unless asked"
    );
}

#[test]
fn makensis_failure_stops_the_release() {
    let rig = Rig::new("nsisfail");
    let out = rig.out();
    let o = rig.run(
        &["--version", "1.2.3", "--out", out.to_str().unwrap()],
        &[("STUB_MAKENSIS_FAIL", "1")],
    );
    assert!(!o.status.success());
    assert!(!out.join("SHA256SUMS").exists());
}

#[test]
fn guide_folder_goes_into_the_stage() {
    let rig = Rig::new("guide");
    let guide = rig.root.join("guide-in");
    std::fs::create_dir_all(&guide).unwrap();
    std::fs::write(guide.join("index.html"), "<p>hi</p>").unwrap();
    let out = rig.out();
    let o = rig.run(
        &[
            "--version",
            "1.2.3",
            "--out",
            out.to_str().unwrap(),
            "--guide",
            guide.to_str().unwrap(),
        ],
        &[],
    );
    ok(&o);
    assert!(rig.pos("stage_has_guide").is_some());
}

#[test]
fn missing_guide_folder_is_an_error() {
    let rig = Rig::new("noguide");
    let out = rig.out();
    let nope = rig.root.join("nope");
    let o = rig.run(
        &[
            "--version",
            "1.2.3",
            "--out",
            out.to_str().unwrap(),
            "--guide",
            nope.to_str().unwrap(),
        ],
        &[],
    );
    assert!(!o.status.success());
    assert!(rig.pos("cargo").is_none(), "checked before building");
}

// ---- artifacts ----

#[test]
fn writes_installer_install_notes_and_checksums() {
    let rig = Rig::new("artifacts");
    ok(&rig.release("1.2.3"));
    let out = rig.out();
    assert_eq!(
        std::fs::read_to_string(out.join("drydock-1.2.3-setup.exe")).unwrap(),
        "stub-installer 1.2.3"
    );
    let notes = std::fs::read(out.join("INSTALL.txt")).expect("INSTALL.txt in the output");
    assert_eq!(
        notes,
        std::fs::read(packaging().join("INSTALL.md")).unwrap(),
        "copy of INSTALL.md"
    );
    assert_eq!(
        checksum_names(&out),
        ["INSTALL.txt", "drydock-1.2.3-setup.exe"]
    );
    let check = Command::new("sha256sum")
        .arg("-c")
        .arg("SHA256SUMS")
        .current_dir(&out)
        .output()
        .unwrap();
    assert!(check.status.success(), "sha256sum -c: {}", text(&check));
}

#[test]
fn version_defaults_to_the_workspace_version() {
    let rig = Rig::new("defver");
    let out = rig.out();
    ok(&rig.run(&["--out", out.to_str().unwrap()], &[]));
    let v = workspace_version();
    assert!(
        out.join(format!("drydock-{v}-setup.exe")).exists(),
        "installer for {v}"
    );
    assert!(
        rig.pos("makensis")
            .is_some_and(|i| rig.log_lines()[i].contains(&format!("-DVERSION={v}")))
    );
}

#[test]
fn bad_version_is_rejected_before_anything_runs() {
    for bad in ["abc", "1.2", "1.2.3.4", "", "1.2.3 beta", "../1.2.3"] {
        let rig = Rig::new("badver");
        let out = rig.out();
        let o = rig.run(&["--version", bad, "--out", out.to_str().unwrap()], &[]);
        assert!(!o.status.success(), "{bad:?} accepted");
        assert!(
            text(&o).to_lowercase().contains("version"),
            "{bad:?}: {}",
            text(&o)
        );
        assert!(!rig.log().exists(), "{bad:?}: nothing may run");
    }
}

#[test]
fn prerelease_versions_are_accepted() {
    let rig = Rig::new("pre");
    ok(&rig.release("1.2.3-beta.1"));
    assert!(rig.out().join("drydock-1.2.3-beta.1-setup.exe").exists());
}

#[test]
fn a_second_run_into_the_same_folder_works_and_lists_each_file_once() {
    let rig = Rig::new("twice");
    ok(&rig.release("1.2.3"));
    ok(&rig.release("1.2.3"));
    assert_eq!(
        checksum_names(&rig.out()),
        ["INSTALL.txt", "drydock-1.2.3-setup.exe"]
    );
}

// ---- signing ----

#[test]
fn unsigned_by_default_and_says_so() {
    let rig = Rig::new("unsigned");
    let o = rig.release("1.2.3");
    ok(&o);
    assert!(
        rig.pos("osslsigncode").is_none(),
        "no signer call without a certificate"
    );
    assert!(
        text(&o).to_lowercase().contains("signing skipped"),
        "{}",
        text(&o)
    );
}

#[test]
fn signs_the_program_then_the_installer_when_a_certificate_is_configured() {
    let rig = Rig::new("signed");
    let pfx = rig.root.join("cert.pfx");
    let pass = rig.root.join("pass.txt");
    std::fs::write(&pfx, "x").unwrap();
    std::fs::write(&pass, "secret\n").unwrap();
    let out = rig.out();
    let o = rig.run(
        &["--version", "1.2.3", "--out", out.to_str().unwrap()],
        &[
            ("DRYDOCK_SIGN_PFX", pfx.to_str().unwrap()),
            ("DRYDOCK_SIGN_PASS_FILE", pass.to_str().unwrap()),
        ],
    );
    ok(&o);
    let lines = rig.log_lines();
    let signs: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("osslsigncode "))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(signs.len(), 2, "two signer calls: {lines:?}");
    let nsis = rig.pos("makensis").unwrap();
    assert!(
        signs[0] < nsis && nsis < signs[1],
        "program, then installer: {lines:?}"
    );
    assert!(
        lines[signs[0]].contains("drydock.exe"),
        "{}",
        lines[signs[0]]
    );
    assert!(
        lines[signs[1]].contains("drydock-1.2.3-setup.exe"),
        "{}",
        lines[signs[1]]
    );
    for l in [&lines[signs[0]], &lines[signs[1]]] {
        for want in [
            "sign",
            "-pkcs12",
            pfx.to_str().unwrap(),
            "-readpass",
            pass.to_str().unwrap(),
            "-in",
            "-out",
        ] {
            assert!(l.contains(want), "{l:?} lacks {want:?}");
        }
        assert!(
            !l.contains("secret"),
            "the password never goes on a command line"
        );
    }
    assert!(
        lines.contains(&"stage_exe=stub-exe-signed".to_string()),
        "installer packs the signed program"
    );
    assert_eq!(
        std::fs::read_to_string(out.join("drydock-1.2.3-setup.exe")).unwrap(),
        "stub-installer 1.2.3-signed"
    );
    let check = Command::new("sha256sum")
        .arg("-c")
        .arg("SHA256SUMS")
        .current_dir(&out)
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "checksums are of the signed installer: {}",
        text(&check)
    );
    assert!(!text(&o).to_lowercase().contains("signing skipped"));
}

#[test]
fn configured_but_missing_certificate_fails_before_building() {
    let rig = Rig::new("nopfx");
    let out = rig.out();
    let nope = rig.root.join("nope.pfx");
    let o = rig.run(
        &["--version", "1.2.3", "--out", out.to_str().unwrap()],
        &[("DRYDOCK_SIGN_PFX", nope.to_str().unwrap())],
    );
    assert!(!o.status.success());
    assert!(text(&o).contains("nope.pfx"), "{}", text(&o));
    assert!(rig.pos("cargo").is_none());
}

#[test]
fn empty_certificate_variable_counts_as_not_configured() {
    let rig = Rig::new("emptypfx");
    let out = rig.out();
    let o = rig.run(
        &["--version", "1.2.3", "--out", out.to_str().unwrap()],
        &[("DRYDOCK_SIGN_PFX", "")],
    );
    ok(&o);
    assert!(rig.pos("osslsigncode").is_none());
}

// ---- dry run ----

#[test]
fn dry_run_prints_the_plan_and_runs_nothing() {
    let rig = Rig::new("dry");
    let out = rig.out();
    let o = rig.run(
        &[
            "--dry-run",
            "--version",
            "1.2.3",
            "--out",
            out.to_str().unwrap(),
        ],
        &[],
    );
    ok(&o);
    let stdout = String::from_utf8_lossy(&o.stdout).into_owned();
    let plan: Vec<&str> = stdout.lines().filter(|l| l.starts_with("+ ")).collect();
    assert!(plan.len() >= 2, "plan lines: {stdout}");
    let cargo = plan
        .iter()
        .position(|l| l.contains("x86_64-pc-windows-gnu"))
        .expect("cargo step");
    let nsis = plan
        .iter()
        .position(|l| l.contains("drydock-1.2.3-setup.exe") && l.contains("-DVERSION=1.2.3"))
        .expect("makensis step");
    assert!(cargo < nsis, "build before installer: {stdout}");
    assert!(!rig.log().exists(), "no program was run");
    assert!(!out.join("SHA256SUMS").exists() && !out.join("drydock-1.2.3-setup.exe").exists());
}

#[test]
fn dry_run_with_certificate_lists_the_signing_steps() {
    let rig = Rig::new("drysign");
    let pfx = rig.root.join("cert.pfx");
    std::fs::write(&pfx, "x").unwrap();
    let out = rig.out();
    let o = rig.run(
        &[
            "--dry-run",
            "--version",
            "1.2.3",
            "--out",
            out.to_str().unwrap(),
        ],
        &[("DRYDOCK_SIGN_PFX", pfx.to_str().unwrap())],
    );
    ok(&o);
    let stdout = String::from_utf8_lossy(&o.stdout).into_owned();
    assert_eq!(
        stdout
            .lines()
            .filter(|l| l.starts_with("+ ") && l.contains("osslsigncode"))
            .count(),
        2,
        "{stdout}"
    );
    assert!(!rig.log().exists());
}

// ---- install notes ----

fn install_notes() -> String {
    std::fs::read_to_string(packaging().join("INSTALL.md"))
        .expect("app/packaging/INSTALL.md exists")
}

#[test]
fn install_notes_cover_smartscreen_and_smart_app_control() {
    let t = install_notes();
    for want in [
        "Subspace Drydock",
        "SmartScreen",
        "More info",
        "Run anyway",
        "Smart App Control",
        "SHA256SUMS",
        "Uninstall",
    ] {
        assert!(t.contains(want), "INSTALL.md lacks {want:?}");
    }
    assert!(
        t.to_lowercase().contains("unsigned"),
        "says the build is unsigned"
    );
    assert!(
        t.to_lowercase().contains("upgrade") || t.to_lowercase().contains("older version"),
        "mentions upgrading"
    );
}

#[test]
fn install_notes_use_only_the_product_name() {
    let t = install_notes().to_lowercase();
    assert!(!t.contains("modeler"), "codename in INSTALL.md");
    assert!(
        !t.contains("sketchup") && !t.contains("trimble"),
        "other products named"
    );
}
