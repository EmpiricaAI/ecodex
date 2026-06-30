#!/usr/bin/env bash
# ecodex wrapper — exports the strict-mode env vars that the empirica
# plugin reads, then exec's the actual ecodex binary.
#
# Installed as `ecodex` on PATH; user invokes this when they type
# `ecodex ...`. The real Rust binary lives at ECODEX_BINARY_PATH (set
# by the install script).

set -euo pipefail

# ─── Strict-mode empirica env vars (E layer of A+B+E discipline) ─────
# These tighten the calibration loop. See comments in
# ecodex/config.toml.default for details on each.

export EMPIRICA_SENTINEL_REQUIRE_BOOTSTRAP="${EMPIRICA_SENTINEL_REQUIRE_BOOTSTRAP:-true}"
export EMPIRICA_SENTINEL_COMPACT_INVALIDATION="${EMPIRICA_SENTINEL_COMPACT_INVALIDATION:-true}"
export EMPIRICA_SENTINEL_CHECK_EXPIRY="${EMPIRICA_SENTINEL_CHECK_EXPIRY:-true}"
export EMPIRICA_CALIBRATION_FEEDBACK="${EMPIRICA_CALIBRATION_FEEDBACK:-true}"

# (Note: these use ${VAR:-default} syntax so an environment that
# already sets them takes precedence. Useful for development overrides
# without editing this script.)

# ─── Cortex mesh auth (opt-in; mesh installs only) ───────────────────
# codex's cortex MCP client reads its bearer from $CORTEX_API_KEY. If
# that's not already in the env but the user has an empirica credentials
# file carrying a cortex key, export it here so mesh installs authenticate
# regardless of shell rc (the 401 some sessions hit). OSS-only users are
# unaffected: with no cortex key (and no cortex server in the default
# config) nothing is exported and nothing changes. Reads the user's own
# ~/.empirica/credentials.yaml — no secret is hardcoded — and fails
# silently if python3 / pyyaml / the file are absent.
if [[ -z "${CORTEX_API_KEY:-}" && -f "${HOME}/.empirica/credentials.yaml" ]]; then
  _ecodex_ck="$(python3 -c "import yaml,sys; d=yaml.safe_load(open(sys.argv[1])) or {}; print(((d.get('cortex') or {}).get('api_key') or '').strip())" "${HOME}/.empirica/credentials.yaml" 2>/dev/null || true)"
  [[ -n "${_ecodex_ck:-}" ]] && export CORTEX_API_KEY="$_ecodex_ck"
  unset _ecodex_ck
fi

# ─── Locate + exec the binary ────────────────────────────────────────
# The install script patches this line to the resolved binary path.
ECODEX_BINARY_PATH="__ECODEX_BINARY_PATH_PLACEHOLDER__"

if [[ ! -x "$ECODEX_BINARY_PATH" ]]; then
  echo "ecodex wrapper: binary not found at $ECODEX_BINARY_PATH" >&2
  echo "  This wrapper was probably not installed correctly. Reinstall:" >&2
  echo "    cd <ecodex-source> && ./ecodex/scripts/install.sh" >&2
  exit 1
fi

exec "$ECODEX_BINARY_PATH" "$@"
