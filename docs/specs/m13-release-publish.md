# m13-release-publish: publish a release to this repository's Gitea releases

> Public releases now go to GitHub Releases instead, through `.github/workflows/release.yml` and
> `app/packaging/sign-github-release.sh` ([docs/releasing.md](../releasing.md)). `publish.sh`
> stays for a Gitea server's releases.

The last step of the release side of M13. `release.sh` already builds the installer and writes
`manifest` and `manifest.sig` next to it (`m13-release-manifest`). This task adds
`app/packaging/publish.sh`, which uploads them. Publish to this repository's Gitea releases
**only**, nowhere else (project decision). Nothing is hard-coded: the repository is public.

Tests: `crates/drydock/tests/publish_script.rs`. They must pass unchanged. They run the script
against a stub `curl` (read the stub at the top of the file: it fixes the form of every `curl`
call) and need only bash, python3 and coreutils. No `jq` (not in the worker image): read JSON with
`python3 -I -c`. No network.

## Where things go

A release `vX.Y.Z` holds `drydock-X.Y.Z-setup.exe`, `INSTALL.txt`, `SHA256SUMS`. A fixed release
with tag `channel-NAME` (default NAME `dev`) holds the current `manifest` and `manifest.sig`; the
channel's manifest URL therefore never changes:
`HOST/OWNER/REPO/releases/download/channel-NAME/manifest`. The manifest's `installer_url` points at
the per-version asset: `HOST/OWNER/REPO/releases/download/vX.Y.Z/drydock-X.Y.Z-setup.exe`.
These are the plain release-download URLs, which accept the token header the client sends; the
script checks that for real at the end (see Verify). If they do not, that check fails and the
script says so; it does not guess another URL form.

## Command line

`publish.sh [--version X.Y.Z] [--out DIR] [--channel NAME] [--channel-only] [--dry-run] [--print-urls]`

- `--version`: as in `release.sh` (same default from `app/Cargo.toml`, same check, exit 2 when
  bad). `--out`: the folder `release.sh` wrote to, default `app/dist` (as `release.sh`).
- `--channel NAME`: letters, digits, `-`, `_`, `.` only, else exit 2. Default `dev`.
- Unknown argument: exit 2. `-h`/`--help`: usage, exit 0.
- `--print-urls`: prints exactly two lines to stdout and exits 0, needing only the URL and repo
  variables (no token, no files):
  `url-base=HOST/OWNER/REPO/releases/download/vX.Y.Z` and
  `notes-url=HOST/OWNER/REPO/releases/tag/vX.Y.Z`. These are the values for `release.sh
  --url-base/--notes-url`, so the manifest it signs matches what this script publishes. Strip
  trailing `/` characters from the host before joining.
- Environment (never in the script, never defaulted): `DRYDOCK_GITEA_URL` (must start with
  `https:` followed by two slashes and a host; the script's own text must not contain a literal
  `http`-scheme URL, a test greps for it: build the check with a regex such as
  `^https:[/][/][^/]`), `DRYDOCK_GITEA_REPO` (`owner/name`, one slash, characters `A-Za-z0-9._-`
  only) and `DRYDOCK_GITEA_TOKEN`. Missing, empty or malformed: exit 1 with a message naming the
  variable (never its value). Checked before anything else except usage errors, also in
  `--dry-run`. `CURL` overrides the `curl` program (tests); default `curl`.

## Checks before any request (exit 1, message names the file or field)

- The five files exist in `--out`: `drydock-X.Y.Z-setup.exe`, `INSTALL.txt`, `SHA256SUMS`,
  `manifest`, `manifest.sig` (with `--channel-only` only the last two).
- `manifest` contains the exact lines `version = X.Y.Z` and
  `installer_url = <the url-base above>/drydock-X.Y.Z-setup.exe` (a manifest signed for another
  version or host is caught here). Do not parse more of it; the signature is already checked by
  the tool that wrote it.
- `manifest.sig` is exactly 64 bytes.

## `--dry-run`

