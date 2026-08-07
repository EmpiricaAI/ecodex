#!/usr/bin/env python3
"""Emit Monitor-compatible events when a live Empirica transaction stalls.

The transaction file is the activity signal: Sentinel advances ``updated_at``
and ``tool_call_count`` on every tool call. Old open transaction files are
common, so a frozen timestamp is only considered a stall when the matching
practitioner process is also demonstrably alive.
"""

import argparse
import errno
import json
import os
import re
import sqlite3
import subprocess
import sys
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

DEFAULT_STALL_SECONDS = 10 * 60
DEFAULT_POLL_SECONDS = 15.0
EVENT_NAME = "lab_stall"
_TRANSACTION_PREFIX = "active_transaction"
_TMUX_LOCATION_RE = re.compile(r"^tmux_(\d+)$")
_SHELL_COMMANDS = frozenset({"bash", "cmd", "fish", "nu", "pwsh", "sh", "zsh"})


@dataclass(frozen=True)
class TransactionSnapshot:
    path: Path
    transaction_id: str
    session_id: str
    claude_session_id: str | None
    status: str
    updated_at: float
    preflight_timestamp: float | None
    tool_call_count: int | None
    project_path: str | None

    @property
    def instance_id(self) -> str | None:
        if self.claude_session_id:
            return self.claude_session_id
        suffix = self.path.stem.removeprefix(_TRANSACTION_PREFIX).removeprefix("_")
        return suffix or None


def _read_json(path: Path) -> dict | None:
    try:
        value = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return None
    return value if isinstance(value, dict) else None


def _as_float(value: object) -> float | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        return float(value)
    return None


def read_transaction(path: Path) -> TransactionSnapshot | None:
    data = _read_json(path)
    if data is None:
        return None
    updated_at = _as_float(data.get("updated_at"))
    transaction_id = data.get("transaction_id")
    session_id = data.get("session_id")
    if updated_at is None or not isinstance(transaction_id, str) or not isinstance(session_id, str):
        return None
    count = data.get("tool_call_count")
    return TransactionSnapshot(
        path=path,
        transaction_id=transaction_id,
        session_id=session_id,
        claude_session_id=(
            data.get("claude_session_id")
            if isinstance(data.get("claude_session_id"), str)
            else None
        ),
        status=data.get("status") if isinstance(data.get("status"), str) else "",
        updated_at=updated_at,
        preflight_timestamp=_as_float(data.get("preflight_timestamp")),
        tool_call_count=count if isinstance(count, int) and not isinstance(count, bool) else None,
        project_path=(
            data.get("project_path") if isinstance(data.get("project_path"), str) else None
        ),
    )


def _matches_instance(snapshot: TransactionSnapshot, instance_id: str | None) -> bool:
    if not instance_id:
        return True
    normalized = instance_id.removeprefix("_").replace(":", "_").replace("%", "")
    return normalized in {
        snapshot.instance_id,
        snapshot.claude_session_id,
        snapshot.session_id,
    }


def open_transactions(
    project_path: Path, instance_id: str | None = None
) -> list[TransactionSnapshot]:
    empirica_dir = project_path / ".empirica"
    snapshots = []
    for path in sorted(empirica_dir.glob("active_transaction*.json")):
        snapshot = read_transaction(path)
        if snapshot is None or snapshot.status != "open":
            continue
        if _matches_instance(snapshot, instance_id):
            snapshots.append(snapshot)
    return sorted(snapshots, key=lambda snapshot: snapshot.updated_at, reverse=True)


def _pid_alive(pid: int) -> bool:
    if pid <= 0:
        return False
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    except OSError as error:
        return error.errno == errno.EPERM
    return True


def _tmux_output_has_live_worker(output: str) -> bool:
    fields = output.strip().split("\t", maxsplit=1)
    if len(fields) != 2:
        return False
    pane_dead, current_command = fields
    return pane_dead == "0" and bool(current_command) and current_command not in _SHELL_COMMANDS


def _tmux_pane_alive(location: str) -> bool:
    match = _TMUX_LOCATION_RE.fullmatch(location)
    if match is None:
        return False
    try:
        result = subprocess.run(
            [
                "tmux",
                "display-message",
                "-p",
                "-t",
                f"%{match.group(1)}",
                "#{pane_dead}\t#{pane_current_command}",
            ],
            capture_output=True,
            text=True,
            timeout=2,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return False
    return result.returncode == 0 and _tmux_output_has_live_worker(result.stdout)


def read_presence(home: Path, instance_id: str | None) -> dict | None:
    if not instance_id:
        return None
    return _read_json(home / ".empirica" / f"practitioner_presence_{instance_id}.json")


def practitioner_is_alive(
    presence: dict | None,
    *,
    ai_id: str | None,
    pid_probe: Callable[[int], bool] = _pid_alive,
    tmux_probe: Callable[[str], bool] = _tmux_pane_alive,
) -> bool:
    if presence is None:
        return False
    if ai_id and presence.get("practice_ai_id") != ai_id:
        return False
    if presence.get("status") in {"closed", "ended", "inactive"}:
        return False
    location = presence.get("location")
    if isinstance(location, str) and tmux_probe(location):
        return True
    pid = presence.get("session_pid")
    return isinstance(pid, int) and not isinstance(pid, bool) and pid_probe(pid)


def read_phase(project_path: Path, snapshot: TransactionSnapshot) -> tuple[str | None, str | None]:
    db_path = project_path / ".empirica" / "sessions" / "sessions.db"
    if not db_path.is_file():
        return None, None
    try:
        connection = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True, timeout=1)
        try:
            row = connection.execute(
                """
                SELECT phase, reflex_data
                FROM reflexes
                WHERE session_id = ? AND transaction_id = ?
                ORDER BY timestamp DESC
                LIMIT 1
                """,
                (snapshot.session_id, snapshot.transaction_id),
            ).fetchone()
        finally:
            connection.close()
    except (OSError, sqlite3.Error):
        return None, None
    if row is None:
        return None, None
    phase = row[0] if isinstance(row[0], str) else None
    gate_decision = None
    if isinstance(row[1], str):
        try:
            reflex_data = json.loads(row[1])
            if isinstance(reflex_data, dict) and isinstance(reflex_data.get("decision"), str):
                gate_decision = reflex_data["decision"]
        except json.JSONDecodeError:
            pass
    return phase, gate_decision


