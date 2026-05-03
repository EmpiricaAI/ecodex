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
WORKSPACE_ROOT="$(cd -- "${ECODEX_ROOT}/.." &> /dev/null && pwd)"

# ─── Defaults ────────────────────────────────────────────────────────
SCOPE="user"           # system | user
PREFIX="/usr/local"
ECODEX_BINARY="${ECODEX_BINARY:-${WORKSPACE_ROOT}/codex-rs/target/release/ecodex}"
PLUGIN_BINARY="${PLUGIN_BINARY:-${WORKSPACE_ROOT}/codex-rs/target/release/codex-empirica-plugin}"
PLUGIN_SRC="${WORKSPACE_ROOT}/codex-rs/codex-empirica-plugin"
PLUGIN_VERSION="0.1.0"
PLUGIN_KEY="empirica@nubaeon"   # codex requires <plugin>@<marketplace> format

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
# NOTE: codex hardcodes SystemRequirementsToml at /etc/codex/requirements.toml
# (Unix). Per-user installs cannot enforce the lock without an upstream
# change. --user mode skips the lock; --system mode installs it.
if [[ "$SCOPE" == "system" ]]; then
  REQUIREMENTS_PATH="/etc/codex/requirements.toml"
  WRAPPER_DEST="${PREFIX}/bin/ecodex"
  BINARY_DEST="${PREFIX}/lib/ecodex/bin/ecodex"
  if [[ "$EUID" -ne 0 ]]; then
    echo "ecodex install --system requires root (rerun with sudo)" >&2
    exit 1
  fi
else
  REQUIREMENTS_PATH=""    # per-user: no lock enforced (codex limitation)
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
if [[ ! -x "$PLUGIN_BINARY" ]]; then
  echo "ecodex install: plugin binary not found at $PLUGIN_BINARY" >&2
  echo "  Build it first: (cd codex-rs && cargo build --release -p codex-empirica-plugin)" >&2
  echo "  Or set PLUGIN_BINARY to override." >&2
  exit 1
fi
if [[ ! -d "$PLUGIN_SRC" ]]; then
  echo "ecodex install: plugin source not found at $PLUGIN_SRC" >&2
  exit 1
fi

# ─── Install requirements.toml (B layer — the lock, system-only) ─────
if [[ -n "$REQUIREMENTS_PATH" ]]; then
  echo "→ Installing requirements.toml lock to $REQUIREMENTS_PATH"
  mkdir -p "$(dirname "$REQUIREMENTS_PATH")"
  cp "${ECODEX_ROOT}/requirements.toml.example" "$REQUIREMENTS_PATH"
else
  echo "→ Per-user install: skipping requirements.toml lock"
  echo "  (codex hardcodes /etc/codex/requirements.toml as the only managed-config path on Unix)"
  echo "  (use --system for sudo install if you want the lock enforced)"
fi

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

# ─── Install empirica plugin (cache + plugin binary on PATH) ─────────
# Codex cache layout: ~/.codex/plugins/cache/<marketplace>/<plugin>/<version>/
PLUGIN_MARKETPLACE="${PLUGIN_KEY##*@}"   # "nubaeon"
PLUGIN_NAME_ONLY="${PLUGIN_KEY%@*}"      # "empirica"
PLUGIN_DEST_DIR="${HOME}/.codex/plugins/cache/${PLUGIN_MARKETPLACE}/${PLUGIN_NAME_ONLY}/${PLUGIN_VERSION}"
PLUGIN_BIN_DEST="$(dirname "$WRAPPER_DEST")/codex-empirica-plugin"

echo "→ Installing empirica plugin to $PLUGIN_DEST_DIR/"
# Codex discovers plugin manifest at <root>/.codex-plugin/plugin.json
mkdir -p "${PLUGIN_DEST_DIR}/.codex-plugin"
cp "${PLUGIN_SRC}/manifest.json"     "${PLUGIN_DEST_DIR}/.codex-plugin/plugin.json"
cp "${PLUGIN_SRC}/hooks.json"        "${PLUGIN_DEST_DIR}/hooks.json"
cp "${PLUGIN_SRC}/mcp_servers.json"  "${PLUGIN_DEST_DIR}/mcp_servers.json"
cp -r "${PLUGIN_SRC}/skills"         "${PLUGIN_DEST_DIR}/skills"

# Bundled hook scripts: codex sets PLUGIN_ROOT when invoking plugin hook
# commands; the plugin binary resolves PLUGIN_ROOT/hooks_scripts/hooks/
# to find sentinel-gate.py / session-init.py / etc. Self-contained:
# no dependency on a coexisting CC empirica install at runtime.
if [[ -d "${PLUGIN_SRC}/assets/hooks_scripts" ]]; then
  cp -r "${PLUGIN_SRC}/assets/hooks_scripts" "${PLUGIN_DEST_DIR}/hooks_scripts"
else
  echo "WARNING: ${PLUGIN_SRC}/assets/hooks_scripts/ missing — plugin will fall back to ~/.claude/...; run scripts/sync-empirica-assets.sh to vendor." >&2
fi

echo "→ Installing plugin binary to $PLUGIN_BIN_DEST"
cp "$PLUGIN_BINARY" "$PLUGIN_BIN_DEST"
chmod +x "$PLUGIN_BIN_DEST"

# ─── Done ────────────────────────────────────────────────────────────
echo ""
echo "✓ ecodex installed."
echo "  • binary:        $BINARY_DEST"
echo "  • wrapper:       $WRAPPER_DEST  (this is what users invoke as 'ecodex')"
echo "  • plugin cache:  $PLUGIN_DEST_DIR/  (manifest+hooks+mcp+skills)"
echo "  • plugin binary: $PLUGIN_BIN_DEST  (codex's hooks invoke this)"
if [[ -n "$REQUIREMENTS_PATH" ]]; then
  echo "  • lock:          $REQUIREMENTS_PATH  (pins empirica@nubaeon enabled — system-enforced)"
else
  echo "  • lock:          (skipped — per-user install; cannot enforce on Linux)"
fi
if [[ ! -f "$CODEX_CONFIG.bak" && -f "$CODEX_CONFIG" ]]; then
  echo "  • config.toml:   $CODEX_CONFIG  (left as-is or freshly installed default)"
fi
echo ""
echo "Verify: $WRAPPER_DEST --version"
echo ""
echo "ecodex is the AI's calibration training environment. The empirica"
echo "plugin is bundled by default."
if [[ -n "$REQUIREMENTS_PATH" ]]; then
  echo "On this --system install the lock at $REQUIREMENTS_PATH"
  echo "structurally prevents the AI from disabling it at runtime."
else
  echo "On this --user install, a determined AI runtime CAN disable it"
  echo "(per-user installs cannot enforce the lock without an upstream"
  echo "codex change). Use --system for sudo-installed enforcement."
fi
echo "To opt out entirely, install upstream codex instead of ecodex."
