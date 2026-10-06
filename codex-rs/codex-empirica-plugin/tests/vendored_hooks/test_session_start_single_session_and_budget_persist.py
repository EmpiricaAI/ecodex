"""One empirica session per codex SessionStart, and budget persistence that never re-resolves a path.

Pins, against the VENDORED hooks, the two fixes empirica 1.14.8 made after David's
2026-10-04/05 reports (goal 1c7051bb):

1. ``post-compact.py`` runs at EVERY codex SessionStart (codex fires every SessionStart
   hook, session-init and post-compact both), so it must act only on a compaction.
   ``source`` is ``startup | resume | clear | fork | compact`` on the wire
   (``codex-rs/hooks/src/events/session_start.rs``). Anything but ``compact`` is
   skipped before any session is touched; a payload with no usable ``source`` is
   judged as before, so a real compaction is never dropped for lack of a field.
   Without the guard, a start whose previous session was complete created a second
   empirica session 0.26 s after session-init created the first.

2. ``session-init.py`` persists budget state only into a ``sessions.db`` it already
   holds. When it does not hold one it skips the write and says why in its own
   words. It must never hand ``persist_state`` no path: that re-resolves the
   location from scratch and, at the moment the hook has failed to find its root,
   the resolver answers "Cannot determine sessions.db path ... run project-init",
   which the model reads as a broken install inside an initialised practice.

The hooks are vendored verbatim; ecodex pins their behaviour so a re-vendor that
loses either fix fails here instead of in a user's session.
"""

from __future__ import annotations

import importlib
import importlib.util
import io
import json
import sys
from pathlib import Path
from unittest.mock import patch

import pytest

_HOOKS = Path(__file__).resolve().parents[2] / "assets" / "hooks_scripts" / "hooks"


def _load(module_name: str, filename: str):
    """Import a hyphenated vendored hook as a fresh module."""
    sys.path.insert(0, str(_HOOKS))
    try:
        sys.modules.pop(module_name, None)
        spec = importlib.util.spec_from_file_location(module_name, _HOOKS / filename)
        assert spec is not None and spec.loader is not None, f"cannot load {filename}"
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        return mod
    finally:
        sys.path.pop(0)


class _Reached(Exception):
    """Raised by the patched first stage so a test can tell the guard let the run through."""


def _run_post_compact_main(payload: dict) -> tuple[bool, str]:
    """Run post-compact's main() on `payload`. Returns (reached_stage_1, stdout)."""
    pc = _load("post_compact_hook_single_session", "post-compact.py")
    out = io.StringIO()
    reached = False
    with (
        patch.object(pc.sys, "stdin", io.StringIO(json.dumps(payload))),
        patch.object(pc, "_resolve_project_and_setup", side_effect=_Reached),
        patch.object(pc.sys, "stdout", out),
    ):
        try:
            pc.main()
        except _Reached:
            reached = True
        except SystemExit as exit_info:
            assert exit_info.code in (0, None), "a skipped SessionStart must exit 0"
    return reached, out.getvalue()


# --- post-compact: only a compaction needs recovering -------------------------------------


@pytest.mark.parametrize("source", ["startup", "resume", "clear", "fork"])
def test_post_compact_skips_every_non_compact_session_start(source):
    reached, stdout = _run_post_compact_main({"session_id": "thread-1", "source": source})
    assert reached is False, f"source={source} must not reach practice resolution or session creation"
    result = json.loads(stdout)
    assert result["ok"] is True and result["skipped"] is True
    assert source in result["reason"]


def test_post_compact_acts_on_a_compaction():
    reached, _ = _run_post_compact_main({"session_id": "thread-1", "source": "compact"})
    assert reached is True


@pytest.mark.parametrize(
    "payload",
    [
        {"session_id": "thread-1"},
        {"session_id": "thread-1", "source": None},
        {"session_id": "thread-1", "source": ""},
        {"session_id": "thread-1", "source": 3},
    ],
    ids=["absent", "null", "empty", "not-a-string"],
)
def test_post_compact_never_drops_a_payload_it_cannot_judge(payload):
    reached, _ = _run_post_compact_main(payload)
    assert reached is True, "no usable `source` means we cannot tell; a compaction must not be dropped"


