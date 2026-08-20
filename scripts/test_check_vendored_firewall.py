from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest

SCRIPT = Path(__file__).with_name("check_vendored_firewall.py")
SPEC = importlib.util.spec_from_file_location("check_vendored_firewall", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)


@pytest.mark.parametrize(
    ("source", "failure_count"),
    [
        ('# RECOVERY ESCAPE HATCH\n# is_safe_empirica_command\n# "permissionDecision"\n', 2),
        (
            "def _validate_check_record():\n"
            "    if is_safe_empirica_command('x'):\n        return None\n"
            "def respond(decision):\n    return {'permissionDecision': 'allow'}\n",
            1,
        ),
    ],
)
def test_dead_markers_do_not_manufacture_a_pass(
    tmp_path: Path, source: str, failure_count: int
) -> None:
    fake = tmp_path / "sentinel-gate.py"
    fake.write_text(source, encoding="utf-8")

    assert len(guard.check(fake)) == failure_count


def test_disabled_in_place_if_false_fails(tmp_path):
    """`if False and (...)` keeps markers AND structure; the guard must still
    fail. Demonstrated live against the structural-only guard (exit 0 with the
    escape hatch dead, 2026-08-21)."""
    import re

    import check_vendored_firewall as guard

    mutated = tmp_path / "sentinel-gate.py"
    text = guard.SENTINEL.read_text(encoding="utf-8")
    patched, n = re.subn(
        r"(\n    if )\((\n        tool_name in NOETIC_TOOLS)",
        r"\1False and (\2",
        text,
        count=1,
    )
    assert n == 1, "escape-hatch anchor moved — update this test"
    mutated.write_text(patched, encoding="utf-8")
    failures = guard.check(mutated)
    assert any("literal False" in f for f in failures), failures
