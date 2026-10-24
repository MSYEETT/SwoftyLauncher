#!/usr/bin/env bash
# Unpacks the DXC workflow's archive for this platform into ./dxc for the release's dxc/ folder. The
# archive comes from this repository's dxc-<tag> pre-release and must match tools/dxc/SHA256SUMS, or
# the release's own SHA256SUMS while that file is absent. Without the pre-release the launcher ships
# without dxc and the job warns.
set -euo pipefail
archive=$1
here=$(dirname "$0")
tag="dxc-$(cat "$here/version")"
dl=$(mktemp -d)
if ! gh release download "$tag" -R "${GITHUB_REPOSITORY:-Swofty-Developments/SwoftyLauncher}" -p "$archive" -p SHA256SUMS -D "$dl"; then
    echo "::warning::the $tag release has no $archive; run the DXC workflow with publish, or this release ships without dxc"
    exit 0
fi
sums="$here/SHA256SUMS"
if [ ! -f "$sums" ]; then
    echo "::warning::tools/dxc/SHA256SUMS is missing; checking $archive against the $tag release's own SHA256SUMS"
    sums="$dl/SHA256SUMS"
fi
sums=$(realpath "$sums")
(cd "$dl" && grep " $archive\$" "$sums" | sha256sum -c -)
tar xzf "$dl/$archive"
test -f dxc/VERSION
echo "dxc $(cat dxc/VERSION) unpacked from $tag"
