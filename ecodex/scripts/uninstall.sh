#!/usr/bin/env bash
# ecodex uninstall script
#
# Removes the wrapper, the binary, the plugin binary and the plugin cache.
# Leaves ~/.codex/config.toml alone (it may contain user customizations).
#
# Usage: ./uninstall.sh [--system | --user] [--prefix DIR] [--purge]
#   --system  Remove a system-scope install (under --prefix)
#   --user    Remove the per-user install (~/.local) — default
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
      sed -n '2,11p' "${BASH_SOURCE[0]}" | sed 's/^# //; s/^#//'
      exit 0
      ;;
    *) echo "ecodex uninstall: unknown arg '$1'" >&2; exit 64 ;;
  esac
done

if [[ "$SCOPE" == "system" ]]; then
  WRAPPER_DEST="${PREFIX}/bin/ecodex"
  BINARY_DEST="${PREFIX}/lib/ecodex"
  if [[ "$EUID" -ne 0 ]]; then
    echo "ecodex uninstall --system requires root (rerun with sudo)" >&2
    exit 1
  fi
  # Older --system installs wrote /etc/codex/requirements.toml, meant as a lock
  # that kept the plugin enabled. codex never enforced it. Remove it only when
  # it is that file, so an administrator's own requirements.toml stays.
  OLD_LOCK="/etc/codex/requirements.toml"
  if [[ -f "$OLD_LOCK" ]] && head -n 1 "$OLD_LOCK" | grep -q '^# ecodex managed config'; then
    echo "→ Removing $OLD_LOCK (written by an older ecodex install)"
    rm -f "$OLD_LOCK"
    rmdir --ignore-fail-on-non-empty "$(dirname "$OLD_LOCK")" 2>/dev/null || true
  fi
  # Even older installs used /etc/ecodex/managed.toml.
  rm -f /etc/ecodex/managed.toml 2>/dev/null || true
  rmdir --ignore-fail-on-non-empty /etc/ecodex 2>/dev/null || true
else
  WRAPPER_DEST="${HOME}/.local/bin/ecodex"
  BINARY_DEST="${HOME}/.local/lib/ecodex"
  # Legacy cleanup: earlier installer used ~/.ecodex/managed.toml
  rm -f "${HOME}/.ecodex/managed.toml" 2>/dev/null || true
  rmdir --ignore-fail-on-non-empty "${HOME}/.ecodex" 2>/dev/null || true
fi

CODEX_CONFIG="${HOME}/.codex/config.toml"

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
# Cache layout: ~/.codex/plugins/cache/<marketplace>/<plugin>/
PLUGIN_CACHE_DIR="${HOME}/.codex/plugins/cache/empiricaAI"

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