class LabStallDetector:
    def __init__(
        self,
        project_path: Path,
        *,
        instance_id: str | None,
        ai_id: str | None,
        stall_seconds: float,
        home: Path,
        require_live_process: bool = True,
        pid_probe: Callable[[int], bool] = _pid_alive,
        tmux_probe: Callable[[str], bool] = _tmux_pane_alive,
    ) -> None:
        self.project_path = project_path.resolve()
        self.instance_id = instance_id
        self.ai_id = ai_id
        self.stall_seconds = stall_seconds
        self.home = home
        self.require_live_process = require_live_process
        self.pid_probe = pid_probe
        self.tmux_probe = tmux_probe
        self._emitted: set[tuple[Path, str, float]] = set()

    def scan(self, *, now: float | None = None) -> list[dict]:
        now = time.time() if now is None else now
        events = []
        active_keys: set[tuple[Path, str, float]] = set()
        for snapshot in open_transactions(self.project_path, self.instance_id):
            key = (snapshot.path, snapshot.transaction_id, snapshot.updated_at)
            active_keys.add(key)
            presence = read_presence(self.home, snapshot.instance_id)
            alive = practitioner_is_alive(
                presence,
                ai_id=self.ai_id,
                pid_probe=self.pid_probe,
                tmux_probe=self.tmux_probe,
            )
            if self.require_live_process and not alive:
                continue
            stalled_for = max(0.0, now - snapshot.updated_at)
            if stalled_for < self.stall_seconds or key in self._emitted:
                continue
            phase, gate_decision = read_phase(self.project_path, snapshot)
            events.append(
                {
                    "event": EVENT_NAME,
                    "ai_id": self.ai_id,
                    "instance_id": snapshot.instance_id,
                    "project_path": str(self.project_path),
                    "transaction_file": str(snapshot.path),
                    "transaction_id": snapshot.transaction_id,
                    "session_id": snapshot.session_id,
                    "claude_session_id": snapshot.claude_session_id,
                    "status": snapshot.status,
                    "phase": phase,
                    "gate_decision": gate_decision,
                    "updated_at": snapshot.updated_at,
                    "preflight_timestamp": snapshot.preflight_timestamp,
                    "tool_call_count": snapshot.tool_call_count,
                    "stalled_for_seconds": round(stalled_for, 3),
                    "threshold_seconds": self.stall_seconds,
                    "process_alive": alive,
                }
            )
            self._emitted.add(key)
        self._emitted.intersection_update(active_keys)
        return events


def _project_ai_id(project_path: Path) -> str:
    project_data = _read_json(project_path / ".empirica" / "project.json")
    if project_data and isinstance(project_data.get("ai_id"), str):
        return project_data["ai_id"]
    try:
        import yaml

        yaml_data = yaml.safe_load((project_path / ".empirica" / "project.yaml").read_text()) or {}
        if isinstance(yaml_data, dict) and isinstance(yaml_data.get("ai_id"), str):
            return yaml_data["ai_id"]
    except (ImportError, OSError, ValueError):
        pass
    return project_path.name


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--project",
        required=True,
        type=Path,
        help="Project containing .empirica transaction state",
    )
    parser.add_argument(
        "--instance",
        help="Transaction suffix, Codex/Claude session id, or Empirica session id",
    )
    parser.add_argument(
        "--ai-id",
        help="Expected practice ai_id (defaults to project configuration)",
    )
    parser.add_argument(
        "--threshold-seconds",
        type=float,
        default=DEFAULT_STALL_SECONDS,
        help=f"Frozen updated_at duration before emitting (default: {DEFAULT_STALL_SECONDS})",
    )
    parser.add_argument(
        "--poll-seconds",
        type=float,
        default=DEFAULT_POLL_SECONDS,
        help=f"Polling interval (default: {DEFAULT_POLL_SECONDS})",
    )
    parser.add_argument("--once", action="store_true", help="Scan once instead of polling forever")
    parser.add_argument(
        "--allow-unverified-process",
        action="store_true",
        help=(
            "Allow events without a live tmux pane/PID "
            "(use only when process namespaces prevent verification)"
        ),
    )
    args = parser.parse_args(argv)
    if args.threshold_seconds < 0:
        parser.error("--threshold-seconds must be non-negative")
    if args.poll_seconds <= 0:
        parser.error("--poll-seconds must be positive")
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    project_path = args.project.resolve()
    if not (project_path / ".empirica").is_dir():
        print(f"lab-stall monitor: no .empirica directory under {project_path}", file=sys.stderr)
        return 2
    detector = LabStallDetector(
        project_path,
        instance_id=args.instance,
        ai_id=args.ai_id or _project_ai_id(project_path),
        stall_seconds=args.threshold_seconds,
        home=Path.home(),
        require_live_process=not args.allow_unverified_process,
    )
    while True:
        for event in detector.scan():
            print(json.dumps(event, sort_keys=True), flush=True)
        if args.once:
            return 0
        time.sleep(args.poll_seconds)


if __name__ == "__main__":
    sys.exit(main())
