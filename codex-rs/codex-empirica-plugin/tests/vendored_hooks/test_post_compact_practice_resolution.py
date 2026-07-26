"""post-compact practice-resolution regression tests for ecodex's VENDORED hook layer.

Mirrors empirica PR #246 (tests/test_post_compact_practice_resolution.py) against
the vendored copy under ``assets/hooks_scripts/hooks/post-compact.py``. Guards the
vendor-agnostic practice resolution the ecodex ``run_hook`` wrapper depends on
(``empirica_cli.rs`` sets ``EMPIRICA_CWD_RELIABLE=true`` — the codex CWD==practice
vouch):

  - ``EMPIRICA_CWD_RELIABLE=true`` → find_project_root gets allow_cwd_fallback +
    allow_git_root, so a FRESH codex practitioner (empty instance_projects cache)
    still resolves its practice from the filesystem. This is the case that
    otherwise made post-compact.py exit 1 → "SessionStart hook (failed)".
  - flag unset (tmux/multiplexer where CWD may be the *launch* dir) → fallback
    stays OFF (KNOWN_ISSUES 11.10 cross-project-bleed guard).
  - nothing resolves → graceful exit 0 (a boundary hook with no practice is a
    no-op), NOT exit 1.
"""
from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
from unittest.mock import patch

import pytest

# The hook's module-level `from project_resolver import ...` (not try/excepted)
# must resolve; conftest puts assets/hooks_scripts/lib on sys.path.
pytest.importorskip("project_resolver")

_HOOKS = Path(__file__).resolve().parents[2] / "assets" / "hooks_scripts" / "hooks"


def _load_post_compact():
    """Import the hyphenated post-compact.py hook as a fresh module."""
    sys.path.insert(0, str(_HOOKS))
    try:
        sys.modules.pop("post_compact_hook", None)
        spec = importlib.util.spec_from_file_location(
            "post_compact_hook", _HOOKS / "post-compact.py"
        )
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        return mod
    finally:
        sys.path.pop(0)


def test_cwd_fallback_enabled_when_cwd_reliable_true(monkeypatch, tmp_path):
    """EMPIRICA_CWD_RELIABLE=true → harness vouches CWD → filesystem fallback enabled."""
    monkeypatch.setenv("EMPIRICA_CWD_RELIABLE", "true")
    pc = _load_post_compact()
    with (
        patch.object(pc, "find_project_root", return_value=tmp_path) as fpr,
        patch.object(pc.os, "chdir"),
        patch.object(pc, "get_instance_id", return_value="i"),
    ):
        pc._resolve_project_and_setup("cc-1")
    kwargs = fpr.call_args.kwargs
    assert kwargs["allow_cwd_fallback"] is True
    assert kwargs["allow_git_root"] is True


def test_cwd_fallback_off_when_unset(monkeypatch, tmp_path):
    """No flag → CWD untrusted (could be the launch dir) → fallback stays off."""
    monkeypatch.delenv("EMPIRICA_CWD_RELIABLE", raising=False)
    pc = _load_post_compact()
    with (
        patch.object(pc, "find_project_root", return_value=tmp_path) as fpr,
        patch.object(pc.os, "chdir"),
        patch.object(pc, "get_instance_id", return_value="i"),
    ):
        pc._resolve_project_and_setup("cc-1")
    kwargs = fpr.call_args.kwargs
    assert kwargs["allow_cwd_fallback"] is False
    assert kwargs["allow_git_root"] is False


def test_unresolved_exits_zero_not_one(monkeypatch):
    """Nothing resolves → graceful no-op (exit 0), not a 'hook failed' (exit 1)."""
    monkeypatch.delenv("EMPIRICA_CWD_RELIABLE", raising=False)
    pc = _load_post_compact()
    with (
        patch.object(pc, "find_project_root", return_value=None),
        pytest.raises(SystemExit) as exc,
    ):
        pc._resolve_project_and_setup("cc-fresh")
    assert exc.value.code == 0
