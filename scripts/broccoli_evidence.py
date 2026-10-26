#!/usr/bin/env python3
"""Turn broccoli.sh's clippy and cargo test output into the evidence files Empirica's POSTFLIGHT reads.

    broccoli_evidence.py clippy <clippy --message-format=json stream> --cargo-status N --rustfmt-ok true|false --scope S --out PATH
    broccoli_evidence.py tests  <cargo test output> --cargo-status N --scope S --out PATH

The files follow empirica core's "Test evidence (any language)" contract: `.empirica/code_quality.json` carries `metrics` (a boolean or a
non-negative count each) and `.empirica/test_report.json` carries `summary` (`passed`, `failed`). Each is written atomically and stamped
when the step ends. A step that produced no results writes nothing and removes the file it would have written, so an earlier pass cannot
stand in for a run that measured nothing. When the target directory does not exist (no `.empirica/`, as in CI) nothing is written.

The exit code is the gate: clippy passes with no warnings and no errors, tests pass when cargo test succeeded.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import tempfile
import time
from pathlib import Path

RUNNER = "broccoli.sh"
_TEST_RESULT = re.compile(
    r"^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out", re.MULTILINE
)


def write_atomically(target: Path, payload: dict) -> None:
    fd, tmp = tempfile.mkstemp(dir=target.parent, prefix=f".{target.stem}.", suffix=".tmp")
    try:
        with os.fdopen(fd, "w") as fh:
            json.dump(payload, fh)
        os.replace(tmp, target)
    except BaseException:
        Path(tmp).unlink(missing_ok=True)
        raise


def emit(target: Path, payload: dict | None) -> str:
    """Write `payload` to `target`, or remove `target` when the step measured nothing. Returns what happened, for the log."""
    if not target.parent.is_dir():
        return f"{target.parent} does not exist: no evidence file written"
    if payload is None:
        target.unlink(missing_ok=True)
        return f"nothing measured: {target.name} not written"
    write_atomically(target, {**payload, "runner": RUNNER, "created": time.time()})
    return f"wrote {target}"


def clippy_counts(stream: str) -> tuple[dict | None, list[str]]:
    """(counts, rendered diagnostics) from a cargo --message-format=json stream; counts is None when the build did not finish.

    A diagnostic is counted once however many targets report it (`--all-targets` lints a lib again as its test target). Summary lines
    (`N warnings emitted`, `aborting due to ...`) carry no span and are not findings.
    """
    seen: set[tuple] = set()
    rendered: list[str] = []
    warnings = errors = 0
    finished = None
    for line in stream.splitlines():
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if not isinstance(msg, dict):
            continue
        if msg.get("reason") == "build-finished":
            finished = bool(msg.get("success"))
            continue
        if msg.get("reason") != "compiler-message":
            continue
        diag = msg.get("message") or {}
        spans = diag.get("spans") or []
        if not spans:
            continue
        primary = next((s for s in spans if s.get("is_primary")), spans[0])
        key = (diag.get("level"), (diag.get("code") or {}).get("code"), diag.get("message"),
               primary.get("file_name"), primary.get("line_start"), primary.get("column_start"))
        if key in seen:
            continue
        seen.add(key)
        if diag.get("level") == "warning":
            warnings += 1
        elif diag.get("level") == "error":
            errors += 1
        else:
            continue
        rendered.append(diag.get("rendered") or diag.get("message") or "")
    if finished is None or (not finished and errors == 0):
        return None, rendered  # no build, or a failure no diagnostic explains: nothing was linted
    return {"warnings": warnings, "errors": errors}, rendered


def cargo_test_counts(output: str, cargo_status: int) -> dict | None:
    """Summed `test result:` lines, or None when they cannot describe the run (none printed, or cargo failed with no failing test)."""
    rows = [tuple(int(n) for n in m.groups()) for m in _TEST_RESULT.finditer(output)]
    if not rows:
        return None
    passed, failed, ignored, _measured, filtered = (sum(col) for col in zip(*rows))
    if cargo_status != 0 and failed == 0:
        return None  # a build error or a crashed binary: the counts would read as a pass
    return {"passed": passed, "failed": failed, "ignored": ignored, "filtered_out": filtered, "binaries": len(rows)}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").split("\n\n")[0])
    parser.add_argument("step", choices=("clippy", "tests"))
    parser.add_argument("log", type=Path, help="the step's captured output")
    parser.add_argument("--cargo-status", type=int, required=True)
    parser.add_argument("--scope", required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--rustfmt-ok", choices=("true", "false"), help="clippy step: whether rustfmt --check passed")
    args = parser.parse_args(argv)
    text = args.log.read_text(errors="replace")

    if args.step == "clippy":
        counts, rendered = clippy_counts(text)
        for diag in rendered:
            print(diag, end="" if diag.endswith("\n") else "\n")
        payload = None
        if counts is not None:
            metrics: dict = {"clippy_warnings": counts["warnings"], "clippy_errors": counts["errors"]}
            if args.rustfmt_ok is not None:
                metrics = {"rustfmt_ok": args.rustfmt_ok == "true", **metrics}
            payload = {"scope": args.scope, "metrics": metrics}
            print(f"  clippy: {counts['warnings']} warning(s), {counts['errors']} error(s)")
        print(f"  evidence: {emit(args.out, payload)}")
        return 0 if args.cargo_status == 0 and counts is not None and counts["warnings"] == 0 and counts["errors"] == 0 else 1

    counts = cargo_test_counts(text, args.cargo_status)
    payload = None if counts is None else {"scope": args.scope, "summary": counts}
    print(f"  evidence: {emit(args.out, payload)}")
    return 0 if args.cargo_status == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
