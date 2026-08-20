from __future__ import annotations

import importlib.util
import subprocess
from pathlib import Path

SCRIPT = Path(__file__).with_name("scoped_cargo_audit.py")
SPEC = importlib.util.spec_from_file_location("scoped_cargo_audit", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
audit = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(audit)


def _result(stdout: str = "", stderr: str = "", returncode: int = 0):
    return subprocess.CompletedProcess([], returncode, stdout, stderr)


def test_dev_only_workspace_dependency_is_not_called_shipped(monkeypatch) -> None:
    calls: list[list[str]] = []

    def fake_run(command: list[str]):
        calls.append(command)
        return _result(stderr="warning: nothing to print.\n")

    monkeypatch.setattr(audit, "run", fake_run)

    assert audit.has_reverse_dependents("pretty_assertions", "1.4.1") is False
    assert [command[3] for command in calls] == list(audit.SHIPPED_ROOTS)
    assert all("normal,build" in command for command in calls)


def test_dependency_of_any_release_root_is_shipped(monkeypatch) -> None:
    results = iter(
        [
            _result(stderr="warning: nothing to print.\n"),
            _result(stdout="vulnerable-crate v1.2.3\n└── codex-empirica-plugin\n"),
        ]
    )
    monkeypatch.setattr(audit, "run", lambda _command: next(results))

    assert audit.has_reverse_dependents("vulnerable-crate", "1.2.3") is True


def test_broken_cargo_tree_fails_closed(monkeypatch) -> None:
    monkeypatch.setattr(
        audit,
        "run",
        lambda _command: _result(stderr="cargo metadata failed", returncode=101),
    )

    assert audit.has_reverse_dependents("vulnerable-crate", "1.2.3") is True
