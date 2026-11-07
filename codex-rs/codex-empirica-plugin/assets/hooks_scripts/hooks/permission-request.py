#!/usr/bin/env python3
"""PermissionRequest: record that codex asked a person to approve a call.

ecodex-owned, not vendored from empirica: Claude Code has no hook at this moment, and
`scripts/setup-codex.py` leaves files empirica does not carry untouched.

codex's PermissionRequest hook fires when it asks a person to approve a call, with tool_name,
tool_input, permission_mode and turn_id. It carries no tool_use_id and the hook never receives the
answer, so this writes one `permission_request` line to the decision record
(`.empirica/decisions.jsonl`) and core's `infer_overrides` reads the answer from what followed,
joined on turn_id and the input hash. The answer is inferred and says so; no hook line ever claims
to have observed it.

It prints nothing, so it can never approve or deny a call, and it exits 0 on every path. Needs
empirica core 1.14.22 or newer; with an older or missing core it does nothing.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


def _decision_record():
    """empirica/core/decision_record.py, loaded from its file (the package init costs about 22 ms).

    The same loader core's own post hooks carry. None when empirica is not installed.
    """
    try:
        import importlib.util

        spec = importlib.util.find_spec("empirica")
        if spec is None or not spec.submodule_search_locations:
            return None
        path = Path(next(iter(spec.submodule_search_locations))) / "core" / "decision_record.py"
        module_spec = importlib.util.spec_from_file_location("_empirica_decision_record", path)
        if module_spec is None or module_spec.loader is None:
            return None
        module = importlib.util.module_from_spec(module_spec)
        module_spec.loader.exec_module(module)
        return module
    except Exception:
        return None


def main() -> int:
    try:
        payload = json.load(sys.stdin)
    except Exception:
        return 0
    record = _decision_record()
    if record is not None and hasattr(record, "record_permission_request"):
        record.record_permission_request(payload)
    return 0


if __name__ == "__main__":
    sys.exit(main())
