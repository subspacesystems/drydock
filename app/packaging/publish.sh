#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Subspace Systems

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: publish.sh [--version X.Y.Z] [--out DIR] [--channel NAME]
                  [--channel-only] [--dry-run] [--print-urls]

Publish the release files written by release.sh to this repository's Gitea
releases. The host, repository and token come from DRYDOCK_GITEA_URL,
DRYDOCK_GITEA_REPO and DRYDOCK_GITEA_TOKEN.
EOF
}

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
app_dir=$(cd "$script_dir/.." && pwd)

version=
version_set=0
out=
channel=dev
channel_only=0
dry_run=0
print_urls=0

while [ $# -gt 0 ]; do
    case $1 in
        --version)
            if [ $# -lt 2 ]; then
                echo "version needs a value" >&2
                exit 2
            fi
            version=$2
            version_set=1
            shift 2
            ;;
        --version=*)
            version=${1#--version=}
            version_set=1
            shift
            ;;
        --out)
            if [ $# -lt 2 ]; then
                echo "out needs a value" >&2
                exit 2
            fi
            out=$2
            shift 2
            ;;
        --out=*)
            out=${1#--out=}
            shift
            ;;
        --channel)
            if [ $# -lt 2 ]; then
                echo "channel needs a value" >&2
                exit 2
            fi
            channel=$2
            shift 2
            ;;
        --channel=*)
            channel=${1#--channel=}
            shift
            ;;
        --channel-only)
            channel_only=1
            shift
            ;;
        --dry-run)
            dry_run=1
            shift
            ;;
        --print-urls)
            print_urls=1
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

