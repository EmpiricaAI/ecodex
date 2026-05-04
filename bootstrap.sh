#!/usr/bin/env bash
# ecodex bootstrap.sh — single-command install for fresh-clone users.
#
# Mirrors the "one command, just works" UX of `npm install -g <pkg>` /
# `pip install <pkg>`. After cloning the ecodex repo, run:
#
#     ./bootstrap.sh             # per-user install
#     sudo ./bootstrap.sh --system   # system-wide install
#
# What it does:
#   1. Verifies cargo + curl are on PATH (or fails fast with install hints).
#   2. Runs cargo build --release -p codex-cli -p codex-empirica-plugin
#      (no-op if already up-to-date — cargo handles incremental builds).
#   3. Runs ecodex/scripts/install.sh which copies the binary, wrapper,
#      plugin cache, hooks_scripts/, agents/, and statusline script into
#      the right locations + ensures ~/.codex/config.toml has the
#      plugins."empirica@nubaeon" enable line.
#   4. Verifies the install end-to-end (binaries executable, plugin
#      manifest declares statusline, statusline script runs, etc.).
#   5. Prints next steps.
#
# Forwards any extra args (e.g. --system, --prefix DIR, --no-build) to
# the underlying install.sh. See ecodex/scripts/install.sh --help.

set -euo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
INSTALLER="${REPO_ROOT}/ecodex/scripts/install.sh"

if [[ ! -x "$INSTALLER" ]]; then
  echo "bootstrap: ${INSTALLER} not found or not executable" >&2
  echo "  Make sure you cloned the ecodex repo and are running from its root." >&2
  exit 1
fi

# Pre-flight: required tools.
if ! command -v cargo >/dev/null 2>&1; then
  echo "bootstrap: cargo not found on PATH" >&2
  echo "  Install Rust: https://rustup.rs/   then re-run ./bootstrap.sh" >&2
  exit 1
fi

# install.sh handles build + install + verification. Forward all args.
exec "$INSTALLER" "$@"
