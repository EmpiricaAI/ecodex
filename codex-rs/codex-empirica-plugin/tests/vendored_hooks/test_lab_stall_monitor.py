import importlib.util
import json
import sqlite3
from pathlib import Path

import pytest

_SCRIPT = (
    Path(__file__).resolve().parents[2]
    / "assets"
    / "hooks_scripts"
    / "scripts"
    / "lab_stall_monitor.py"
)
_SPEC = importlib.util.spec_from_file_location("lab_stall_monitor", _SCRIPT)
assert _SPEC is not None and _SPEC.loader is not None
lab_stall_monitor = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(lab_stall_monitor)


def _write_transaction(
    project: Path,
    *,
    instance: str = "worker-1",
    transaction_id: str = "tx-1",
    status: str = "open",
    updated_at: float = 100.0,
    tool_call_count: int = 4,
) -> Path:
    empirica = project / ".empirica"
    empirica.mkdir(parents=True, exist_ok=True)
    path = empirica / f"active_transaction_{instance}.json"
    path.write_text(
        json.dumps(
            {
                "transaction_id": transaction_id,
                "session_id": "session-1",
                "claude_session_id": instance,
                "status": status,
                "preflight_timestamp": 50.0,
                "updated_at": updated_at,
                "tool_call_count": tool_call_count,
                "project_path": str(project),
            }
        )
    )
    return path


def _write_presence(home: Path, *, instance: str = "worker-1", ai_id: str = "ecodex-lab") -> None:
    empirica = home / ".empirica"
    empirica.mkdir(parents=True, exist_ok=True)
    (empirica / f"practitioner_presence_{instance}.json").write_text(
        json.dumps(
            {
                "claude_session_id": instance,
                "practice_ai_id": ai_id,
                "status": "active",
                "location": "tmux_9",
                "session_pid": 1234,
            }
        )
    )


def _write_reflex(project: Path, *, phase: str = "CHECK", decision: str = "proceed") -> None:
    db_dir = project / ".empirica" / "sessions"
    db_dir.mkdir(parents=True, exist_ok=True)
    connection = sqlite3.connect(db_dir / "sessions.db")
    try:
        connection.execute(
            "CREATE TABLE reflexes ("
            "session_id TEXT, transaction_id TEXT, phase TEXT, "
            "timestamp REAL, reflex_data TEXT)"
        )
        connection.execute(
            "INSERT INTO reflexes VALUES (?, ?, ?, ?, ?)",
            ("session-1", "tx-1", phase, 101.0, json.dumps({"decision": decision})),
        )
        connection.commit()
    finally:
        connection.close()


def _detector(project: Path, home: Path, **kwargs):
    return lab_stall_monitor.LabStallDetector(
        project,
        instance_id=kwargs.pop("instance_id", "worker-1"),
        ai_id=kwargs.pop("ai_id", "ecodex-lab"),
        stall_seconds=kwargs.pop("stall_seconds", 300.0),
        home=home,
        pid_probe=kwargs.pop("pid_probe", lambda _pid: False),
        tmux_probe=kwargs.pop("tmux_probe", lambda location: location == "tmux_9"),
        **kwargs,
    )


def test_stall_event_uses_transaction_activity_and_db_phase(tmp_path):
    project = tmp_path / "ecodex-lab"
    home = tmp_path / "home"
    transaction_path = _write_transaction(project)
    _write_presence(home)
    _write_reflex(project)

    events = _detector(project, home).scan(now=401.0)

    assert events == [
        {
            "event": "lab_stall",
            "ai_id": "ecodex-lab",
            "instance_id": "worker-1",
            "project_path": str(project.resolve()),
            "transaction_file": str(transaction_path),
            "transaction_id": "tx-1",
            "session_id": "session-1",
            "claude_session_id": "worker-1",
            "status": "open",
            "phase": "CHECK",
            "gate_decision": "proceed",
            "updated_at": 100.0,
            "preflight_timestamp": 50.0,
            "tool_call_count": 4,
            "stalled_for_seconds": 301.0,
            "threshold_seconds": 300.0,
            "process_alive": True,
        }
    ]


@pytest.mark.parametrize(
    ("status", "updated_at", "tmux_alive"),
    [
        ("closed", 100.0, True),
        ("open", 102.0, True),
        ("open", 100.0, False),
    ],
)
def test_no_event_when_closed_fresh_or_process_dead(tmp_path, status, updated_at, tmux_alive):
    project = tmp_path / "ecodex-lab"
    home = tmp_path / "home"
    _write_transaction(project, status=status, updated_at=updated_at)
    _write_presence(home)
    detector = _detector(project, home, stall_seconds=2.0, tmux_probe=lambda _location: tmux_alive)

    assert detector.scan(now=103.0) == []


def test_event_deduplicates_until_updated_at_advances(tmp_path):
    project = tmp_path / "ecodex-lab"
    home = tmp_path / "home"
    _write_transaction(project)
    _write_presence(home)
    detector = _detector(project, home, stall_seconds=10.0)

    assert len(detector.scan(now=111.0)) == 1
    assert detector.scan(now=200.0) == []

    _write_transaction(project, updated_at=205.0, tool_call_count=5)
    assert detector.scan(now=210.0) == []
    resumed_events = detector.scan(now=216.0)

    assert len(resumed_events) == 1
    assert resumed_events[0]["updated_at"] == 205.0
    assert resumed_events[0]["tool_call_count"] == 5


def test_instance_filter_prevents_cross_practitioner_event(tmp_path):
    project = tmp_path / "ecodex-lab"
    home = tmp_path / "home"
    _write_transaction(project, instance="worker-1")
    _write_presence(home, instance="worker-1")

    detector = _detector(project, home, instance_id="worker-2", stall_seconds=1.0)

    assert detector.scan(now=200.0) == []


def test_allow_unverified_process_is_explicit_escape_hatch(tmp_path):
    project = tmp_path / "ecodex-lab"
    home = tmp_path / "home"
    _write_transaction(project)
    detector = _detector(
        project,
        home,
        stall_seconds=1.0,
        require_live_process=False,
    )

    events = detector.scan(now=200.0)

    assert len(events) == 1
    assert events[0]["process_alive"] is False


def test_pid_liveness_fallback_when_not_in_tmux():
    presence = {
        "practice_ai_id": "ecodex-lab",
        "location": "headless",
        "session_pid": 4321,
    }

    assert lab_stall_monitor.practitioner_is_alive(
        presence,
        ai_id="ecodex-lab",
        pid_probe=lambda pid: pid == 4321,
        tmux_probe=lambda _location: False,
    )


@pytest.mark.parametrize(
    ("output", "expected"),
    [
        ("0\tecodex\n", True),
        ("0\tclaude\n", True),
        ("0\tbash\n", False),
        ("1\tecodex\n", False),
        ("malformed", False),
    ],
)
def test_tmux_liveness_requires_a_running_worker_not_just_a_live_pane(output, expected):
    assert lab_stall_monitor._tmux_output_has_live_worker(output) is expected


def test_malformed_transaction_is_ignored(tmp_path):
    project = tmp_path / "ecodex-lab"
    empirica = project / ".empirica"
    empirica.mkdir(parents=True)
    (empirica / "active_transaction_worker-1.json").write_text("{not-json")

    assert lab_stall_monitor.open_transactions(project) == []
