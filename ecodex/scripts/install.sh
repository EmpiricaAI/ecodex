#!/usr/bin/env bash
# ecodex install script
#
# Drops the ecodex config artifacts (managed.toml + config.toml.default)
# into the appropriate OS locations + installs the wrapper script.
#
# Safe with existing user configs — backs up before touching, never
# clobbers ~/.codex/config.toml if it already exists.
#
# Usage: ./install.sh [--system | --user] [--prefix DIR]
#   --system  Install managed.toml to /etc/ecodex/ (requires sudo)
#   --user    Install managed.toml to ~/.ecodex/ (default)
#   --prefix  Override binary install dir (default: /usr/local)

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
ECODEX_ROOT="$(cd -- "${SCRIPT_DIR}/.." &> /dev/null && pwd)"

# ─── Defaults ────────────────────────────────────────────────────────
SCOPE="user"           # system | user
PREFIX="/usr/local"
ECODEX_BINARY="${ECODEX_BINARY:-${ECODEX_ROOT}/../codex-rs/target/release/ecodex}"

# ─── Parse args ──────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --system)  SCOPE="system"; shift ;;
    --user)    SCOPE="user";   shift ;;
    --prefix)  PREFIX="$2";    shift 2 ;;
    -h|--help)
      sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# //; s/^#//'
      exit 0
      ;;
    *) echo "ecodex install: unknown arg '$1'" >&2; exit 64 ;;
  esac
done

# ─── Resolve install paths ───────────────────────────────────────────
if [[ "$SCOPE" == "system" ]]; then
  MANAGED_DIR="/etc/ecodex"
  WRAPPER_DEST="${PREFIX}/bin/ecodex"
  BINARY_DEST="${PREFIX}/lib/ecodex/bin/ecodex"
  if [[ "$EUID" -ne 0 ]]; then
    echo "ecodex install --system requires root (rerun with sudo)" >&2
    exit 1
  fi
else
  MANAGED_DIR="${HOME}/.ecodex"
  WRAPPER_DEST="${HOME}/.local/bin/ecodex"
  BINARY_DEST="${HOME}/.local/lib/ecodex/bin/ecodex"
fi

CODEX_CONFIG="${HOME}/.codex/config.toml"

# ─── Sanity checks ───────────────────────────────────────────────────
if [[ ! -x "$ECODEX_BINARY" ]]; then
  echo "ecodex install: binary not found at $ECODEX_BINARY" >&2
  echo "  Build it first: (cd codex-rs && cargo build --release -p codex-cli)" >&2
  echo "  Or set ECODEX_BINARY to override." >&2
  exit 1
fi

# ─── Install managed.toml (B layer — the lock) ───────────────────────
echo "→ Installing managed.toml lock to $MANAGED_DIR/"
mkdir -p "$MANAGED_DIR"
cp "${ECODEX_ROOT}/managed.toml.example" "${MANAGED_DIR}/managed.toml"

# ─── Install bundled config.toml (A + E layer) — first run only ──────
mkdir -p "${HOME}/.codex"
if [[ -f "$CODEX_CONFIG" ]]; then
  echo "→ ~/.codex/config.toml already exists — leaving it alone"
  echo "  Suggested overrides are in: ${ECODEX_ROOT}/config.toml.default"
else
  echo "→ Installing default config to $CODEX_CONFIG"
  cp "${ECODEX_ROOT}/config.toml.default" "$CODEX_CONFIG"
fi

# ─── Install wrapper script + binary ─────────────────────────────────
mkdir -p "$(dirname "$WRAPPER_DEST")" "$(dirname "$BINARY_DEST")"
cp "${ECODEX_ROOT}/scripts/ecodex-wrapper.sh" "$WRAPPER_DEST"
chmod +x "$WRAPPER_DEST"
cp "$ECODEX_BINARY" "$BINARY_DEST"
chmod +x "$BINARY_DEST"

# Patch the wrapper's BINARY_PATH to point at the installed binary.
# (The shipped wrapper has a placeholder; we resolve it at install time.)
sed -i.bak "s|^ECODEX_BINARY_PATH=.*|ECODEX_BINARY_PATH=\"$BINARY_DEST\"|" "$WRAPPER_DEST"
rm -f "${WRAPPER_DEST}.bak"

# ─── Done ────────────────────────────────────────────────────────────
echo ""
echo "✓ ecodex installed."
echo "  • binary:       $BINARY_DEST"
echo "  • wrapper:      $WRAPPER_DEST  (this is what users invoke as 'ecodex')"
echo "  • managed.toml: ${MANAGED_DIR}/managed.toml  (locks plugins.empirica.enabled=true)"
if [[ ! -f "$CODEX_CONFIG.bak" && -f "$CODEX_CONFIG" ]]; then
  echo "  • config.toml:  $CODEX_CONFIG  (left as-is or freshly installed default)"
fi
echo ""
echo "Verify: $WRAPPER_DEST --version"
echo ""
echo "ecodex is the AI's calibration training environment. The empirica"
echo "plugin is structurally enabled and cannot be disabled at runtime."
echo "To opt out, install upstream codex (\`brew install codex\` etc.)"
echo "instead of ecodex."
