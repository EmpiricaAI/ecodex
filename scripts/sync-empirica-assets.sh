#!/usr/bin/env bash
# sync-empirica-assets.sh — vendor empirica plugin assets from the
# CC empirica install (~/.claude/plugins/local/empirica) into the ecodex
# plugin source tree (codex-rs/codex-empirica-plugin/assets/).
#
# Run this after empirica releases new hook scripts / system-prompt /
# lib changes that the ecodex plugin should track.
#
# Vendored layout in plugin source:
#   codex-rs/codex-empirica-plugin/assets/
#     empirica-system-prompt.md          (compiled into binary via include_str!)
#     hooks_scripts/
#       hooks/                           (subprocess'd at runtime)
#         sentinel-gate.py
#         session-init.py
#         tool-router.py
#         transaction-enforcer.py
#         ... (all CC hook scripts)
#       lib/
#         project_resolver.py            (sibling lookup by hooks scripts)
#
# Maintainer workflow:
#   1. Pull the latest empirica into ~/.claude/plugins/local/empirica/
#   2. Run this script
#   3. Inspect git diff to see what drifted
#   4. Bump PLUGIN_VERSION in ecodex/scripts/install.sh if hook contract changed
#   5. Commit + rebuild + reinstall
#
# Source path can be overridden via EMPIRICA_PLUGIN_SOURCE (default: CC location).

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
ECODEX_ROOT="$(cd -- "${SCRIPT_DIR}/.." &> /dev/null && pwd)"
PLUGIN_ASSETS="${ECODEX_ROOT}/codex-rs/codex-empirica-plugin/assets"
SOURCE_ROOT="${EMPIRICA_PLUGIN_SOURCE:-${HOME}/.claude/plugins/local/empirica}"
SYSTEM_PROMPT_SRC="${HOME}/.claude/empirica-system-prompt.md"

if [[ ! -d "${SOURCE_ROOT}" ]]; then
  echo "sync-empirica-assets: source not found at ${SOURCE_ROOT}" >&2
  echo "Set EMPIRICA_PLUGIN_SOURCE to override (e.g. point at a fresh empirica checkout)." >&2
  exit 1
fi

if [[ ! -d "${SOURCE_ROOT}/hooks" ]] || [[ ! -d "${SOURCE_ROOT}/lib" ]]; then
  echo "sync-empirica-assets: ${SOURCE_ROOT} missing expected hooks/ or lib/ subdir" >&2
  exit 1
fi

# NOTE: the system prompt is intentionally NOT synced (see section 1) — the
# ecodex vendored copy is hand-authored, not a copy of ${SYSTEM_PROMPT_SRC}.
# We therefore do NOT require the Claude source prompt to exist.

echo "→ Source:  ${SOURCE_ROOT}"
echo "→ Target:  ${PLUGIN_ASSETS}"
echo ""

# ─── 1. System prompt — INTENTIONALLY NOT SYNCED ─────────────────────
# The vendored empirica-system-prompt.md is a hand-authored ecodex-NATIVE
# reminder (de-Claude'd in bdbbfc6625), compiled into the binary via
# include_str! (src/agents_md.rs). The Claude lean-core source at
# ${SYSTEM_PROMPT_SRC} is Claude-centric and ~6x longer — blindly copying it
# would revert the de-Claude work and change the compiled binary's behavior.
# Edit codex-rs/codex-empirica-plugin/assets/empirica-system-prompt.md
# directly if the ecodex reminder needs updating.
mkdir -p "${PLUGIN_ASSETS}"
echo "• empirica-system-prompt.md — preserved (ecodex-native, NOT synced from Claude source)"

# ─── 2. Hook scripts + shared lib (copied into plugin install at install time) ─
HOOKS_DEST="${PLUGIN_ASSETS}/hooks_scripts"
rm -rf "${HOOKS_DEST}"
mkdir -p "${HOOKS_DEST}"

cp -r "${SOURCE_ROOT}/hooks" "${HOOKS_DEST}/hooks"
cp -r "${SOURCE_ROOT}/lib"   "${HOOKS_DEST}/lib"

# Vendor the statusline script (Tx6(b)/4): the empirica plugin declares it
# in manifest.statusline; the codex tui invokes it on a 1.5s tick and
# renders captured stdout below the prompt.
if [[ -d "${SOURCE_ROOT}/scripts" ]]; then
  mkdir -p "${HOOKS_DEST}/scripts"
  # Be selective — only copy the statusline script, not other scripts/
  # entries that aren't part of the plugin contract.
  if [[ -f "${SOURCE_ROOT}/scripts/statusline_empirica.py" ]]; then
    cp "${SOURCE_ROOT}/scripts/statusline_empirica.py" "${HOOKS_DEST}/scripts/"
    chmod +x "${HOOKS_DEST}/scripts/statusline_empirica.py"
  fi
fi

# Strip Python bytecode caches — never want those in source control.
find "${HOOKS_DEST}" -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true
find "${HOOKS_DEST}" -type f -name "*.pyc" -delete 2>/dev/null || true

HOOK_COUNT=$(find "${HOOKS_DEST}/hooks" -type f -name "*.py" | wc -l)
LIB_COUNT=$(find "${HOOKS_DEST}/lib" -type f -name "*.py" | wc -l)
SCRIPT_COUNT=$(find "${HOOKS_DEST}/scripts" -type f -name "*.py" 2>/dev/null | wc -l)
TOTAL_SIZE=$(du -sh "${HOOKS_DEST}" | awk '{print $1}')

echo "✓ hooks_scripts/hooks/    (${HOOK_COUNT} python scripts)"
echo "✓ hooks_scripts/lib/      (${LIB_COUNT} python modules)"
echo "✓ hooks_scripts/scripts/  (${SCRIPT_COUNT} python scripts — statusline_empirica.py)"
echo "✓ Total bundled:          ${TOTAL_SIZE}"

# ─── 3. Subagents (copied into <codex_home>/agents/empirica/ at SessionStart) ─
AGENTS_DEST="${PLUGIN_ASSETS}/agents"
if [[ -d "${SOURCE_ROOT}/agents" ]]; then
  rm -rf "${AGENTS_DEST}"
  cp -r "${SOURCE_ROOT}/agents" "${AGENTS_DEST}"
  AGENT_COUNT=$(find "${AGENTS_DEST}" -type f -name "*.md" | wc -l)
  echo "✓ agents/               (${AGENT_COUNT} subagent definitions)"
else
  echo "⚠ agents/ missing at ${SOURCE_ROOT} — skipping (CC empirica may not have shipped subagents)"
fi

# ─── 3. Surface drift for the maintainer to review ───────────────────
echo ""
echo "Next: review drift with"
echo "  git -C \"${ECODEX_ROOT}\" diff --stat codex-rs/codex-empirica-plugin/assets/"
echo "  git -C \"${ECODEX_ROOT}\" status --short codex-rs/codex-empirica-plugin/assets/"
