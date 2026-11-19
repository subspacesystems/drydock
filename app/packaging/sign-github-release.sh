#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Subspace Systems

# Sign the update manifest for a draft GitHub release that the Release workflow built, add it to
# the release and publish the release (docs/releasing.md). Run it where the private update key
# is, never on GitHub. Needs gh (signed in to an account that can edit the repository's
# releases), cargo, sha256sum and curl.

set -euo pipefail

usage() {
    cat <<'USAGE'
Usage: sign-github-release.sh [--repo OWNER/NAME] [--key-env NAME] [--dry-run] VERSION

Download the draft release vVERSION's installer, check it against the release's SHA256SUMS,
sign a manifest for it with the private key in the environment variable NAME (default
DRYDOCK_UPDATE_SIGNING_KEY), upload manifest and manifest.sig, and publish the release as the
latest. --repo defaults to subspacesystems/drydock. --dry-run stops before uploading.
USAGE
}

repo=subspacesystems/drydock
key_env=DRYDOCK_UPDATE_SIGNING_KEY
dry_run=0
version=

while [ $# -gt 0 ]; do
    case $1 in
        --repo) repo=${2:?--repo needs a value}; shift 2 ;;
        --repo=*) repo=${1#--repo=}; shift ;;
        --key-env) key_env=${2:?--key-env needs a value}; shift 2 ;;
        --key-env=*) key_env=${1#--key-env=}; shift ;;
        --dry-run) dry_run=1; shift ;;
        -h | --help) usage; exit 0 ;;
        -*) echo "unknown argument: $1" >&2; exit 2 ;;
        *)
            if [ -n "$version" ]; then
                echo "only one VERSION" >&2
                exit 2
            fi
            version=$1
            shift
            ;;
    esac
done

if ! [[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]]; then
    echo "VERSION must look like 1.2.3 (got '$version')" >&2
    exit 2
fi
if ! [[ $repo =~ ^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$ ]]; then
    echo "--repo must be owner/name" >&2
    exit 2
fi
if [ -z "${!key_env:-}" ]; then
    echo "the private update key is not in the environment variable $key_env" >&2
    exit 1
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
app_dir=$(cd "$script_dir/.." && pwd)
tag=v$version
installer=drydock-$version-setup.exe
base=https://github.com/$repo/releases

if [ "$(gh release view "$tag" --repo "$repo" --json isDraft --jq .isDraft)" != true ]; then
    echo "release $tag in $repo is not a draft; not changing it" >&2
    exit 1
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/drydock-sign.XXXXXX")
trap 'rm -rf "$work"' EXIT

gh release download "$tag" --repo "$repo" --dir "$work" --pattern "$installer" --pattern SHA256SUMS
(cd "$work" && grep -F "  $installer" SHA256SUMS | sha256sum -c -)

(
    cd "$app_dir"
    cargo run --quiet --release --locked -p drydock-release -- sign \
        --key-env "$key_env" --version "$version" \
        --notes-url "$base/tag/$tag" --installer-url "$base/download/$tag/$installer" \
        "$work/$installer" "$work/out"
)
cat "$work/out/manifest"

if [ "$dry_run" = 1 ]; then
    echo "dry run: not uploading or publishing"
    exit 0
fi

gh release upload "$tag" --repo "$repo" "$work/out/manifest" "$work/out/manifest.sig"
gh release edit "$tag" --repo "$repo" --draft=false --latest

# The URL built into the program must now serve this manifest.
sleep 5
curl --silent --show-error --fail --location --output "$work/served" "$base/latest/download/manifest"
if ! cmp -s "$work/served" "$work/out/manifest"; then
    echo "published, but $base/latest/download/manifest does not serve the new manifest yet; check it" >&2
    exit 1
fi
echo "published $tag; $base/latest/download/manifest serves its manifest"
