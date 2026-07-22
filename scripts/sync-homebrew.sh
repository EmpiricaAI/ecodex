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

declare -A TARGETS=(
  [AARCH64_APPLE_DARWIN]="aarch64-apple-darwin"
  [X86_64_APPLE_DARWIN]="x86_64-apple-darwin"
  [AARCH64_UNKNOWN_LINUX_MUSL]="aarch64-unknown-linux-musl"
  [X86_64_UNKNOWN_LINUX_MUSL]="x86_64-unknown-linux-musl"
)

fetch_sha() {
  # print the sha256 hex for a target's tarball, from the release .sha256 asset
  local target="$1"
  local url="https://github.com/${REPO}/releases/download/v${VERSION}/ecodex-${target}.tar.gz.sha256"
  curl -fsSL "$url" | awk '{print $1}'
}

cp "$TEMPLATE" "$OUT"
sed -i.bak "s/__VERSION__/${VERSION}/g" "$OUT"
for token in "${!TARGETS[@]}"; do
  target="${TARGETS[$token]}"
  echo "fetching sha256 for ${target}…" >&2
  sha="$(fetch_sha "$target")" || { echo "missing sha256 for ${target} — is the release built?" >&2; exit 1; }
  [ -n "$sha" ] || { echo "empty sha256 for ${target}" >&2; exit 1; }
  sed -i.bak "s/__SHA256_${token}__/${sha}/g" "$OUT"
done
rm -f "${OUT}.bak"

if grep -q '__SHA256_\|__VERSION__' "$OUT"; then
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
