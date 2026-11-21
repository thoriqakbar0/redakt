#!/bin/sh
set -eu

repository=thoriqakbar0/redakt
latest=$(gh release view --repo "$repository" --json tagName --jq .tagName)
tags=$(gh api --paginate "repos/$repository/releases" --jq '
    .[]
    | select(.draft == false and .prerelease == false)
    | select(.tag_name | test("^v[0-9]+\\.[0-9]+\\.[0-9]+$"))
    | select(any(.assets[]; .name | startswith("redakt-v")))
    | .tag_name
')

scratch=$(mktemp -d .release-assets.XXXXXX)
trap 'rm -rf "$scratch"' EXIT
trap 'exit 1' HUP INT TERM
for tag in $tags; do
    archives="$scratch/releases/download/$tag"
    mkdir -p "$archives"
    gh release download "$tag" --repo "$repository" --dir "$archives" \
        --pattern "redakt-$tag-*.tar.gz" --pattern "redakt-$tag-*.zip" --pattern SHA256SUMS
    (
        cd "$archives"
        set -- ./*.tar.gz ./*.zip
        if [ "$#" -ne 5 ]; then
            printf 'Release %s must contain five platform archives.\n' "$tag" >&2
            exit 1
        fi
        for archive in \
            "redakt-$tag-aarch64-apple-darwin.tar.gz" \
            "redakt-$tag-aarch64-unknown-linux-gnu.tar.gz" \
            "redakt-$tag-x86_64-apple-darwin.tar.gz" \
            "redakt-$tag-x86_64-pc-windows-msvc.zip" \
            "redakt-$tag-x86_64-unknown-linux-gnu.tar.gz"; do
            shasum -a 256 "$archive"
        done > expected
        LC_ALL=C sort expected > expected.sorted
        LC_ALL=C sort SHA256SUMS > manifest.sorted
        if ! cmp -s expected.sorted manifest.sorted; then
            printf 'Release %s has missing or invalid archive checksums.\n' "$tag" >&2
            exit 1
        fi
        rm expected expected.sorted manifest.sorted
    )
done

if [ ! -f "$scratch/releases/download/$latest/SHA256SUMS" ]; then
    printf 'The latest release %s has no verified redakt archives.\n' "$latest" >&2
    exit 1
fi
mkdir -p "$scratch/releases/latest/download"
cp "$scratch/releases/download/$latest/SHA256SUMS" "$scratch/releases/latest/download/SHA256SUMS"
mkdir -p public/releases
cp -R "$scratch/releases/." public/releases/
