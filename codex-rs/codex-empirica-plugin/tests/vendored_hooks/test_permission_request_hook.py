"""permission-request.py records that codex asked a person to approve a call.

codex's PermissionRequest payload carries tool_name, tool_input, permission_mode and
turn_id, no tool_use_id, and the hook never receives the answer, so the hook writes a
`permission_request` line and core's `infer_overrides` reads the answer from what followed.
These tests run ecodex's script as codex runs it, a subprocess fed the payload on stdin.
They need empirica core 1.14.22 or newer installed, as CI's pinned checkout provides.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

_HOOK = (
    Path(__file__).resolve().parents[2]
    / "assets"
    / "hooks_scripts"
    / "hooks"
    / "permission-request.py"
)

pytest.importorskip(
    "empirica.core.decision_record", reason="needs empirica core with the decision record"
)


def _practice(tmp_path: Path) -> Path:
    (tmp_path / ".empirica").mkdir()
    (tmp_path / ".empirica" / "project.yaml").write_text("ai_id: test\n")
    return tmp_path


def _run(payload, tmp_path: Path, **env):
    full_env = {**os.environ, "EMPIRICA_HARNESS": "codex", "HOME": str(tmp_path / "home"), **env}
    return subprocess.run(
        [sys.executable, str(_HOOK)],
        input=payload if isinstance(payload, str) else json.dumps(payload),
        capture_output=True,
        text=True,
        env=full_env,
        cwd=tmp_path,
    )


def _lines(practice: Path) -> list[dict]:
    path = practice / ".empirica" / "decisions.jsonl"
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def _payload(practice: Path) -> dict:
    return {
        "session_id": "00000000-0000-0000-0000-000000000000",
        "hook_event_name": "PermissionRequest",
        "turn_id": "turn_7",
        "cwd": str(practice),
        "tool_name": "Bash",
        "tool_input": {"command": "git push origin main"},
        "permission_mode": "default",
    }


def test_a_permission_request_line_is_written_to_the_practice(tmp_path):
    practice = _practice(tmp_path)
    result = _run(_payload(practice), tmp_path)
    assert result.returncode == 0
    (line,) = _lines(practice)
    assert line["event"] == "permission_request"
    assert line["harness"] == "codex"
    assert line["turn_id"] == "turn_7"
    assert line["tool"] == "Bash"
    assert line["permission_mode"] == "default"
    assert "git push" not in json.dumps(line), "the raw command must never reach the record"


def test_the_hook_prints_nothing_so_it_cannot_decide_for_the_person(tmp_path):
    practice = _practice(tmp_path)
    result = _run(_payload(practice), tmp_path)
    assert result.stdout == ""


def test_malformed_input_still_exits_clean(tmp_path):
    practice = _practice(tmp_path)
    result = _run("not json", tmp_path)
    assert result.returncode == 0
    assert _lines(practice) == []


def test_the_off_switch_is_honoured(tmp_path):
    practice = _practice(tmp_path)
    result = _run(_payload(practice), tmp_path, EMPIRICA_DECISION_RECORD="0")
    assert result.returncode == 0
    assert _lines(practice) == []
