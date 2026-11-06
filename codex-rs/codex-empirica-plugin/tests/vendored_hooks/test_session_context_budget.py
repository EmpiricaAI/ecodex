"""The startup project context stays bounded and under codex's hook-output cap.

codex caps each hook's additional context at 2,500 tokens, counted as
ceil(bytes / 4) per context text (codex-rs/hooks/src/output_spill.rs
DEFAULT_HOOK_OUTPUT_TOKEN_LIMIT, utils/string/src/truncate.rs
APPROX_BYTES_PER_TOKEN). Past the cap the full text is spilled to a file and the
model sees a head/tail preview, so an over-budget block is cut silently.

The block session-init.py builds from `project-bootstrap` grows with what the
bootstrap returns. Today the hook caps the items it reads before it renders, so
the block cannot outgrow the cap. Core's bootstrap now returns more (decisions,
assumptions, per-type coverage, blindspot status) and a future re-vendor may
render it. This test is the gate for that: it feeds the vendored hook a
bootstrap history of every category, at a small and at an enormous size, and
fails if the rendered project context crosses the cap or grows with the
history. It asserts the cap codex enforces, not a smaller target, since no
smaller target has been agreed with core.
"""

from __future__ import annotations

import importlib.util
import json
import math
import sys
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

_HOOKS = Path(__file__).resolve().parents[2] / "assets" / "hooks_scripts" / "hooks"

CODEX_HOOK_TOKEN_CAP = 2_500
BYTES_PER_TOKEN = 4

# Multi-byte text, so a count by characters instead of bytes would pass here and fail in codex.
_TEXT = "Ünïcode — a long artifact line that a real practice might have logged. " * 8


def _approx_tokens(text: str) -> int:
    return math.ceil(len(text.encode("utf-8")) / BYTES_PER_TOKEN)


def _load_session_init():
    sys.path.insert(0, str(_HOOKS))
    try:
        sys.modules.pop("session_init_budget_test", None)
        spec = importlib.util.spec_from_file_location(
            "session_init_budget_test", _HOOKS / "session-init.py"
        )
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module
    finally:
        sys.path.pop(0)


def _bootstrap_payload(items: int) -> dict:
    """The shape `project-bootstrap --output json` returns, every category at `items` entries."""

    def entries(key: str) -> list[dict]:
        return [{key: f"{index}: {_TEXT}", "id": f"id-{index}"} for index in range(items)]

    return {
        "ok": True,
        "breadcrumbs": {
            "goals": entries("objective"),
            "findings": entries("finding"),
            "unknowns": entries("unknown"),
            "decisions": entries("choice"),
            "dead_ends": entries("approach"),
            "mistakes_to_avoid": entries("mistake"),
            "assumptions": entries("assumption"),
            "artifact_edges": [{"from": "a", "to": "b", "relation": "evidence"}] * items,
            "coverage": {
                kind: {"loaded": items, "available": items * 3}
                for kind in ("findings", "unknowns", "decisions", "assumptions")
            },
            "truncation_warnings": [f"{_TEXT} {index}" for index in range(items)],
            "blindspot_status": {"total": items, "note": _TEXT},
        },
    }


def _project_context_block(items: int) -> str:
    """Run the hook's own bootstrap parse and renderer on a history of `items` per category."""
    module = _load_session_init()
    completed = SimpleNamespace(
        returncode=0, stdout=json.dumps(_bootstrap_payload(items)), stderr=""
    )
    with patch.object(module.subprocess, "run", return_value=completed):
        _data, project_context = module._run_bootstrap("budget-test-session", {})
    assert project_context is not None, "the hook must parse a bootstrap carrying every category"
    return module.format_context(project_context)


def test_the_project_context_stays_under_the_codex_hook_cap_for_a_huge_history():
    block = _project_context_block(items=20_000)
    tokens = _approx_tokens(block)
    assert tokens <= CODEX_HOOK_TOKEN_CAP, (
        f"the startup project context is {tokens} estimated tokens against codex's "
        f"{CODEX_HOOK_TOKEN_CAP}-token cap; codex would spill it and show the model a preview"
    )


def test_the_project_context_does_not_grow_with_the_history():
    small = _approx_tokens(_project_context_block(items=20))
    huge = _approx_tokens(_project_context_block(items=20_000))
    assert huge <= small, (
        f"the startup project context grew from {small} to {huge} estimated tokens as the history "
        "grew a thousandfold; a renderer must choose what to show before it formats"
    )


def test_the_yardstick_does_fire_on_an_unbounded_block():
    """Positive control: the counting above must be able to fail.

    Hand the renderer an uncapped context (what a future renderer would see if it
    skipped the selection pass) and require this file's own measure to put it over
    the cap. Without this, a bug in the measure would leave the two tests above
    green for the wrong reason.
    """
    module = _load_session_init()
    uncapped = {
        "goals": _bootstrap_payload(2_000)["breadcrumbs"]["goals"],
        "findings": _bootstrap_payload(2_000)["breadcrumbs"]["findings"],
        "unknowns": _bootstrap_payload(2_000)["breadcrumbs"]["unknowns"],
    }
    assert _approx_tokens(module.format_context(uncapped)) > CODEX_HOOK_TOKEN_CAP
