// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The publish script `packaging/publish.sh` (`docs/specs/m13-release-publish.md`). It runs against
//! a stub `curl` that logs each call and answers from route files, so these tests need only bash,
//! python3 and coreutils, and never touch a network.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const HOST: &str = "https://git.example.test";
const REPO: &str = "acme/drydock";
const TOKEN: &str = "tok-SECRET-1234";
const API: &str = "/api/v1/repos/acme/drydock";

/// Answers one request from `$ROUTES/<method>_<path>[.N]`: first line the HTTP status, the rest the
/// body. `.N` files serve the Nth call of the same route; otherwise the plain file; no file is 599.
const CURL_STUB: &str = r#"#!/usr/bin/env bash
method=GET; outfile=; hdr=; data=; form=
while [ $# -gt 1 ]; do
  case $1 in
    --request) method=$2; shift;;
    --output) outfile=$2; shift;;
    --header) case $2 in @*) hdr=${2#@};; esac; shift;;
    --data) data=$2; shift;;
    --form) form=$2; shift;;
    --write-out) shift;;
  esac
  shift
done
url=${!#}
echo "argv $*" >> "$LOG"
rest=${url#*://}
path=/${rest#*/}
echo "call $method $path" >> "$LOG"
[ -n "$form" ] && echo "form $form" >> "$LOG"
if [ -n "$data" ]; then
  case $data in @*) data=$(tr -d '\n ' < "${data#@}");; esac
  echo "data $data" >> "$LOG"
fi
if [ -z "$hdr" ] || [ "$(stat -c %a "$hdr")" != 600 ] || ! grep -qxF "Authorization: token $EXPECT_TOKEN" "$hdr"; then
  echo "badauth $method $path" >> "$LOG"
  [ -n "$outfile" ] && : > "$outfile"
  printf 401
  exit 0
fi
key=$(printf '%s %s' "$method" "$path" | tr -c 'A-Za-z0-9.-' '_')
mkdir -p "$ROUTES/count"
n=$(( $(cat "$ROUTES/count/$key" 2>/dev/null || echo 0) + 1 ))
echo "$n" > "$ROUTES/count/$key"
f=$ROUTES/$key.$n
[ -f "$f" ] || f=$ROUTES/$key
if [ ! -f "$f" ]; then
  echo "unrouted $method $path" >> "$LOG"
  [ -n "$outfile" ] && : > "$outfile"
  printf 599
  exit 0
fi
[ -n "$outfile" ] && tail -n +2 "$f" > "$outfile"
head -n 1 "$f" | tr -d '\n'
"#;

struct Rig {
    root: PathBuf,
}

impl Rig {
    fn new(name: &str) -> Rig {
        let root =
            std::env::temp_dir().join(format!("drydock-publish-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["bin", "routes", "tmp", "out"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        let p = root.join("bin/curl");
        std::fs::write(&p, CURL_STUB).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let rig = Rig { root };
        rig.write_release_files("1.2.3");
        rig
    }

    fn out(&self) -> PathBuf {
        self.root.join("out")
    }

    /// The five files `release.sh --sign-key-env ...` leaves in the output folder.
    fn write_release_files(&self, v: &str) {
        let o = self.out();
        std::fs::write(o.join(format!("drydock-{v}-setup.exe")), "stub-installer").unwrap();
        std::fs::write(o.join("INSTALL.txt"), "install notes\n").unwrap();
        std::fs::write(o.join("SHA256SUMS"), "00  INSTALL.txt\n").unwrap();
        std::fs::write(o.join("manifest"), manifest_text(v)).unwrap();
        std::fs::write(o.join("manifest.sig"), [7u8; 64]).unwrap();
    }

    /// Serves `status` and `body` for a request (`n` selects the Nth call of the route).
    fn route(&self, method: &str, path: &str, n: Option<u32>, status: u16, body: &str) {
        let mut name = format!("{method} {API}{path}");
        name = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if let Some(n) = n {
            name = format!("{name}.{n}");
        }
        std::fs::write(
            self.root.join("routes").join(name),
            format!("{status}\n{body}"),
        )
        .unwrap();
    }

    /// Like `route`, for a path outside the API (the download URL).
    fn route_raw(&self, method: &str, path: &str, status: u16, body: &str) {
        let name: String = format!("{method} {path}")
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        std::fs::write(
            self.root.join("routes").join(name),
            format!("{status}\n{body}"),
        )
        .unwrap();
    }

    /// Routes for a whole successful publish of 1.2.3 to a channel release that does not exist yet.
    fn happy_routes(&self) {
        self.route("GET", "/releases/tags/v1.2.3", None, 404, "{}");
        self.route("POST", "/releases", Some(1), 201, r#"{"id":11}"#);
        for n in ["drydock-1.2.3-setup.exe", "INSTALL.txt", "SHA256SUMS"] {
            self.route(
                "POST",
                &format!("/releases/11/assets?name={n}"),
                None,
                201,
                "{}",
            );
        }
        self.route("GET", "/releases/tags/channel-dev", None, 404, "{}");
        self.route("POST", "/releases", Some(2), 201, r#"{"id":7}"#);
        self.route("POST", "/releases/7/assets?name=manifest", None, 201, "{}");
        self.route(
            "POST",
            "/releases/7/assets?name=manifest.sig",
            None,
            201,
            "{}",
        );
        self.route_raw(
            "GET",
            "/acme/drydock/releases/download/channel-dev/manifest",
            200,
            &manifest_text("1.2.3"),
        );
    }

    fn log_lines(&self) -> Vec<String> {
        std::fs::read_to_string(self.root.join("log.txt"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// The `METHOD PATH` of every request, in order.
    fn calls(&self) -> Vec<String> {
        self.log_lines()
            .iter()
            .filter_map(|l| l.strip_prefix("call "))
            .map(str::to_string)
            .collect()
    }

    fn publish(&self, args: &[&str]) -> Output {
        self.publish_env(args, &[])
    }

    /// `env` entries with an empty value remove the variable.
    fn publish_env(&self, args: &[&str], env: &[(&str, &str)]) -> Output {
        let out = self.out();
        let mut cmd = Command::new("bash");
        cmd.arg(script())
            .args(["--version", "1.2.3", "--out", out.to_str().unwrap()])
            .args(args)
            .env("LOG", self.root.join("log.txt"))
            .env("ROUTES", self.root.join("routes"))
            .env("CURL", self.root.join("bin/curl"))
            .env("TMPDIR", self.root.join("tmp"))
            .env("EXPECT_TOKEN", TOKEN)
            .env("DRYDOCK_GITEA_URL", HOST)
            .env("DRYDOCK_GITEA_REPO", REPO)
            .env("DRYDOCK_GITEA_TOKEN", TOKEN);
        for (k, v) in env {
            if v.is_empty() {
                cmd.env_remove(k);
            } else {
                cmd.env(k, v);
            }
        }
        cmd.output().expect("bash runs")
    }

    /// Nothing may be left in TMPDIR (the header file with the token is removed).
    fn tmp_is_empty(&self) -> bool {
        std::fs::read_dir(self.root.join("tmp"))
            .unwrap()
            .next()
            .is_none()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/publish.sh")
}

fn manifest_text(v: &str) -> String {
    format!(
        "version = {v}\nnotes_url = {HOST}/{REPO}/releases/tag/v{v}\n\
         installer_url = {HOST}/{REPO}/releases/download/v{v}/drydock-{v}-setup.exe\n\
         size = 14\nsha256 = {}\n",
        "ab".repeat(32)
    )
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

fn api(call: &str) -> String {
    let (m, p) = call.split_once(' ').unwrap();
    format!("{m} {API}{p}")
}

fn no_token(rig: &Rig, o: &Output) {
    assert!(!text(o).contains(TOKEN), "token printed: {}", text(o));
    let log = rig.log_lines().join("\n");
    assert!(!log.contains(TOKEN), "token passed on the command line");
    assert!(rig.tmp_is_empty(), "temporary files left behind");
}

// ---- inputs ----

#[test]
fn missing_or_empty_variables_are_clear_errors_and_call_nothing() {
    for var in [
        "DRYDOCK_GITEA_URL",
        "DRYDOCK_GITEA_REPO",
        "DRYDOCK_GITEA_TOKEN",
    ] {
        let rig = Rig::new("novar");
        rig.happy_routes();
        let o = rig.publish_env(&[], &[(var, "")]);
        assert_eq!(o.status.code(), Some(1), "{var}: {}", text(&o));
        assert!(text(&o).contains(var), "{var} named: {}", text(&o));
        assert!(rig.calls().is_empty(), "{var}: no request was made");
        no_token(&rig, &o);
        // dry run needs them too
        let o = rig.publish_env(&["--dry-run"], &[(var, "")]);
        assert_eq!(o.status.code(), Some(1), "{var} dry run: {}", text(&o));
        assert!(text(&o).contains(var));
    }
}

#[test]
fn the_host_must_be_https_and_the_repo_owner_slash_name() {
    let rig = Rig::new("badvar");
    let o = rig.publish_env(&[], &[("DRYDOCK_GITEA_URL", "http://git.example.test")]);
    assert_eq!(o.status.code(), Some(1));
    assert!(text(&o).contains("DRYDOCK_GITEA_URL"));
    for bad in [
        "acme",
        "acme/",
        "/drydock",
        "a/b/c",
        "acme/dry dock",
        "a/b?x=1",
    ] {
        let o = rig.publish_env(&[], &[("DRYDOCK_GITEA_REPO", bad)]);
        assert_eq!(o.status.code(), Some(1), "{bad:?}");
        assert!(text(&o).contains("DRYDOCK_GITEA_REPO"), "{bad:?}");
    }
    assert!(rig.calls().is_empty());
}

#[test]
fn a_missing_release_file_is_an_error_before_any_request() {
    for file in [
        "drydock-1.2.3-setup.exe",
        "INSTALL.txt",
        "SHA256SUMS",
        "manifest",
        "manifest.sig",
    ] {
        let rig = Rig::new("nofile");
        rig.happy_routes();
        std::fs::remove_file(rig.out().join(file)).unwrap();
        let o = rig.publish(&[]);
        assert_eq!(o.status.code(), Some(1), "{file}: {}", text(&o));
        assert!(text(&o).contains(file), "{file} named: {}", text(&o));
        assert!(rig.calls().is_empty());
        let o = rig.publish(&["--dry-run"]);
        assert_eq!(o.status.code(), Some(1), "{file} dry run");
    }
}

#[test]
fn the_manifest_must_match_the_release_being_published() {
    let rig = Rig::new("badmanifest");
    rig.happy_routes();
    let good = manifest_text("1.2.3");
    for (name, bad, want) in [
        (
            "other version",
            good.replace("version = 1.2.3", "version = 1.2.4"),
            "version",
        ),
        (
            "other installer url",
            good.replace("/download/v1.2.3/", "/download/v9.9.9/"),
            "installer_url",
        ),
        (
            "other host",
            good.replace("git.example.test", "elsewhere.example.test"),
            "installer_url",
        ),
    ] {
        std::fs::write(rig.out().join("manifest"), bad).unwrap();
        let o = rig.publish(&[]);
        assert_eq!(o.status.code(), Some(1), "{name}: {}", text(&o));
        assert!(text(&o).contains(want), "{name}: {}", text(&o));
        assert!(rig.calls().is_empty(), "{name}");
    }
    std::fs::write(rig.out().join("manifest"), good).unwrap();
    std::fs::write(rig.out().join("manifest.sig"), [1u8; 63]).unwrap();
    let o = rig.publish(&[]);
    assert_eq!(o.status.code(), Some(1), "short signature");
    assert!(text(&o).contains("manifest.sig"));
    assert!(rig.calls().is_empty());
}

#[test]
fn bad_version_and_unknown_arguments_are_usage_errors() {
    let rig = Rig::new("usage");
    let o = Command::new("bash")
        .arg(script())
        .args(["--version", "one"])
        .envs([
            ("DRYDOCK_GITEA_URL", HOST),
            ("DRYDOCK_GITEA_REPO", REPO),
            ("DRYDOCK_GITEA_TOKEN", TOKEN),
        ])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2), "{}", text(&o));
    let o = rig.publish(&["--bogus"]);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o));
    let o = rig.publish(&["--channel", "bad name!"]);
    assert_eq!(o.status.code(), Some(2), "{}", text(&o));
    assert!(rig.calls().is_empty());
}

#[test]
fn version_defaults_to_the_workspace_version() {
    let ws =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml"))
            .unwrap();
    let v = ws
        .split("[workspace.package]")
        .nth(1)
        .unwrap()
        .lines()
        .find(|l| l.trim_start().starts_with("version"))
        .unwrap()
        .split('"')
        .nth(1)
        .unwrap()
        .to_string();
    let o = Command::new("bash")
        .arg(script())
        .args(["--print-urls"])
        .envs([("DRYDOCK_GITEA_URL", HOST), ("DRYDOCK_GITEA_REPO", REPO)])
        .output()
        .unwrap();
    ok(&o);
    assert!(
        text(&o).contains(&format!("/releases/download/v{v}")),
        "{}",
        text(&o)
    );
}

// ---- dry run and URLs ----

#[test]
fn dry_run_prints_the_calls_in_order_with_the_token_masked() {
    let rig = Rig::new("dry");
    let o = rig.publish(&["--dry-run"]);
    ok(&o);
    let t = text(&o);
    assert!(rig.calls().is_empty(), "dry run makes no request");
    assert!(!t.contains(TOKEN), "token printed: {t}");
    assert!(t.contains("***"), "token shown masked: {t}");
    let lines: Vec<&str> = t.lines().filter(|l| l.starts_with("+ ")).collect();
    let want = [
        format!("+ GET {HOST}{API}/releases/tags/v1.2.3"),
        format!("+ POST {HOST}{API}/releases"),
        format!("+ POST {HOST}{API}/releases/{{id}}/assets?name=drydock-1.2.3-setup.exe"),
        format!("+ POST {HOST}{API}/releases/{{id}}/assets?name=INSTALL.txt"),
        format!("+ POST {HOST}{API}/releases/{{id}}/assets?name=SHA256SUMS"),
        format!("+ GET {HOST}{API}/releases/tags/channel-dev"),
        format!("+ DELETE {HOST}{API}/releases/{{id}}/assets/{{asset}}"),
        format!("+ POST {HOST}{API}/releases/{{id}}/assets?name=manifest"),
        format!("+ POST {HOST}{API}/releases/{{id}}/assets?name=manifest.sig"),
        format!("+ GET {HOST}/{REPO}/releases/download/channel-dev/manifest"),
    ];
    let mut at = 0;
    for w in &want {
        let found = lines[at..]
            .iter()
            .position(|l| l.starts_with(w.as_str()))
            .unwrap_or_else(|| panic!("missing or out of order: {w}\nin:\n{t}"));
        at += found + 1;
    }
}

#[test]
fn dry_run_uses_the_chosen_channel_tag() {
    let rig = Rig::new("dry-channel");
    let o = rig.publish(&["--dry-run", "--channel", "beta"]);
    ok(&o);
    let t = text(&o);
    assert!(t.contains("/releases/tags/channel-beta"), "{t}");
    assert!(
        t.contains("/releases/download/channel-beta/manifest"),
        "{t}"
    );
    assert!(!t.contains("channel-dev"));
}

#[test]
fn print_urls_gives_the_values_release_sh_needs_and_needs_no_token() {
    let o = Command::new("bash")
        .arg(script())
        .args(["--version", "1.2.3", "--print-urls"])
        .env("DRYDOCK_GITEA_URL", format!("{HOST}//"))
        .env("DRYDOCK_GITEA_REPO", REPO)
        .env_remove("DRYDOCK_GITEA_TOKEN")
        .output()
        .unwrap();
    ok(&o);
    let lines: Vec<String> = String::from_utf8_lossy(&o.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(
        lines,
        vec![
            format!("url-base={HOST}/{REPO}/releases/download/v1.2.3"),
            format!("notes-url={HOST}/{REPO}/releases/tag/v1.2.3"),
        ]
    );
}

// ---- publishing ----

#[test]
fn publishes_the_version_then_replaces_the_channel_manifest() {
    let rig = Rig::new("happy");
    rig.happy_routes();
    let o = rig.publish(&[]);
    ok(&o);
    let want: Vec<String> = [
        "GET /releases/tags/v1.2.3",
        "POST /releases",
        "POST /releases/11/assets?name=drydock-1.2.3-setup.exe",
        "POST /releases/11/assets?name=INSTALL.txt",
        "POST /releases/11/assets?name=SHA256SUMS",
        "GET /releases/tags/channel-dev",
        "POST /releases",
        "POST /releases/7/assets?name=manifest",
        "POST /releases/7/assets?name=manifest.sig",
    ]
    .iter()
    .map(|c| api(c))
    .chain([format!(
        "GET /{REPO}/releases/download/channel-dev/manifest"
    )])
    .collect();
    assert_eq!(rig.calls(), want);
    assert!(
        !rig.log_lines()
            .iter()
            .any(|l| l.starts_with("badauth") || l.starts_with("unrouted")),
        "{:?}",
        rig.log_lines()
    );
    let data: Vec<String> = rig
        .log_lines()
        .into_iter()
        .filter(|l| l.starts_with("data "))
        .collect();
    assert_eq!(data.len(), 2, "one JSON body per created release: {data:?}");
    assert!(data[0].contains(r#""tag_name":"v1.2.3""#), "{}", data[0]);
    assert!(
        data[1].contains(r#""tag_name":"channel-dev""#),
        "{}",
        data[1]
    );
    let forms: Vec<String> = rig
        .log_lines()
        .into_iter()
        .filter(|l| l.starts_with("form "))
        .collect();
    assert_eq!(forms.len(), 5, "{forms:?}");
    for (form, file) in forms.iter().zip([
        "drydock-1.2.3-setup.exe",
        "INSTALL.txt",
        "SHA256SUMS",
        "manifest",
        "manifest.sig",
    ]) {
        assert!(form.contains("attachment=@"), "{form}");
        assert!(form.ends_with(&format!("/{file}")), "{form}");
    }
    no_token(&rig, &o);
}

#[test]
fn an_existing_channel_release_keeps_its_id_and_loses_only_the_old_manifest_files() {
    let rig = Rig::new("channel-exists");
    rig.happy_routes();
    rig.route(
        "GET",
        "/releases/tags/channel-dev",
        None,
        200,
        r#"{"id":7,"assets":[{"id":70,"name":"manifest"},{"id":72,"name":"notes.txt"},{"id":71,"name":"manifest.sig"}]}"#,
    );
    rig.route("DELETE", "/releases/7/assets/70", None, 204, "");
    rig.route("DELETE", "/releases/7/assets/71", None, 204, "");
    let o = rig.publish(&[]);
    ok(&o);
    let calls = rig.calls();
    let tail: Vec<&str> = calls[5..].iter().map(String::as_str).collect();
    assert_eq!(
        tail,
        [
            format!("GET {API}/releases/tags/channel-dev"),
            format!("DELETE {API}/releases/7/assets/70"),
            format!("DELETE {API}/releases/7/assets/71"),
            format!("POST {API}/releases/7/assets?name=manifest"),
            format!("POST {API}/releases/7/assets?name=manifest.sig"),
            format!("GET /{REPO}/releases/download/channel-dev/manifest"),
        ]
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>(),
        "{calls:?}"
    );
    no_token(&rig, &o);
}

#[test]
fn publishing_the_same_version_again_fails_without_changing_anything() {
    let rig = Rig::new("again");
    rig.happy_routes();
    rig.route(
        "GET",
        "/releases/tags/v1.2.3",
        None,
        200,
        r#"{"id":11,"assets":[]}"#,
    );
    let o = rig.publish(&[]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).contains("v1.2.3"), "{}", text(&o));
    assert!(text(&o).to_lowercase().contains("already"), "{}", text(&o));
    assert_eq!(rig.calls(), vec![api("GET /releases/tags/v1.2.3")]);
    no_token(&rig, &o);
}

#[test]
fn an_unexpected_answer_to_the_first_lookup_stops_everything() {
    for status in [500u16, 401, 403] {
        let rig = Rig::new("lookup");
        rig.happy_routes();
        rig.route("GET", "/releases/tags/v1.2.3", None, status, "{}");
        let o = rig.publish(&[]);
        assert_eq!(o.status.code(), Some(1), "{status}: {}", text(&o));
        assert_eq!(rig.calls().len(), 1, "{status}");
    }
}

#[test]
fn a_wrong_token_is_a_failure_and_makes_no_changes() {
    let rig = Rig::new("badtoken");
    rig.happy_routes();
    let o = rig.publish_env(&[], &[("DRYDOCK_GITEA_TOKEN", "someone-elses-token")]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert_eq!(rig.calls().len(), 1);
    assert!(!text(&o).contains("someone-elses-token"));
}

#[test]
fn a_failed_upload_removes_the_half_made_release_and_its_tag_and_leaves_the_channel_alone() {
    let rig = Rig::new("upload-fail");
    rig.happy_routes();
    rig.route(
        "POST",
        "/releases/11/assets?name=SHA256SUMS",
        None,
        500,
        "{}",
    );
    rig.route("DELETE", "/releases/11", None, 204, "");
    rig.route("DELETE", "/tags/v1.2.3", None, 204, "");
    let o = rig.publish(&[]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    let calls = rig.calls();
    assert_eq!(calls[4], api("POST /releases/11/assets?name=SHA256SUMS"));
    assert_eq!(calls[5], api("DELETE /releases/11"));
    assert_eq!(calls[6], api("DELETE /tags/v1.2.3"));
    assert_eq!(calls.len(), 7, "{calls:?}");
    assert!(text(&o).contains("SHA256SUMS"), "{}", text(&o));
    no_token(&rig, &o);
}

#[test]
fn a_failed_channel_update_keeps_the_version_release_and_names_the_retry() {
    let rig = Rig::new("channel-fail");
    rig.happy_routes();
    rig.route(
        "POST",
        "/releases/7/assets?name=manifest.sig",
        None,
        500,
        "{}",
    );
    let o = rig.publish(&[]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(
        !rig.calls().iter().any(|c| c.starts_with("DELETE")),
        "the version release stays: {:?}",
        rig.calls()
    );
    let t = text(&o);
    assert!(t.contains("--channel-only"), "{t}");
    assert!(t.contains("manifest.sig"), "{t}");
    no_token(&rig, &o);
}

#[test]
fn channel_only_skips_the_version_release() {
    let rig = Rig::new("channel-only");
    rig.happy_routes();
    // the channel release is the first one created in this run
    rig.route("POST", "/releases", Some(1), 201, r#"{"id":7}"#);
    let o = rig.publish(&["--channel-only"]);
    ok(&o);
    let calls = rig.calls();
    assert_eq!(calls[0], api("GET /releases/tags/channel-dev"));
    assert!(
        !calls
            .iter()
            .any(|c| c.contains("v1.2.3") || c.contains("/releases/11")),
        "{calls:?}"
    );
    assert_eq!(
        calls.last().unwrap(),
        &format!("GET /{REPO}/releases/download/channel-dev/manifest")
    );
}

#[test]
fn the_published_manifest_is_fetched_back_and_compared() {
    let rig = Rig::new("verify");
    rig.happy_routes();
    rig.route_raw(
        "GET",
        "/acme/drydock/releases/download/channel-dev/manifest",
        200,
        "something else\n",
    );
    let o = rig.publish(&[]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).to_lowercase().contains("manifest"), "{}", text(&o));
    rig.route_raw(
        "GET",
        "/acme/drydock/releases/download/channel-dev/manifest",
        404,
        "",
    );
    let rig2 = Rig::new("verify404");
    rig2.happy_routes();
    rig2.route_raw(
        "GET",
        "/acme/drydock/releases/download/channel-dev/manifest",
        404,
        "",
    );
    let o = rig2.publish(&[]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
}

// ---- the script itself ----

#[test]
fn the_script_has_a_header_and_no_hard_coded_host() {
    let s = std::fs::read_to_string(script()).unwrap();
    assert!(s.contains("SPDX-License-Identifier: GPL-3.0-or-later"));
    assert!(s.contains("Copyright (C) 2026 Subspace Systems"));
    assert!(s.contains("set -euo pipefail"));
    assert!(
        !s.contains("https://"),
        "the host comes from the environment"
    );
    assert!(!s.contains("http://"));
}

#[test]
fn the_script_never_puts_the_token_on_a_command_line() {
    let s = std::fs::read_to_string(script()).unwrap();
    for (i, line) in s.lines().enumerate() {
        let l = line.trim_start();
        if l.starts_with('#') || !l.contains("DRYDOCK_GITEA_TOKEN") {
            continue;
        }
        assert!(
            !l.contains("--header \"Authorization") && !l.contains("-H \"Authorization"),
            "line {}: token inline: {line}",
            i + 1
        );
    }
}
