#!/bin/bash
# Builds a release's macOS assets on this Mac, with no GitHub workflows:
#
#   monitorctl-v<version>-macos-arm64.tar.gz   (+ .sha256)
#   Monitor-Control-v<version>-macos-arm64.zip  (+ .sha256)
#
# into target/release-assets/v<version>/. Run it from a clean checkout of the
# release tag, which must match Cargo.toml's version. With --draft NOTES.md it
# also creates the draft GitHub release with these assets; the Linux and
# Windows assets are added later from other machines (see
# specs/releases/<version>/tasks.md).
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
cd "$repo"

notes=""
if [ "${1:-}" = "--draft" ]; then
    notes=${2:?"usage: $0 [--draft NOTES.md]"}
    [ -f "$notes" ] || { echo "error: no release notes at $notes" >&2; exit 1; }
    # Absolute, since the script changes directory before calling gh.
    notes="$(cd "$(dirname "$notes")" && pwd)/$(basename "$notes")"
fi

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
tag="v$version"

# Release builds come only from the tagged, unmodified source.
if [ -n "$(git status --porcelain)" ]; then
    echo "error: the working tree has changes" >&2
    echo "hint: commit or stash them, then check out $tag" >&2
    exit 1
fi
if [ "$(git rev-parse HEAD)" != "$(git rev-parse -q --verify "$tag^{commit}" || true)" ]; then
    echo "error: HEAD isn't the $tag tag (Cargo.toml says $version)" >&2
    echo "hint: git tag $tag && git push origin $tag, or git checkout $tag" >&2
    exit 1
fi

out="$repo/target/release-assets/$tag"
rm -rf "$out"
mkdir -p "$out"

echo "Building monitorctl ${version}…"
cargo build --release --locked

cli="monitorctl-$tag-macos-arm64"
mkdir "$out/$cli"
cp target/release/monitorctl README.md LICENSE-MIT LICENSE-APACHE "$out/$cli/"
tar -czf "$out/$cli.tar.gz" -C "$out" "$cli"
rm -rf "${out:?}/$cli"

echo "Building Monitor Control ${version}…"
"$repo/macos-app/build.sh"
app="Monitor-Control-$tag-macos-arm64"
ditto -c -k --keepParent "$repo/target/Monitor Control.app" "$out/$app.zip"

cd "$out"
for asset in "$cli.tar.gz" "$app.zip"; do
    shasum -a 256 "$asset" > "$asset.sha256"
    shasum -a 256 -c "$asset.sha256"
done
# The archive holds one folder with the binary, README, and both licenses.
[ "$(tar -tzf "$cli.tar.gz" | grep -c "^$cli/.")" = 4 ] || { echo "error: $cli.tar.gz is missing files" >&2; exit 1; }

echo
echo "Assets in $out:"
ls -1 "$out"

if [ -n "$notes" ]; then
    gh release create "$tag" --draft --title "$tag" --notes-file "$notes" \
        "$cli.tar.gz" "$cli.tar.gz.sha256" "$app.zip" "$app.zip.sha256"
    echo "Created draft release $tag. Linux and Windows assets come next (specs/releases/$version/tasks.md)."
fi
