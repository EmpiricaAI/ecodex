"""truncation-legibility.py must work on the payload codex actually sends.

Empirica wrote the hook against Claude Code's PostToolUse(Bash) payload, where
`tool_response` is an object with `stdout`. codex reports a finished shell call
as tool_name "Bash" with `tool_input.command` and `tool_response` as a plain
string of the output (core: tools/handlers/unified_exec.rs, tools/context.rs
ExecCommandToolOutput). These tests run ecodex's VENDORED copy as codex runs it
— a subprocess fed that payload on stdin — so a re-vendor that stops handling
a string response goes red here rather than going quiet in sessions.
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

_HOOK = (
    Path(__file__).resolve().parents[2]
    / "assets"
    / "hooks_scripts"
    / "hooks"
    / "truncation-legibility.py"
)


def _run(tool_name: str, command: str, output: str) -> str:
    payload = {
        "session_id": "00000000-0000-0000-0000-000000000000",
        "hook_event_name": "PostToolUse",
        "tool_name": tool_name,
        "tool_use_id": "call_1",
        "tool_input": {"command": command},
        "tool_response": output,
    }
    result = subprocess.run(
        [sys.executable, str(_HOOK)],
        input=json.dumps(payload),
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout


def test_output_that_fills_its_head_limit_gets_a_notice():
    out = _run("Bash", "rg -n TODO src | head -3", "a.rs:1\nb.rs:2\nc.rs:3\n")
    context = json.loads(out)["hookSpecificOutput"]["additionalContext"]
    assert "returned exactly 3 lines" in context


def test_output_shorter_than_its_limit_is_left_alone():
    assert _run("Bash", "rg -n TODO src | head -3", "a.rs:1\nb.rs:2\n") == ""


def test_declared_page_in_a_string_response_gets_a_notice():
    out = _run("Bash", "curl -s api/items", '{"items": [1, 2], "has_more": true}')
    context = json.loads(out)["hookSpecificOutput"]["additionalContext"]
    assert "declared itself partial (has_more)" in context


def test_other_tools_are_ignored():
    assert _run("apply_patch", "rg -n TODO src | head -3", "a\nb\nc\n") == ""
