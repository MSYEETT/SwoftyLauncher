#!/usr/bin/env bash
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
art=$(dirname "$here")
work=$art/work
cpus=${ART_CPUS:-2-7,10,11,14-31}
py=$(nix build --no-link --print-out-paths --impure --expr \
    'with import <nixpkgs> {}; python3.withPackages (p: [p.numpy p.pillow p.scipy p.zstandard])')/bin/python3
names=("$@")
if [ ${#names[@]} -eq 0 ]; then
    mapfile -t names < <("$py" -c "import json; [print(u[0]) for b in json.load(open('$here/upgrades.json'))['branches'] for u in b['upgrades']]")
fi
mkdir -p "$work" "$art/out"
taskset -c "$cpus" nix shell nixpkgs#blender --command \
    blender -b --factory-startup --python "$here/blender/render.py" -- "$work" 480 "${names[@]}"
taskset -c "$cpus" "$py" "$here/post.py" "$work" "$art/out" "${names[@]}"
"$py" "$here/icons.py" "$art/out"
"$py" "$here/manifest.py"
"$py" "$here/preview.py"
