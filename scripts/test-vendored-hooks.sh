#!/usr/bin/env bash
# Run the Python tests for the vendored empirica hook layer.
# The hooks are synced from empirica but execute inside ecodex, so we verify
# their behaviour here (not only py_compile + diff-against-upstream).
#
# Requires: pytest. The ai_id tests provide a hermetic InstanceResolver stub
# when empirica core is unavailable.
set -euo pipefail
cd "$(dirname "$0")/.."
exec python3 -m pytest codex-rs/codex-empirica-plugin/tests/vendored_hooks/ -v "$@"
