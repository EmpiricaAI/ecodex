#!/usr/bin/env bash
# ecodex install script
#
# Builds (if needed) and installs the ecodex binary + bundled empirica
# plugin in one shot. Mirrors the "single command, just works" UX of
# `npm install -g <pkg>` / `pip install <pkg>`.
#
# Safe with existing user configs — backs up before touching, never
# clobbers ~/.codex/config.toml if it already exists.
#
# Usage: ./install.sh [--system | --user] [--prefix DIR] [--no-build] [--fast]
#   --system    Install managed.toml to /etc/ecodex/ (requires sudo)
#   --user      Install managed.toml to ~/.ecodex/ (default)
#   --prefix    Override binary install dir (default: /usr/local)
#   --no-build  Skip the cargo build step (assume binaries already built)
#   --fast      Build with [profile.fast-release] (lto=thin, codegen-units=16)
#               for faster dev iteration. Use --release builds for shipping.

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
ECODEX_ROOT="$(cd -- "${SCRIPT_DIR}/.." &> /dev/null && pwd)"
WORKSPACE_ROOT="$(cd -- "${ECODEX_ROOT}/.." &> /dev/null && pwd)"

# Capture whether the caller set ECODEX_BINARY / PLUGIN_BINARY in the
# environment BEFORE we apply defaults (the `${VAR:-default}` form below
# would otherwise make every invocation look like "the var was set").
# Used downstream to decide whether to skip the cargo build entirely.
# `${VAR+1}` expands to `1` if VAR was set (even to empty), else to nothing.
ECODEX_BINARY_PRESET=${ECODEX_BINARY+1}
PLUGIN_BINARY_PRESET=${PLUGIN_BINARY+1}

# ─── Defaults ────────────────────────────────────────────────────────
SCOPE="user"           # system | user
PREFIX="/usr/local"
SKIP_BUILD=0
BUILD_PROFILE="release"   # release | fast-release  (T79 — --fast flag flips this)
CARGO_TARGET_DIR_NAME="release"   # cargo's directory name for the chosen profile
ECODEX_BINARY="${ECODEX_BINARY:-${WORKSPACE_ROOT}/codex-rs/target/release/ecodex}"
PLUGIN_BINARY="${PLUGIN_BINARY:-${WORKSPACE_ROOT}/codex-rs/target/release/codex-empirica-plugin}"
PLUGIN_SRC="${WORKSPACE_ROOT}/codex-rs/codex-empirica-plugin"
PLUGIN_VERSION="0.1.0"
PLUGIN_KEY="empirica@nubaeon"   # codex requires <plugin>@<marketplace> format

# ─── Parse args ──────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --system)    SCOPE="system";  shift ;;
    --user)      SCOPE="user";    shift ;;
    --prefix)    PREFIX="$2";     shift 2 ;;
    --no-build)  SKIP_BUILD=1;    shift ;;
    --fast)
      BUILD_PROFILE="fast-release"
      CARGO_TARGET_DIR_NAME="fast-release"
      ECODEX_BINARY="${WORKSPACE_ROOT}/codex-rs/target/fast-release/ecodex"
      PLUGIN_BINARY="${WORKSPACE_ROOT}/codex-rs/target/fast-release/codex-empirica-plugin"
      shift
      ;;
    -h|--help)
      sed -n '2,17p' "${BASH_SOURCE[0]}" | sed 's/^# //; s/^#//'
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
HUGGINGFACE_PROFILE="${HOME}/.codex/huggingface.config.toml"

# ─── Build (auto, unless --no-build) ─────────────────────────────────
# Run cargo build --release for both targets. cargo is a fast no-op
# when nothing has changed, so the cost on a clean install is paid
# once and incremental rebuilds during development are quick.
#
# Skip the build if --no-build is set OR if both binaries are explicitly
# overridden via env at script-entry time (i.e. user is pointing at a
# pre-built artifact in CI/release). We use the pre-defaults capture from
# the top of the script — the prior comparison-against-release-path
# misfired under `--fast` because that flag remaps the var to
# `target/fast-release/...` and the comparison then trivially succeeded.
ECODEX_BINARY_OVERRIDDEN=${ECODEX_BINARY_PRESET:-0}
PLUGIN_BINARY_OVERRIDDEN=${PLUGIN_BINARY_PRESET:-0}

