#!/usr/bin/env bash
# The repository ships no track art. This puts your own archive for the code hockenheim at track/hockenheim.erp and
# its mipmaps at track/mipmaps/: from an imported track's folder (its one .erp and mipmaps/), or the files you name.
#   track/standin.sh ~/tracks/hockenheim/                     the folder's .erp and mipmaps/
#   track/standin.sh ~/tracks/hockenheim.erp ~/tracks/mips/    an archive and a mipmaps folder
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"
SRC=${1:?"usage: track/standin.sh <track folder> | <archive.erp> [mipmaps folder]"}
if [ -d "$SRC" ]; then
    ERP=$(find "$SRC" -maxdepth 1 -name '*.erp' | head -1)
    MIPS=$SRC/mipmaps
else
    ERP=$SRC
    MIPS=${2:-}
fi
[ -f "$ERP" ] || { echo "track/standin.sh: no .erp in $SRC" >&2; exit 2; }
cp "$ERP" hockenheim.erp
echo "track/hockenheim.erp <- $ERP"
if [ -n "$MIPS" ] && [ -d "$MIPS" ]; then
    rm -rf mipmaps
    cp -r "$MIPS" mipmaps
    echo "track/mipmaps/ <- $MIPS"
fi
