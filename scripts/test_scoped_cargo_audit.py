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


def _vuln(advisory_id: str, crate: str, version: str = "1.0.0") -> dict:
    return {
        "advisory": {"id": advisory_id, "title": "t", "url": "u"},
        "package": {"name": crate, "version": version},
    }


def _entry(advisory_id: str, crate: str, **extra) -> dict:
    entry = {
        "id": advisory_id,
        "crate": crate,
        "reason": "fix needs an upstream semver-major bump",
        "recorded": "2026-10-27",
        "recheck": "each upstream sync",
    }
    entry.update(extra)
    return entry


def _audit_with(monkeypatch, vulns: list[dict], accepted: list[dict], tmp_path) -> int:
    path = tmp_path / "accepted.json"
    path.write_text(audit.json.dumps({"accepted": accepted}))
    monkeypatch.setattr(audit, "cargo_audit_json", lambda: {"vulnerabilities": {"list": vulns}})
    monkeypatch.setattr(audit, "has_reverse_dependents", lambda _name, _version: True)
    return audit.main(["--accepted", str(path)])


def test_an_accepted_shipped_advisory_does_not_fail(monkeypatch, tmp_path, capsys) -> None:
    code = _audit_with(
        monkeypatch, [_vuln("RUSTSEC-1", "zstd")], [_entry("RUSTSEC-1", "zstd")], tmp_path
    )
    assert code == 0
    assert "ACCEPTED (1)" in capsys.readouterr().out


def test_an_unaccepted_shipped_advisory_still_fails(monkeypatch, tmp_path) -> None:
    vulns = [_vuln("RUSTSEC-1", "zstd"), _vuln("RUSTSEC-2", "other")]
    assert _audit_with(monkeypatch, vulns, [_entry("RUSTSEC-1", "zstd")], tmp_path) == 1


def test_an_entry_for_another_crate_accepts_nothing(monkeypatch, tmp_path) -> None:
    code = _audit_with(
        monkeypatch, [_vuln("RUSTSEC-1", "zstd")], [_entry("RUSTSEC-1", "zstd-safe")], tmp_path
    )
    assert code == 1


def test_a_stale_entry_fails_so_the_list_follows_reality(monkeypatch, tmp_path, capsys) -> None:
    code = _audit_with(monkeypatch, [], [_entry("RUSTSEC-1", "zstd")], tmp_path)
    assert code == 1
    assert "STALE" in capsys.readouterr().out


def test_an_entry_without_a_reason_is_refused(monkeypatch, tmp_path) -> None:
    unreasoned = [_entry("RUSTSEC-1", "zstd", reason="")]
    assert _audit_with(monkeypatch, [_vuln("RUSTSEC-1", "zstd")], unreasoned, tmp_path) == 2


def test_the_checked_in_list_is_well_formed() -> None:
    entries = audit.load_accepted(audit.DEFAULT_ACCEPTED)
    assert entries, "the accept list exists and is not empty"