# --- session-init: persist into the db the hook holds, or say why not ---------------------


@pytest.fixture
def session_init():
    return _load("session_init_hook_budget", "session-init.py")


@pytest.fixture
def real_budget_module():
    """The production context-budget module. Its absence is a failure, not a skip."""
    module = importlib.import_module("empirica.core.context_budget")
    assert hasattr(module, "ContextBudgetManager"), (
        "these tests need the real empirica core (CI installs the pinned checkout); "
        "the hermetic stub in conftest.py cannot stand in for it"
    )
    return module


def _practice(tmp_path: Path, *, with_db: bool) -> Path:
    root = tmp_path / "practice"
    (root / ".empirica" / "sessions").mkdir(parents=True)
    if with_db:
        (root / ".empirica" / "sessions" / "sessions.db").write_bytes(b"")
    return root


def test_budget_db_is_the_practices_own_store_when_it_exists(session_init, tmp_path, monkeypatch):
    monkeypatch.delenv("EMPIRICA_SESSION_DB", raising=False)
    root = _practice(tmp_path, with_db=True)
    assert session_init._budget_db_path(root) == root / ".empirica" / "sessions" / "sessions.db"


def test_budget_db_is_none_without_a_root_or_without_the_file(session_init, tmp_path, monkeypatch):
    monkeypatch.delenv("EMPIRICA_SESSION_DB", raising=False)
    assert session_init._budget_db_path(None) is None
    assert session_init._budget_db_path(_practice(tmp_path, with_db=False)) is None


def test_budget_db_honours_an_explicit_override_the_resolver_would_use(session_init, tmp_path, monkeypatch):
    override = tmp_path / "ci-sessions.db"
    override.write_bytes(b"")
    monkeypatch.setenv("EMPIRICA_SESSION_DB", str(override))
    assert session_init._budget_db_path(None) == override


def test_unpersisted_reason_names_the_cause_source_and_cwd_without_blaming_the_install(session_init, tmp_path):
    no_root = session_init._unpersisted_reason(None, {"source": "compact"}, "/work/thing")
    assert no_root == (
        "budget state not persisted: practice root unknown for this SessionStart (source=compact, cwd=/work/thing)"
    )
    root = _practice(tmp_path, with_db=False)
    no_db = session_init._unpersisted_reason(root, {"type": "resume"}, "/work/thing")
    assert f"sessions.db not found under {root}" in no_db and "source=resume" in no_db
    unknown = session_init._unpersisted_reason(None, {}, "/work/thing")
    assert "source=unknown" in unknown
    for text in (no_root, no_db, unknown):
        assert "project-init" not in text, "inside an initialised practice that remedy is wrong and reads as a broken install"


def test_without_a_db_path_the_budget_is_not_persisted_and_the_resolver_never_speaks(
    session_init, real_budget_module, tmp_path, monkeypatch
):
    monkeypatch.chdir(tmp_path)  # not a git repository, no practice
    monkeypatch.delenv("EMPIRICA_SESSION_DB", raising=False)
    reason = "budget state not persisted: practice root unknown for this SessionStart (source=compact, cwd=x)"
    with patch.object(
        real_budget_module.ContextBudgetManager,
        "persist_state",
        side_effect=AssertionError("persist_state must not be reached without a db path"),
    ):
        summary = session_init._init_context_budget("sess-1", {}, db_path=None, unpersisted_reason=reason)
    assert "error" not in summary, summary
    assert summary["persist"] == f"skipped: {reason}"
    assert "Cannot determine" not in json.dumps(summary)


def test_with_a_db_path_the_hook_passes_that_path_through(
    session_init, real_budget_module, tmp_path, monkeypatch
):
    monkeypatch.delenv("EMPIRICA_SESSION_DB", raising=False)
    db = _practice(tmp_path, with_db=True) / ".empirica" / "sessions" / "sessions.db"
    with patch.object(real_budget_module.ContextBudgetManager, "persist_state", return_value=True) as persist:
        summary = session_init._init_context_budget("sess-1", {"goals": [{"objective": "ship"}]}, db_path=db)
    persist.assert_called_once_with(db_path=db)
    assert summary["persist"] == "persisted"
