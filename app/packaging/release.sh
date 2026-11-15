#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Subspace Systems

set -euo pipefail

usage() {
    cat <<'EOF'
Usage: release.sh [--version X.Y.Z] [--out DIR] [--guide DIR] [--dry-run]
                  [--sign-key-env NAME --url-base URL --notes-url URL]

Build the Windows program, pack it into the installer and write the files a
beta tester downloads.
EOF
}

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
packaging_dir=$script_dir
app_dir=$(cd "$script_dir/.." && pwd)

version=
version_set=0
out=
guide=
dry_run=0
sign_key_env=
url_base=
notes_url=
sign_key_env_set=0
url_base_set=0
notes_url_set=0

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
        --guide)
            if [ $# -lt 2 ]; then
                echo "guide needs a value" >&2
                exit 2
            fi
            guide=$2
            shift 2
            ;;
        --guide=*)
            guide=${1#--guide=}
            shift
            ;;
        --sign-key-env)
            if [ $# -lt 2 ]; then
                echo "sign-key-env needs a value" >&2
                exit 2
            fi
            sign_key_env=$2
            sign_key_env_set=1
            shift 2
            ;;
        --sign-key-env=*)
            sign_key_env=${1#--sign-key-env=}
            sign_key_env_set=1
            shift
            ;;
        --url-base)
            if [ $# -lt 2 ]; then
                echo "url-base needs a value" >&2
                exit 2
            fi
            url_base=$2
            url_base_set=1
            shift 2
            ;;
        --url-base=*)
            url_base=${1#--url-base=}
            url_base_set=1
            shift
            ;;
        --notes-url)
            if [ $# -lt 2 ]; then
                echo "notes-url needs a value" >&2
                exit 2
            fi
            notes_url=$2
            notes_url_set=1
            shift 2
            ;;
        --notes-url=*)
            notes_url=${1#--notes-url=}
            notes_url_set=1
            shift
            ;;
        --dry-run)
            dry_run=1
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

if [ "$sign_key_env_set" = 1 ] || [ "$url_base_set" = 1 ] || [ "$notes_url_set" = 1 ]; then
    if [ "$sign_key_env_set" = 0 ] || [ "$url_base_set" = 0 ] || [ "$notes_url_set" = 0 ]; then
        echo "signing options must be given together: --sign-key-env, --url-base and --notes-url" >&2
        exit 2
    fi
fi

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

if [ "$sign_key_env_set" = 1 ] && [ "$dry_run" = 0 ]; then
    if [ -z "$sign_key_env" ] || [ -z "${!sign_key_env:-}" ]; then
        echo "signing key environment variable not set: $sign_key_env" >&2
        exit 1
    fi
fi

repo=$(cd "$app_dir/.." && pwd)
export DRYDOCK_VERSION=$version
if [ -z "${DRYDOCK_GIT_HASH:-}" ]; then
    hash=$(git -c safe.directory="$repo" -C "$repo" rev-parse --short HEAD 2>/dev/null || true)
    if [ -n "$hash" ]; then
        export DRYDOCK_GIT_HASH=$hash
    fi
fi

if [ -n "$guide" ] && [ ! -d "$guide" ]; then
    echo "guide folder not found: $guide" >&2
    exit 1
fi

sign_pfx=${DRYDOCK_SIGN_PFX:-}
sign_pass_file=${DRYDOCK_SIGN_PASS_FILE:-}

if [ -n "$sign_pfx" ] && [ ! -f "$sign_pfx" ]; then
    echo "signing certificate not found: $sign_pfx" >&2
    exit 1
fi

if [ -z "$sign_pfx" ]; then
    echo "signing skipped: no DRYDOCK_SIGN_PFX configured"
fi

if [ -z "$out" ]; then
    out=$app_dir/dist
fi

if [ "$dry_run" = 1 ]; then
    out=$(realpath -m "$out")
else
    mkdir -p "$out"
    out=$(cd "$out" && pwd)
fi

cargo=${CARGO:-cargo}
makensis=${MAKENSIS:-makensis}
osslsigncode=${OSSLSIGNCODE:-osslsigncode}

if [ -z "${CARGO_TARGET_DIR:-}" ]; then
    CARGO_TARGET_DIR=$app_dir/target