if [[ ! -d "$PLUGIN_SRC" ]]; then
  echo "ecodex install: plugin source not found at $PLUGIN_SRC" >&2
  exit 1
fi

if [[ "$SKIP_BUILD" -eq 1 ]]; then
  echo "→ Skipping cargo build (--no-build)"
elif [[ "$ECODEX_BINARY_OVERRIDDEN" -eq 1 && "$PLUGIN_BINARY_OVERRIDDEN" -eq 1 ]]; then
  echo "→ Skipping cargo build (ECODEX_BINARY + PLUGIN_BINARY both overridden via env)"
else
  if ! command -v cargo >/dev/null 2>&1; then
    echo "ecodex install: cargo not found on PATH" >&2
    echo "  Install Rust: https://rustup.rs/   then re-run this script." >&2
    echo "  (Or pre-build elsewhere and re-run with ECODEX_BINARY=... PLUGIN_BINARY=... ./install.sh --no-build)" >&2
    exit 1
  fi
  echo "→ Building ecodex + empirica plugin (cargo build --profile=${BUILD_PROFILE}; no-op if up-to-date)"
  (cd "${WORKSPACE_ROOT}/codex-rs" && cargo build --profile="${BUILD_PROFILE}" -p codex-cli -p codex-empirica-plugin)
fi

# ─── Sanity checks (post-build) ──────────────────────────────────────
if [[ ! -x "$ECODEX_BINARY" ]]; then
  echo "ecodex install: ecodex binary still missing at $ECODEX_BINARY after build" >&2
  echo "  Run cargo build manually to see errors:" >&2
  echo "    (cd ${WORKSPACE_ROOT}/codex-rs && cargo build --release -p codex-cli)" >&2
  exit 1
fi
if [[ ! -x "$PLUGIN_BINARY" ]]; then
  echo "ecodex install: plugin binary still missing at $PLUGIN_BINARY after build" >&2
  echo "  Run cargo build manually to see errors:" >&2
  echo "    (cd ${WORKSPACE_ROOT}/codex-rs && cargo build --release -p codex-empirica-plugin)" >&2
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

# Profile-v2 uses a separate `$CODEX_HOME/<name>.config.toml` file rather than
# the legacy `[profiles.<name>]` table. Keep the Hugging Face quick-test profile
# independently installable and never overwrite user edits.
if [[ -f "$HUGGINGFACE_PROFILE" ]]; then
  echo "→ ~/.codex/huggingface.config.toml already exists — leaving it alone"
else
  echo "→ Installing Hugging Face profile to $HUGGINGFACE_PROFILE"
  cp "${ECODEX_ROOT}/huggingface.config.toml" "$HUGGINGFACE_PROFILE"
fi