if [ "$version_set" = 0 ]; then
    version=$(awk '
        /^\[workspace\.package\]/ { in_package = 1; next }
        in_package && /^[[:space:]]*version[[:space:]]*=/ {
            line = $0
            sub(/^[^"]*"/, "", line)
            sub(/".*$/, "", line)
            print line
            exit
        }
    ' "$app_dir/Cargo.toml")
fi

if ! [[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z.-]+)?$ ]]; then
    echo "invalid version: '$version'" >&2
    exit 2
fi

if ! [[ $channel =~ ^[A-Za-z0-9._-]+$ ]]; then
    echo "invalid channel: '$channel'" >&2
    exit 2
fi

if [ -z "${DRYDOCK_GITEA_URL:-}" ]; then
    echo "DRYDOCK_GITEA_URL is not set" >&2
    exit 1
fi
if ! [[ ${DRYDOCK_GITEA_URL} =~ ^https:[/][/][^/] ]]; then
    echo "DRYDOCK_GITEA_URL must be an https host" >&2
    exit 1
fi

if [ -z "${DRYDOCK_GITEA_REPO:-}" ]; then
    echo "DRYDOCK_GITEA_REPO is not set" >&2
    exit 1
fi
if ! [[ ${DRYDOCK_GITEA_REPO} =~ ^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$ ]]; then
    echo "DRYDOCK_GITEA_REPO must be owner/name" >&2
    exit 1
fi

host=${DRYDOCK_GITEA_URL}
while [ "${host%/}" != "$host" ]; do
    host=${host%/}
done
repo=${DRYDOCK_GITEA_REPO}

url_base=$host/$repo/releases/download/v$version
notes_url=$host/$repo/releases/tag/v$version

if [ "$print_urls" = 1 ]; then
    printf 'url-base=%s\n' "$url_base"
    printf 'notes-url=%s\n' "$notes_url"
    exit 0
fi

if [ -z "${DRYDOCK_GITEA_TOKEN:-}" ]; then
    echo "DRYDOCK_GITEA_TOKEN is not set" >&2
    exit 1
fi
token=${DRYDOCK_GITEA_TOKEN}

api=$host/api/v1/repos/$repo

if [ -z "$out" ]; then
    out=$app_dir/dist
fi
out=$(realpath -m "$out")

version_files=(drydock-$version-setup.exe INSTALL.txt SHA256SUMS)
channel_files=(manifest manifest.sig)

if [ "$channel_only" = 1 ]; then
    need_files=("${channel_files[@]}")
else
    need_files=("${version_files[@]}" "${channel_files[@]}")
fi

for file in "${need_files[@]}"; do
    if [ ! -f "$out/$file" ]; then
        echo "release file not found: $out/$file" >&2
        exit 1
    fi
done

if ! grep -qxF "version = $version" "$out/manifest"; then
    echo "manifest does not have the line version = $version" >&2
    exit 1
fi
expected_installer_url=$url_base/drydock-$version-setup.exe
if ! grep -qxF "installer_url = $expected_installer_url" "$out/manifest"; then
    echo "manifest installer_url does not match $expected_installer_url" >&2
    exit 1
fi

sig_size=$(wc -c < "$out/manifest.sig")
sig_size=${sig_size//[[:space:]]/}
if [ "$sig_size" != 64 ]; then
    echo "manifest.sig must be exactly 64 bytes (found $sig_size)" >&2
    exit 1
fi

channel_tag=channel-$channel

if [ "$dry_run" = 1 ]; then
    if [ "$channel_only" = 0 ]; then
        printf '+ %s %s\n' GET "$api/releases/tags/v$version"
        printf '+ %s %s\n' POST "$api/releases"
        for file in "${version_files[@]}"; do
            printf '+ %s %s (file %s)\n' POST \
                "$api/releases/{id}/assets?name=$file" "$out/$file"
        done
    fi
    printf '+ %s %s\n' GET "$api/releases/tags/$channel_tag"
    printf '+ %s %s\n' DELETE "$api/releases/{id}/assets/{asset}"
    for file in "${channel_files[@]}"; do
        printf '+ %s %s (file %s)\n' POST \
            "$api/releases/{id}/assets?name=$file" "$out/$file"
    done
    printf '+ %s %s\n' GET "$host/$repo/releases/download/$channel_tag/manifest"
    printf 'token is sent as Authorization: token ***\n'
    exit 0
fi

curl=${CURL:-curl}
tmpdir=${TMPDIR:-/tmp}

hdr=
json_hdr=
body=
json_file=
cleanup() {
    [ -z "$hdr" ] || rm -f "$hdr"
    [ -z "$json_hdr" ] || rm -f "$json_hdr"
    [ -z "$body" ] || rm -f "$body"
    [ -z "$json_file" ] || rm -f "$json_file"
}
trap cleanup EXIT

new_temp() {
    mktemp "$tmpdir/drydock-publish.XXXXXX"
}

hdr=$(new_temp)
chmod 600 "$hdr"
printf 'Authorization: token %s\n' "$token" > "$hdr"

json_hdr=$(new_temp)
chmod 600 "$json_hdr"
printf 'Authorization: token %s\n' "$token" > "$json_hdr"
printf 'Content-Type: application/json\n' >> "$json_hdr"

body=$(new_temp)
json_file=$(new_temp)

last_status=000

do_request() {
    local method=$1 url=$2 kind=${3:-none} source=${4:-} header=${5:-$hdr} follow=${6:-0}
    local -a opts=(--silent --show-error --request "$method"
        --header "@$header" --output "$body" --write-out '%{http_code}')
    if [ "$follow" = 1 ]; then
        opts=(--location "${opts[@]}")
    fi
    local response
    case $kind in
        json)
            if response=$("$curl" "${opts[@]}" --data "@$source" "$url"); then
                last_status=$response
            else
                last_status=000
            fi
            ;;
        form)
            if response=$("$curl" "${opts[@]}" --form "attachment=@$source" "$url"); then
                last_status=$response
            else
                last_status=000
            fi
            ;;
        *)
            if response=$("$curl" "${opts[@]}" "$url"); then
                last_status=$response
            else
                last_status=000
            fi
            ;;
    esac
}

is_success() {
    [[ $1 =~ ^2[0-9][0-9]$ ]]
}

report_failure() {
    local method=$1 url=$2 status=$3
    printf 'request failed: %s %s\n' "$method" "$url" >&2
    printf 'status: %s\n' "$status" >&2
    printf 'response: %s\n' "$(head -c 200 "$body" 2>/dev/null || true)" >&2
}

response_id() {
    python3 -I -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$body"
}

write_release_json() {
    local tag=$1 name=$2 target=${3:-}
    if [ -n "$target" ]; then
        printf '{"tag_name":"%s","name":"%s","target_commitish":"%s"}' \
            "$tag" "$name" "$target" > "$json_file"
    else
        printf '{"tag_name":"%s","name":"%s"}' "$tag" "$name" > "$json_file"
    fi
}

channel_failure() {
    local what=$1
    echo "failed to update $what in the channel release" >&2
    echo "the version release is published; fix the problem and run again with --channel-only" >&2
    exit 1
}

