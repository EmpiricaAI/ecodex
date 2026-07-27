"""tool-failure.py — a captured failure must clear a HIGH bar before it becomes a
permanent dead-end.

Mirrors empirica (tests/test_tool_failure_capture_filter.py) against ecodex's
VENDORED copy under assets/hooks_scripts/hooks/tool-failure.py. This is the fix
for the dead-end-noise defect: a dead-end is retrieved into later sessions as
"avoid re-trying", so a false positive doesn't merely add noise — it removes a
viable approach from the practice's option space. The dominant real miss was
timeouts (a timeout message is long, so the old >=20-char heuristic waved every
one through), including a `git commit` that SUCCEEDED but was recorded because a
CI-wait loop in the same command hit `timeout`.

Guards the three fix legs the vendored copy MUST carry (peer correction, empirica
core, 2026-07-27): IGNORE_PATTERNS (timeouts / SIGTERM 143 / SIGKILL 137 /
connection-refused / DNS), SUCCESS_MARKERS (own-text-shows-it-landed veto), and
that a genuine approach failure still lands.
"""
from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest

_HOOK = (
    Path(__file__).resolve().parents[2]
    / "assets"
    / "hooks_scripts"
    / "hooks"
    / "tool-failure.py"
)


@pytest.fixture(scope="module")
def hook():
    spec = importlib.util.spec_from_file_location("tool_failure_hook", _HOOK)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


@pytest.mark.parametrize(
    "error",
    [
        "Exit code 143\nCommand timed out after 10m 0s",  # THE dominant real case
        "Command timed out after 2m 0s and was moved to the background",
        "Exit code 137 — killed",
        "Error: operation timed out after 30s waiting for the run to finish",
    ],
)
def test_timeouts_and_signals_are_not_dead_ends(hook, error):
    """The clock or the harness killed it. That says nothing about whether the
    APPROACH works — and a timeout message is long, so the >=20-char heuristic waved
    every one of these straight through."""
    assert hook._is_interesting_failure("Bash", error) is False


def test_a_command_that_actually_worked_is_not_a_dead_end(hook):
    """The exact live case: the push landed; a wait loop in the same command timed
    out."""
    error = (
        "Exit code 143\nCommand timed out after 10m 0s\n"
        "   205c60bf2..678335bc7  develop -> develop"
    )
    assert hook._is_interesting_failure("Bash", error) is False


@pytest.mark.parametrize(
    "error",
    ["3 files changed, 40 insertions(+)", "Successfully installed empirica", "12 passed in 0.3s"],
)
def test_success_markers_veto_capture(hook, error):
    assert hook._is_interesting_failure("Bash", error) is False


def test_operational_outages_are_not_epistemic(hook):
    """A service being down is an operational fact with a lifetime of minutes; a
    dead-end is permanent."""
    conn_refused = "curl: (7) Failed to connect: Connection refused"
    dns_fail = "ssh: Could not resolve host: empirica-server"
    assert hook._is_interesting_failure("Bash", conn_refused) is False
    assert hook._is_interesting_failure("Bash", dns_fail) is False


def test_a_genuine_approach_failure_is_still_captured(hook):
    """The filter must not be so wide that nothing lands — this is the case the hook
    exists for."""
    error = (
        "ModuleNotFoundError: No module named 'qdrant_client.async_client' — "
        "the async API was removed in 1.9 and there is no drop-in replacement"
    )
    assert hook._is_interesting_failure("Bash", error) is True


def test_short_errors_remain_uninteresting(hook):
    assert hook._is_interesting_failure("Bash", "nope") is False
