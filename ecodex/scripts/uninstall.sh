#!/usr/bin/env bash
# ecodex uninstall script
#
# Removes the wrapper, binary, and managed.toml lock. Leaves
# ~/.codex/config.toml alone (it may contain user customizations).
#
# Usage: ./uninstall.sh [--system | --user] [--prefix DIR] [--purge]
#   --system  Remove system-scope install (/etc/ecodex/, /usr/local/bin)
#   --user    Remove per-user install (~/.ecodex/, ~/.local/bin) — default
#   --prefix  Override binary install dir (default: /usr/local)
#   --purge   Also remove ~/.codex/config.toml (CAUTION: loses user config)

set -euo pipefail

SCOPE="user"
PREFIX="/usr/local"
PURGE=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --system)  SCOPE="system"; shift ;;
    --user)    SCOPE="user";   shift ;;
    --prefix)  PREFIX="$2";    shift 2 ;;
    --purge)   PURGE=1;        shift ;;
    -h|--help)
      sed -n '2,15p' "${BASH_SOURCE[0]}" | sed 's/^# //; s/^#//'
      exit 0
      ;;
    *) echo "ecodex uninstall: unknown arg '$1'" >&2; exit 64 ;;
  esac
done

if [[ "$SCOPE" == "system" ]]; then
  MANAGED_DIR="/etc/ecodex"
  WRAPPER_DEST="${PREFIX}/bin/ecodex"
  BINARY_DEST="${PREFIX}/lib/ecodex"
  if [[ "$EUID" -ne 0 ]]; then
    echo "ecodex uninstall --system requires root (rerun with sudo)" >&2
    exit 1
  fi
else
  MANAGED_DIR="${HOME}/.ecodex"
  WRAPPER_DEST="${HOME}/.local/bin/ecodex"
  BINARY_DEST="${HOME}/.local/lib/ecodex"
fi

CODEX_CONFIG="${HOME}/.codex/config.toml"

# ─── Remove managed.toml (unlocks plugins.empirica.enabled) ──────────
if [[ -f "${MANAGED_DIR}/managed.toml" ]]; then
  echo "→ Removing managed.toml lock from ${MANAGED_DIR}/"
  rm -f "${MANAGED_DIR}/managed.toml"
  rmdir --ignore-fail-on-non-empty "$MANAGED_DIR" 2>/dev/null || true
fi

# ─── Remove wrapper + binary ─────────────────────────────────────────
if [[ -f "$WRAPPER_DEST" ]]; then
  echo "→ Removing wrapper $WRAPPER_DEST"
  rm -f "$WRAPPER_DEST"
fi
if [[ -d "$BINARY_DEST" ]]; then
  echo "→ Removing binary tree $BINARY_DEST"
  rm -rf "$BINARY_DEST"
fi

# ─── Remove plugin (cache + plugin binary on PATH) ───────────────────
PLUGIN_BIN_DEST="$(dirname "$WRAPPER_DEST")/codex-empirica-plugin"
PLUGIN_CACHE_DIR="${HOME}/.codex/plugins/cache/empirica"

if [[ -f "$PLUGIN_BIN_DEST" ]]; then
  echo "→ Removing plugin binary $PLUGIN_BIN_DEST"
  rm -f "$PLUGIN_BIN_DEST"
fi
if [[ -d "$PLUGIN_CACHE_DIR" ]]; then
  echo "→ Removing plugin cache $PLUGIN_CACHE_DIR"
  rm -rf "$PLUGIN_CACHE_DIR"
  # Clean up parent cache dirs if now empty
  rmdir --ignore-fail-on-non-empty "${HOME}/.codex/plugins/cache" 2>/dev/null || true
  rmdir --ignore-fail-on-non-empty "${HOME}/.codex/plugins" 2>/dev/null || true
fi

# ─── Optional --purge of user config ─────────────────────────────────
if [[ "$PURGE" -eq 1 && -f "$CODEX_CONFIG" ]]; then
  BACKUP="${CODEX_CONFIG}.uninstall-backup-$(date +%Y%m%d-%H%M%S)"
  echo "→ --purge: moving $CODEX_CONFIG → $BACKUP"
  mv "$CODEX_CONFIG" "$BACKUP"
fi

echo ""
echo "✓ ecodex uninstalled."
echo ""
if [[ "$PURGE" -ne 1 && -f "$CODEX_CONFIG" ]]; then
  echo "Note: $CODEX_CONFIG was NOT removed (it may contain your customizations)."
  echo "      Re-run with --purge to remove it (a backup will be made first)."
fi
echo ""
echo "If you want to switch to vanilla codex, install it now:"
echo "  brew install codex   # or your platform's equivalent"
