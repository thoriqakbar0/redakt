#!/bin/sh
set -eu

fail() {
    printf 'redakt installer: %s\n' "$1" >&2
    exit 1
}

case "$(uname -s):$(uname -m)" in
    Linux:x86_64) target=x86_64-unknown-linux-gnu ;;
    Linux:aarch64|Linux:arm64) target=aarch64-unknown-linux-gnu ;;
    Darwin:x86_64) target=x86_64-apple-darwin ;;
    Darwin:arm64) target=aarch64-apple-darwin ;;
    *) fail 'No prebuilt archive for this platform. See https://github.com/thoriqakbar0/redakt#install-the-cli.' ;;
esac

if command -v sha256sum >/dev/null 2>&1; then
    checksum() { sha256sum; }
elif command -v shasum >/dev/null 2>&1; then
    checksum() { shasum -a 256; }
else
    fail 'sha256sum or shasum is required to verify the archive.'
fi

repository=thoriqakbar0/redakt
releases=https://redakt.ta-0.com/releases
scratch=$(mktemp -d)
staged=
cleanup() {
    if [ -n "$staged" ]; then
        rm -f "$staged"
    fi
    rm -rf "$scratch"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

download() {
    asset=$1
    shift
    release_path=latest/download
    if [ "$#" -gt 0 ]; then
        release_path=download/$1
    fi
    if command -v curl >/dev/null 2>&1 &&
        curl -q --proto '=https' --proto-redir '=https' --retry 3 --retry-all-errors \
            --connect-timeout 10 -fsL -o "$scratch/$asset" "$releases/$release_path/$asset"; then
        return
    fi
    command -v gh >/dev/null 2>&1 || return 1
    gh release download "$@" --repo "$repository" --pattern "$asset" --output "$scratch/$asset" --clobber
}

download SHA256SUMS ||
    fail 'Could not download the latest release checksums. For a private repository, run gh auth login with an account that has access.'

entry=$(LC_ALL=C awk -v target="$target" '
    { sub(/\r$/, "") }
    $2 ~ ("^redakt-v[0-9]+\\.[0-9]+\\.[0-9]+-" target "\\.tar\\.gz$") {
        if (++count == 1 && length($1) == 64 && $1 ~ /^[0-9a-fA-F]+$/) {
            print tolower($1), $2
        }
    }
    END { if (count != 1) exit 1 }
' "$scratch/SHA256SUMS") || entry=
[ -n "$entry" ] ||
    fail "No unique redakt archive checksum for $target in the latest release. You can install with 'cargo install redakt --locked'."

expected=${entry%% *}
archive=${entry#* }
folder=${archive%.tar.gz}
tag=${folder#redakt-}
tag=${tag%"-$target"}
download "$archive" "$tag" ||
    fail "Could not download $archive."
actual=$(checksum < "$scratch/$archive") || fail "Could not hash $archive."
[ "${actual%% *}" = "$expected" ] || fail "SHA-256 mismatch for $archive."

install_dir=${REDAKT_INSTALL_DIR:-"$HOME/.local/bin"}
case "$install_dir" in
    /*) ;;
    *) install_dir=$PWD/$install_dir ;;
esac
destination=$install_dir/redakt
[ ! -d "$destination" ] || fail "Cannot replace directory $destination."
mkdir -p "$install_dir"
staged=$(mktemp "$install_dir/.redakt.XXXXXX")
tar -xOzf "$scratch/$archive" "$folder/redakt" > "$staged" ||
    fail "Could not extract redakt from $archive."
chmod 755 "$staged"
version=$("$staged" --version) || fail 'The downloaded binary could not run on this system.'
[ "$version" = "redakt ${tag#v}" ] || fail "The downloaded binary did not report version ${tag#v}."
mv -f "$staged" "$destination"

printf 'Installed %s at %s\n' "$version" "$destination"
case ":${PATH:-}:" in
    *":$install_dir:"*) ;;
    *) printf 'Add %s to PATH to run redakt by name.\n' "$install_dir" ;;
esac