target=
if [ -n "${DRYDOCK_RELEASE_COMMIT:-}" ]; then
    if [[ $DRYDOCK_RELEASE_COMMIT =~ ^[0-9a-f]{7,40}$ ]] ||
        [[ $DRYDOCK_RELEASE_COMMIT =~ ^[A-Za-z0-9._/-]+$ ]]; then
        target=$DRYDOCK_RELEASE_COMMIT
    fi
fi

version_id=
channel_id=

if [ "$channel_only" = 0 ]; then
    request_url=$api/releases/tags/v$version
    do_request GET "$request_url"
    case $last_status in
        404)
            ;;
        200)
            echo "release v$version already exists; not overwriting" >&2
            exit 1
            ;;
        *)
            report_failure GET "$request_url" "$last_status"
            exit 1
            ;;
    esac

    write_release_json "v$version" "Drydock $version" "$target"
    request_url=$api/releases
    do_request POST "$request_url" json "$json_file" "$json_hdr"
    if ! is_success "$last_status"; then
        report_failure POST "$request_url" "$last_status"
        exit 1
    fi
    version_id=$(response_id)

    for file in "${version_files[@]}"; do
        request_url=$api/releases/$version_id/assets?name=$file
        do_request POST "$request_url" form "$out/$file"
        if is_success "$last_status"; then
            continue
        fi
        report_failure POST "$request_url" "$last_status"
        do_request DELETE "$api/releases/$version_id"
        release_delete_status=$last_status
        do_request DELETE "$api/tags/v$version"
        tag_delete_status=$last_status
        if ! is_success "$release_delete_status"; then
            echo "release v$version (id $version_id) was left behind" >&2
        fi
        if ! is_success "$tag_delete_status"; then
            echo "tag v$version was left behind" >&2
        fi
        echo "failed to upload $file; the version release was rolled back" >&2
        exit 1
    done
fi

request_url=$api/releases/tags/$channel_tag
do_request GET "$request_url"
channel_status=$last_status
channel_assets=

case $channel_status in
    404)
        write_release_json "$channel_tag" "Update channel $channel" "$target"
        request_url=$api/releases
        do_request POST "$request_url" json "$json_file" "$json_hdr"
        if ! is_success "$last_status"; then
            report_failure POST "$request_url" "$last_status"
            channel_failure "the channel release"
        fi
        channel_id=$(response_id)
        ;;
    200)
        channel_id=$(response_id)
        channel_assets=$(python3 -I -c '
import json, sys
release = json.load(open(sys.argv[1]))
for asset in release.get("assets", []):
    if asset.get("name") in ("manifest", "manifest.sig"):
        print(asset["id"], asset["name"])
' "$body")
        ;;
    *)
        report_failure GET "$request_url" "$channel_status"
        channel_failure "the channel release"
        ;;
esac

# Replace the channel manifest and signature. There is a short moment with no
# manifest, or with a manifest and no matching signature; the client rejects
# that and checks again later, so this is fine.
while IFS=' ' read -r asset_id asset_name; do
    [ -n "$asset_id" ] || continue
    request_url=$api/releases/$channel_id/assets/$asset_id
    do_request DELETE "$request_url"
    if ! is_success "$last_status"; then
        report_failure DELETE "$request_url" "$last_status"
        channel_failure "the old $asset_name"
    fi
done <<< "$channel_assets"

for file in "${channel_files[@]}"; do
    request_url=$api/releases/$channel_id/assets?name=$file
    do_request POST "$request_url" form "$out/$file"
    if ! is_success "$last_status"; then
        report_failure POST "$request_url" "$last_status"
        channel_failure "$file"
    fi
done

verify_url=$host/$repo/releases/download/$channel_tag/manifest
do_request GET "$verify_url" none "" "$hdr" 1
if ! is_success "$last_status" || ! cmp -s "$out/manifest" "$body"; then
    echo "published, but the channel manifest URL does not serve the new manifest with token authentication" >&2
    exit 1
fi

if [ "$channel_only" = 0 ]; then
    printf 'published release v%s (id %s)\n' "$version" "$version_id"
    for file in "${version_files[@]}"; do
        printf 'uploaded %s\n' "$file"
    done
fi
printf 'updated channel %s (id %s)\n' "$channel" "$channel_id"
printf 'uploaded manifest\n'
printf 'uploaded manifest.sig\n'
printf 'verified %s\n' "$verify_url"
