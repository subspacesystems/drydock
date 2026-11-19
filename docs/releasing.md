# Maintaining and releasing Drydock

For maintainers. Contributors only need [CONTRIBUTING.md](../CONTRIBUTING.md).

## Where the code lives

Subspace Systems' own Git server is the source of truth. It push-mirrors `main` and the tags to
[github.com/subspacesystems/drydock](https://github.com/subspacesystems/drydock), where issues,
pull requests, CI and releases live.

A push mirror overwrites GitHub with the source repository. So:

- **Don't merge pull requests with GitHub's Merge button**, and don't push branches or tags to
  GitHub directly. The next mirror push replaces `main` and deletes anything that isn't in the
  source repository.
- **To merge a pull request**, fetch it into the source repository, merge it there and push. The
  mirror brings the merge to GitHub, and GitHub marks the pull request as merged because its
  commits are now on `main`:

  ```sh
  git fetch https://github.com/subspacesystems/drydock.git pull/123/head:pr-123
  git merge --no-ff pr-123     # after review, CI green and the CLA signed
  git push origin main
  ```

## CI

`.github/workflows/ci.yml` runs on every push to `main` and every pull request. Its Linux job runs
the same gates as Drydock's own test oracle:

- formatting, clippy and the tests, including the real-window tests on Xvfb and rendering on Mesa's
  software Vulkan;
- clippy for the Windows target;
- the installer script;
- the dependency license policy.

The advisories job checks RustSec separately, because a new advisory can fail it without any code
change. The Windows job builds the program and runs the tests that need no graphics adapter.

## Update signing key

Windows builds check for updates at
`https://github.com/subspacesystems/drydock/releases/latest/download/manifest`. Each release's
`manifest` names the installer's size and SHA-256. Its `manifest.sig` is an Ed25519 signature of
the manifest, and the program checks it against the public key built into it. The private key
never goes to GitHub, so a compromised GitHub account can't push an update.

To make the key pair, once, on a trusted Linux machine:

```sh
cd app
cargo run --release -p drydock-release -- keygen ~/drydock-update-key   # prints the public key
```

- Keep the private key file offline, with a backup. Anyone who has it can sign updates that
  every installed copy will accept.
- On GitHub, set the repository variable (Settings > Secrets and variables > Actions >
  Variables) `DRYDOCK_UPDATE_KEY` to the printed public key. It's public, and it is not a secret.
  The Release workflow refuses to build without it, because a build without it would carry the
  test key in `app/packaging/update-key.pub`, whose private half is in the tests.

The key built into installed copies can't be changed without users reinstalling, so losing the
private key ends automatic updates for every copy that has it.

## Making a release

1. Set `version` under `[workspace.package]` in `app/Cargo.toml` to the new version, commit and
   push.
2. Tag the commit and push the tag to the source repository:
   `git tag -a v0.13.0 -m "Drydock 0.13.0" && git push origin v0.13.0`. The mirror brings the
   tag to GitHub.
3. On GitHub, `.github/workflows/release.yml` builds:
   - the Windows installer, cross-compiled with `app/packaging/release.sh`, with the update URL
     and public key built in;
   - a Linux build, `drydock-X.Y.Z-linux-x86_64.tar.gz`, with the user guide;
   - `SHA256SUMS`.

   It puts them on a **draft** release. Drafts are visible only to maintainers.
4. Check the draft: write the release notes, and try the installer if the release warrants it.
5. Where the private key is, sign and publish:

   ```sh
   DRYDOCK_UPDATE_SIGNING_KEY=$(cat ~/drydock-update-key) app/packaging/sign-github-release.sh 0.13.0
   ```

   The script:
   - downloads the draft's installer and checks it against `SHA256SUMS`;
   - signs a manifest for it;
   - uploads `manifest` and `manifest.sig`;
   - publishes the release as the latest;
   - checks that the update URL serves the new manifest.

   It needs `gh`, signed in to an account that can edit the repository's releases. Pass
   `--dry-run` to stop before uploading.

Installed copies find the new version within a day: they check 20 seconds after starting and then
every 24 hours.

`app/packaging/publish.sh` publishes to a Gitea server's releases instead. It isn't used for public
releases, because our Gitea requires sign-in to download.

## Code signing

The installer isn't code-signed yet, so Windows SmartScreen warns about it
([app/packaging/INSTALL.md](../app/packaging/INSTALL.md)). `release.sh` signs with
`osslsigncode` when `DRYDOCK_SIGN_PFX` and `DRYDOCK_SIGN_PASS_FILE` are set. Once there's a
certificate, signing goes in the same place as the update signing (step 5), not in CI.