# ─── Idempotent feature-flag patch (A — unlocks plugin host) ─────────
# Without `[features] plugin_hooks = true` the plugin host is DARK and
# every empirica hook (sentinel-gate, session-init, tool-router, etc.)
# silently no-ops. We discovered this 4 days late on a previous run.
# This block is a no-op when the keys are already present, so it's
# safe to run on every install.
patch_feature_keys() {
  local cfg="$1"
  local needs_patch=0
  grep -qE '^[[:space:]]*plugin_hooks[[:space:]]*=[[:space:]]*true' "$cfg" || needs_patch=1
  grep -qE '^[[:space:]]*plugins[[:space:]]*=[[:space:]]*true' "$cfg" || needs_patch=1
  if [[ "$needs_patch" -eq 0 ]]; then
    echo "  ✓ [features] plugin_hooks + plugins already set"
    return
  fi
  echo "  → Appending [features] block to $cfg (plugin_hooks=true, plugins=true)"
  {
    echo ""
    echo "# ─── Feature gates (added by ecodex install.sh) ───────────────────"
    echo "# Without these the plugin host is dark and ALL plugin hooks no-op."
    echo "# Re-run install.sh to re-apply if you remove these by accident."
    grep -qE '^[[:space:]]*suppress_unstable_features_warning' "$cfg" \
      || echo "suppress_unstable_features_warning = true"
    echo ""
    echo "[features]"
    grep -qE '^[[:space:]]*plugins[[:space:]]*=[[:space:]]*true' "$cfg" \
      || echo "plugins = true"
    grep -qE '^[[:space:]]*plugin_hooks[[:space:]]*=[[:space:]]*true' "$cfg" \
      || echo "plugin_hooks = true"
  } >> "$cfg"
}
echo "→ Verifying [features] gates in $CODEX_CONFIG"
patch_feature_keys "$CODEX_CONFIG"

# ─── Install wrapper script + binary ─────────────────────────────────
# `cp` over a binary that's currently executing fails with ETXTBSY
# ("Text file busy") on Linux. Linux holds the inode alive via the
# running process's FD even after the directory entry is removed, so
# `rm -f` then `cp` is the safe pattern: existing sessions keep
# running, new launches pick up the fresh binary.
install_binary() {
  local src="$1" dst="$2"
  rm -f "$dst" 2>/dev/null || true
  cp "$src" "$dst"
  chmod +x "$dst"
}
mkdir -p "$(dirname "$WRAPPER_DEST")" "$(dirname "$BINARY_DEST")"
install_binary "${ECODEX_ROOT}/scripts/ecodex-wrapper.sh" "$WRAPPER_DEST"
install_binary "$ECODEX_BINARY" "$BINARY_DEST"

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
# Re-installs need an explicit clean before `cp -r`. Without the rm, GNU cp
# detects an existing destination directory and copies the source *into* it
# (creating skills/skills/, hooks_scripts/hooks_scripts/, agents/agents/).
# The duplicates double the model-visible skill count, blow the 2% skills
# context budget, and leave stale top-level copies pinned to old content
# from the first install. Clean each dest before copying.
rm -rf "${PLUGIN_DEST_DIR}/skills"
cp -r "${PLUGIN_SRC}/skills"         "${PLUGIN_DEST_DIR}/skills"

# Bundled hook scripts: codex sets PLUGIN_ROOT when invoking plugin hook
# commands; the plugin binary resolves PLUGIN_ROOT/hooks_scripts/hooks/
# to find sentinel-gate.py / session-init.py / etc. Self-contained:
# no dependency on a coexisting CC empirica install at runtime.
if [[ -d "${PLUGIN_SRC}/assets/hooks_scripts" ]]; then
  rm -rf "${PLUGIN_DEST_DIR}/hooks_scripts"
  cp -r "${PLUGIN_SRC}/assets/hooks_scripts" "${PLUGIN_DEST_DIR}/hooks_scripts"
else
  echo "WARNING: ${PLUGIN_SRC}/assets/hooks_scripts/ missing — plugin will fall back to ~/.claude/...; run scripts/sync-empirica-assets.sh to vendor." >&2
fi

# Bundled empirica subagents (architecture, security, ux, etc.):
# SessionStart hook copies these into <codex_home>/agents/empirica/ at
# session start so codex can dispatch to specialists. Source dir resolved
# at runtime via PLUGIN_ROOT/agents.
if [[ -d "${PLUGIN_SRC}/assets/agents" ]]; then
  rm -rf "${PLUGIN_DEST_DIR}/agents"
  cp -r "${PLUGIN_SRC}/assets/agents" "${PLUGIN_DEST_DIR}/agents"
else
  echo "WARNING: ${PLUGIN_SRC}/assets/agents/ missing — subagents won't seed; run scripts/sync-empirica-assets.sh to vendor." >&2
fi