Runs the checks, makes no request, prints the plan: one line per request in the order below,
each `+ METHOD URL` (full URL; a release id or asset id not yet known is the literal `{id}` or
`{asset}`), uploads with ` (file PATH)` appended; and one line saying the token is sent as
`Authorization: token ***`. The token itself never appears anywhere in output.

## Publishing (all requests use `curl`)

API base is `HOST/api/v1/repos/OWNER/REPO`. Every request is exactly of this form (the stub parses
it): `"$curl" --silent --show-error --request METHOD --header @HEADERFILE --output BODYFILE
--write-out '%{http_code}' [--data @JSONFILE | --form attachment=@PATH] URL` (URL last; you may
add `--location`, `--max-time`, `--connect-timeout` before it). `HEADERFILE` is made with
`mktemp` (under `TMPDIR`), mode 0600 *before* the token is written, holds the single line
`Authorization: token <token>`, and is removed on every exit (`trap`). So the token is never in
the argument list or in output. `--data` also uses a file for a JSON body (`Content-Type` header
goes in the header file as a second line, for those requests only). A status outside 2xx (except
where said) is a failure: print the request, the status and the response body's first 200
characters (the token is not in it) to stderr and exit 1.

Steps, in this order:

1. `GET /releases/tags/vX.Y.Z`. **404**: go on. **200**: exit 1, "release vX.Y.Z already exists;
   not overwriting". Anything else: failure. Nothing is changed in either case.
2. `POST /releases` with the compact JSON `{"tag_name":"vX.Y.Z","name":"Drydock X.Y.Z"}`
   (add `"target_commitish"` only when the env var `DRYDOCK_RELEASE_COMMIT` is set, and then
   only if it is `[0-9a-f]{7,40}` or a plain branch name `A-Za-z0-9._/-`). Read `id` from the
   response.
3. Upload the installer, `INSTALL.txt`, `SHA256SUMS` in that order:
   `POST /releases/ID/assets?name=FILE` with `--form attachment=@PATH`.
   If any request in steps 2-3 fails, try to undo: `DELETE /releases/ID`, then
   `DELETE /tags/vX.Y.Z` (ignore failures of these two, but say what was left), and exit 1 with the
   original failure. A retry is then possible.
4. `GET /releases/tags/channel-NAME`. **404**: `POST /releases` with
   `{"tag_name":"channel-NAME","name":"Update channel NAME"}` and read its `id`. **200**: read
   `id` and the `assets` array.
5. For every existing asset named exactly `manifest` or `manifest.sig` (array order),
   `DELETE /releases/ID/assets/ASSETID`. Other assets are left alone. Then upload `manifest`,
   then `manifest.sig`. (There is a short moment without a manifest, or with a manifest and no
   matching signature: the client rejects that and checks again later, so this is fine. Say so in
   a comment.)
   A failure in steps 4-5 does **not** undo step 3 (the version release is good). Exit 1 with a
   message that names the failed file and says to run again with `--channel-only` once fixed.
6. **Verify**: `GET HOST/OWNER/REPO/releases/download/channel-NAME/manifest` with the same header
   file (`--location` allowed); the body must equal `OUT/manifest` byte for byte (`cmp`). Not 2xx
   or different: exit 1, "published, but the channel manifest URL does not serve the new manifest
   with token authentication".
7. Print one line per published thing, no token.

`--channel-only` runs only steps 4-7 (a retry after a channel failure; it does not need the
installer files).

## Not in scope

`release.sh` itself (unchanged), the client, Authenticode, any other host, deleting old versions,
release notes text (the body stays empty), running the real publish (that waits for Tyrel's go).

## Where to look

- `app/packaging/release.sh`: option loop, version default and check, `print_cmd`, `usage`, header style (copy the style).
- `crates/drydock/tests/publish_script.rs`: `CURL_STUB` (the exact request form), `Rig::route`, `happy_routes`, `Rig::publish_env`, `no_token`.
- `drydock_update::Manifest::to_text` (`show Manifest::to_text drydock-update`): the manifest's line format.
- `crates/drydock/tests/release_script.rs`: `the_script_has_no_hard_coded_host`, the same rule here.
