#!/bin/sh
# Downloads the packages in packages.txt into a folder and checks each sha256.
# Usage: fetch.sh packages.txt folder
set -eu
list=$1
out=$2
mkdir -p "$out"
grep -v '^#' "$list" | while read -r name version sha; do
    [ -n "$name" ] || continue
    file="$out/$(echo "$name" | sed 's|/|__|')@$version.webc"
    curl -fsSL --retry 3 -o "$file" "https://cdn.wasmer.io/webcimages/$sha.webc"
    echo "$sha  $file" | sha256sum -c -
done
