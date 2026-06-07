#!/usr/bin/env bash
# Fetch and vendor the OCPI specifications from github.com/ocpi/ocpi.
#
# The vendored spec text is (c) EV Roaming Foundation and is NOT covered by this
# repository's MIT license. See specs/NOTICE.md.
#
# Usage: scripts/fetch-specs.sh
set -euo pipefail

REPO="https://github.com/ocpi/ocpi.git"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SPECS="$ROOT/specs/ocpi"
TMP="$ROOT/.spec-tmp"

# version : upstream branch holding that release's text
VERSIONS=(
  "2.1.1:release-2.1.1-bugfixes"
  "2.2:release-2.2-bugfixes"
  "2.2.1:release-2.2.1-bugfixes"
  "2.3.0:release-2.3.0-bugfixes"
)

echo ">> cloning $REPO"
rm -rf "$TMP"
git clone --quiet "$REPO" "$TMP"

for entry in "${VERSIONS[@]}"; do
  version="${entry%%:*}"
  branch="${entry##*:}"
  dest="$SPECS/$version"
  echo ">> $version  (branch: $branch)"
  if ! git -C "$TMP" checkout --quiet "$branch" 2>/dev/null; then
    echo "   !! branch $branch not found upstream; skipping"
    continue
  fi
  mkdir -p "$dest"
  # Module specs are *.asciidoc at the repo root (OCPI 2.2+). Copy those.
  asciidoc_n="$(find "$TMP" -maxdepth 1 -type f -name '*.asciidoc' | wc -l | tr -d ' ')"
  find "$TMP" -maxdepth 1 -type f -name '*.asciidoc' -exec cp -f {} "$dest"/ \;
  if [ "$asciidoc_n" -eq 0 ]; then
    # Legacy versions (e.g. 2.1.1) ship only as PDF. Vendor the single canonical
    # release PDF rather than the whole upstream releases/ archive.
    pdf="$(find "$TMP" -type f -name "OCPI_${version}.pdf" ! -path '*/.git/*' | head -1)"
    [ -z "$pdf" ] && pdf="$(find "$TMP" -type f -iname "*${version}*.pdf" ! -path '*/.git/*' | head -1)"
    [ -n "$pdf" ] && cp -f "$pdf" "$dest"/
  fi
  sha="$(git -C "$TMP" rev-parse HEAD)"
  cdate="$(git -C "$TMP" show -s --format=%ci HEAD)"
  cat > "$dest/SOURCE.txt" <<EOF
source:     $REPO
branch:     $branch
commit:     $sha
commit_date:$cdate
fetched_by: scripts/fetch-specs.sh
copyright:  (c) EV Roaming Foundation — see ../../NOTICE.md
EOF
  count="$(find "$dest" -type f \( -name '*.asciidoc' -o -name '*.pdf' \) | wc -l | tr -d ' ')"
  echo "   vendored $count files -> specs/ocpi/$version/"
done

rm -rf "$TMP"
echo ">> done."