echo "→ Installing plugin binary to $PLUGIN_BIN_DEST"
# Same ETXTBSY safety as the ecodex binary above.
install_binary "$PLUGIN_BINARY" "$PLUGIN_BIN_DEST"

# ─── Verify install ──────────────────────────────────────────────────
# Catch the common "everything copied but it doesn't actually work"
# failure modes before the user finds them at runtime. Each check is
# fast; the whole block adds <1s.
echo ""
echo "→ Verifying install"
verify_failures=0
verify() {
  local label="$1"; local condition="$2"
  if eval "$condition" >/dev/null 2>&1; then
    echo "  ✓ $label"
  else
    echo "  ✗ $label  ($condition)" >&2
    verify_failures=$((verify_failures + 1))
  fi
}
verify "wrapper executable + on PATH"   "[[ -x \"$WRAPPER_DEST\" ]]"
verify "wrapper resolves the binary"     "grep -q \"^ECODEX_BINARY_PATH=\\\"$BINARY_DEST\\\"\" \"$WRAPPER_DEST\""
verify "ecodex binary executable"        "[[ -x \"$BINARY_DEST\" ]]"
verify "plugin binary on PATH"           "[[ -x \"$PLUGIN_BIN_DEST\" ]]"
verify "plugin manifest readable"        "[[ -r \"${PLUGIN_DEST_DIR}/.codex-plugin/plugin.json\" ]]"
verify "plugin manifest declares statusline"  "grep -q '\"statusline\"' \"${PLUGIN_DEST_DIR}/.codex-plugin/plugin.json\""
verify "bundled hooks_scripts/ present"  "[[ -d \"${PLUGIN_DEST_DIR}/hooks_scripts\" ]]"
verify "bundled agents/ present"          "[[ -d \"${PLUGIN_DEST_DIR}/agents\" ]]"
verify "statusline script executable"    "[[ -x \"${PLUGIN_DEST_DIR}/hooks_scripts/scripts/statusline_empirica.py\" ]]"
verify "config.toml has plugins.${PLUGIN_KEY} enabled"  "grep -E '^\\[plugins\\.\"empirica@nubaeon\"\\]' \"$CODEX_CONFIG\""
if [[ "$verify_failures" -gt 0 ]]; then
  echo ""
  echo "✗ Install completed with $verify_failures verification failure(s) above. Inspect before running ecodex." >&2
  exit 2
fi

# ─── Done ────────────────────────────────────────────────────────────
echo ""
echo "✓ ecodex installed."
echo "  • binary:        $BINARY_DEST"
echo "  • wrapper:       $WRAPPER_DEST  (this is what users invoke as 'ecodex')"
echo "  • plugin cache:  $PLUGIN_DEST_DIR/  (manifest+hooks+mcp+skills+statusline)"
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
# Running ecodex sessions still hold the OLD binary's inode (the
# rm-then-cp pattern above lets the install succeed without ETXTBSY,
# but in-flight sessions are unaffected). New behavior — including any
# fresh hooks, plugin updates, or feature-gate changes from this run —
# only takes effect on session restart.
if pgrep -f "${BINARY_DEST}\|${WRAPPER_DEST}" >/dev/null 2>&1; then
  echo "⚠ Restart any running ecodex sessions to pick up this build."
  echo "  In-flight sessions keep the previous binary via inherited FD."
  echo ""
fi

# ─── Post-build disk hygiene (threshold-gated, build-safe) ───────────
# Rust target dirs balloon across profiles (debug/release/fast-release) +
# incremental caches. Run the cargo cache guard after each build: it's a
# no-op unless target/ is large or disk is low, and its safety gate refuses
# to touch target/ while any cargo/rustc is running. Non-fatal.
GUARD="${WORKSPACE_ROOT}/scripts/cargo-cache-guard.sh"
if [[ -x "$GUARD" ]]; then
  "$GUARD" || echo "⚠ cargo-cache-guard returned non-zero (ignored)"
fi

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
