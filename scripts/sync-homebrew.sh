#!/usr/bin/env bash
# Fill packaging/homebrew/ecodex.rb with the per-platform SHA-256s from a
# published GitHub Release, producing a ready-to-commit formula for the
# EmpiricaAI/homebrew-tap repo.
#
#   scripts/sync-homebrew.sh 0.2.6 [--tap /path/to/homebrew-tap]
#
# Reads ecodex-<target>.tar.gz.sha256 from the release (uploaded by
# .github/workflows/release.yml). Without --tap it writes the filled formula to
# packaging/homebrew/ecodex.filled.rb and prints it; with --tap it also copies
# to <tap>/Formula/ecodex.rb (you commit + push the tap yourself — release gate).
set -euo pipefail

REPO="EmpiricaAI/ecodex"
HERE="$(cd "$(dirname "$0")/.." && pwd)"
TEMPLATE="${HERE}/packaging/homebrew/ecodex.rb"
OUT="${HERE}/packaging/homebrew/ecodex.filled.rb"

VERSION="${1:?usage: sync-homebrew.sh <version> [--tap DIR]}"
VERSION="${VERSION#v}"   # accept 0.2.6 or v0.2.6
TAP=""
shift || true
while [ $# -gt 0 ]; do
  case "$1" in
    --tap) TAP="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

# Indexed array (not associative) so this runs on macOS's stock bash 3.2 —
# `declare -A` requires bash 4+. The __SHA256_<TOKEN>__ placeholder token is
# derived from each target: uppercase, with '-' mapped to '_'.
TARGETS=(
  aarch64-apple-darwin
  x86_64-apple-darwin
  aarch64-unknown-linux-gnu
  x86_64-unknown-linux-gnu
)

fetch_sha() {
  # print the sha256 hex for a target's tarball, from the release .sha256 asset
  local target="$1"
  local url="https://github.com/${REPO}/releases/download/v${VERSION}/ecodex-${target}.tar.gz.sha256"
  curl -fsSL "$url" | awk '{print $1}'
}

cp "$TEMPLATE" "$OUT"
sed -i.bak "s/__VERSION__/${VERSION}/g" "$OUT"
for target in "${TARGETS[@]}"; do
  token="$(printf '%s' "$target" | tr 'a-z-' 'A-Z_')"
  echo "fetching sha256 for ${target}…" >&2
  sha="$(fetch_sha "$target")" || { echo "missing sha256 for ${target} — is the release built?" >&2; exit 1; }
  [ -n "$sha" ] || { echo "empty sha256 for ${target}" >&2; exit 1; }
  sed -i.bak "s/__SHA256_${token}__/${sha}/g" "$OUT"
done
rm -f "${OUT}.bak"

# Match only real unfilled tokens (__SHA256_<UPPER>), not the doc comment's __SHA256_*.
if grep -v '^#' "$OUT" | grep -q '__SHA256_[A-Z]\|__VERSION__'; then
  echo "ERROR: unfilled placeholders remain in ${OUT}" >&2
  exit 1
fi

echo "wrote ${OUT}" >&2
if [ -n "$TAP" ]; then
  mkdir -p "${TAP}/Formula"
  cp "$OUT" "${TAP}/Formula/ecodex.rb"
  echo "copied to ${TAP}/Formula/ecodex.rb — review, commit, and push the tap." >&2
fi
cat "$OUT"