else
    case $CARGO_TARGET_DIR in
        /*) ;;
        *) CARGO_TARGET_DIR=$app_dir/$CARGO_TARGET_DIR ;;
    esac
fi
export CARGO_TARGET_DIR

installer=$out/drydock-$version-setup.exe
installer_url=
if [ "$sign_key_env_set" = 1 ]; then
    while [ "${url_base%/}" != "$url_base" ]; do
        url_base=${url_base%/}
    done
    installer_url=$url_base/drydock-$version-setup.exe
fi
nsi=$packaging_dir/windows/drydock.nsi
stage=$out/stage
exe_name=drydock.exe
exe=$CARGO_TARGET_DIR/x86_64-pc-windows-gnu/release/$exe_name

print_cmd() {
    printf '+'
    printf ' %q' "$@"
    printf '\n'
}

sign_file() {
    local file=$1
    if [ "$dry_run" = 1 ]; then
        print_cmd "$osslsigncode" sign -pkcs12 "$sign_pfx" -readpass "$sign_pass_file" \
            -h sha256 -in "$file" -out "$file.signed"
        return
    fi
    "$osslsigncode" sign -pkcs12 "$sign_pfx" -readpass "$sign_pass_file" \
        -h sha256 -in "$file" -out "$file.signed"
    mv "$file.signed" "$file"
}

if [ "$dry_run" = 1 ]; then
    printf 'DRYDOCK_VERSION=%s\n' "$version"
    if [ -n "${DRYDOCK_GIT_HASH:-}" ]; then
        printf 'DRYDOCK_GIT_HASH=%s\n' "$DRYDOCK_GIT_HASH"
    fi
    print_cmd "$cargo" build --release --locked --target x86_64-pc-windows-gnu -p drydock
    if [ -z "$guide" ]; then
        docs_guide=$(realpath -m "$app_dir/../docs/guide")
        print_cmd "$cargo" run --release --locked -p drydock-guide -- "$docs_guide" "$stage/guide"
    fi
    if [ -n "$sign_pfx" ]; then
        sign_file "$stage/$exe_name"
    fi
    print_cmd "$makensis" "-DVERSION=$version" "-DSTAGE=$stage" "-DOUTFILE=$installer" "$nsi"
    if [ -n "$sign_pfx" ]; then
        sign_file "$installer"
    fi
    if [ "$sign_key_env_set" = 1 ]; then
        print_cmd "$cargo" run --release --locked -p drydock-release -- sign \
            --key-env "$sign_key_env" --version "$version" --notes-url "$notes_url" \
            --installer-url "$installer_url" "$installer" "$out"
    fi
    exit 0
fi

(
    cd "$app_dir"
    "$cargo" build --release --locked --target x86_64-pc-windows-gnu -p drydock
)

if [ ! -f "$exe" ]; then
    echo "$exe_name not found at: $exe" >&2
    exit 1
fi

rm -rf "$stage"
mkdir -p "$stage"
cp "$exe" "$stage/$exe_name"
if [ -n "$guide" ]; then
    cp -R "$guide" "$stage/guide"
else
    docs_guide=$(cd "$app_dir/../docs/guide" && pwd)
    (
        cd "$app_dir"
        "$cargo" run --release --locked -p drydock-guide -- "$docs_guide" "$stage/guide"
    )
fi

if [ -n "$sign_pfx" ]; then
    sign_file "$stage/$exe_name"
fi

"$makensis" "-DVERSION=$version" "-DSTAGE=$stage" "-DOUTFILE=$installer" "$nsi"

if [ ! -f "$installer" ]; then
    echo "installer not found: $installer" >&2
    exit 1
fi

if [ -n "$sign_pfx" ]; then
    sign_file "$installer"
fi

if [ "$sign_key_env_set" = 1 ]; then
    (
        cd "$app_dir"
        "$cargo" run --release --locked -p drydock-release -- sign \
            --key-env "$sign_key_env" --version "$version" --notes-url "$notes_url" \
            --installer-url "$installer_url" "$installer" "$out"
    )
fi

cp "$packaging_dir/INSTALL.md" "$out/INSTALL.txt"

(
    cd "$out"
    sha256sum INSTALL.txt "drydock-$version-setup.exe" > SHA256SUMS
)
