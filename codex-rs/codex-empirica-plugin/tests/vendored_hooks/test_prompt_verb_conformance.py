"""Prompt-verb conformance guard for the ecodex-native system prompt.

The vendored ``empirica-system-prompt.md`` is hand-authored (intentionally NOT
synced from the Claude lean-core prompt — see scripts/sync-empirica-assets.sh
section 1). Because it is compiled into the binary via ``include_str!`` and is a
separate source of truth from empirica's own prompt, it can drift: an edit can
reference an ``empirica <verb>`` that doesn't exist (a phantom verb), and nothing
would catch it until a model in the field runs the bad command.

This guard closes that gap (the ecodex analogue of empirica's #291
prompt-parser conformance test): every ``empirica <verb>`` token referenced in
the prompt MUST resolve to a real empirica CLI subcommand.

Skips cleanly when the ``empirica`` CLI isn't on PATH (e.g. a minimal CI image),
so it never produces a false red — it only fails on a genuine phantom verb.
"""

from __future__ import annotations

import re
import shutil
import subprocess
from pathlib import Path

import pytest

# codex-empirica-plugin/tests/vendored_hooks/ -> codex-empirica-plugin/
_PLUGIN_ROOT = Path(__file__).resolve().parents[2]
_PROMPT = _PLUGIN_ROOT / "assets" / "empirica-system-prompt.md"

# `empirica <verb>` where <verb> is a subcommand token (letters, digits,
# hyphens). Stops at the first non-token char, so flags/args aren't captured.
_VERB_RE = re.compile(r"\bempirica\s+([a-z][a-z0-9-]*)")

# Tokens that follow "empirica" but are NOT subcommands (global flags / prose).
_NON_VERBS = {"is", "the", "cli", "and", "or", "commands", "session"}


def _referenced_verbs() -> set[str]:
    text = _PROMPT.read_text(encoding="utf-8")
    return {m.group(1) for m in _VERB_RE.finditer(text)} - _NON_VERBS


def _empirica_available() -> bool:
    return shutil.which("empirica") is not None


def test_prompt_file_exists():
    assert _PROMPT.is_file(), f"ecodex system prompt missing at {_PROMPT}"


def test_every_referenced_verb_is_a_real_empirica_command():
    if not _empirica_available():
        pytest.skip("empirica CLI not on PATH — cannot validate verbs")

    verbs = _referenced_verbs()
    assert verbs, "no `empirica <verb>` references found — regex or prompt changed?"

    phantom: list[str] = []
    for verb in sorted(verbs):
        # A real subcommand's --help exits 0; an unknown choice exits non-zero
        # (argparse: "invalid choice"). Cheap, format-independent validity probe.
        proc = subprocess.run(
            ["empirica", verb, "--help"],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if proc.returncode != 0:
            phantom.append(verb)

    assert not phantom, (
        "ecodex prompt references empirica verbs that are NOT real CLI commands "
        f"(phantom-verb drift): {phantom}. Fix the prompt or the command name."
    )
