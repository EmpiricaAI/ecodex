#!/usr/bin/env python3
"""Turn broccoli.sh's clippy and cargo test output into the evidence files Empirica's POSTFLIGHT
reads.

    broccoli_evidence.py clippy <clippy --message-format=json stream> --cargo-status N \
        --rustfmt-ok true|false --packages a,b,c --scope S --out PATH
    broccoli_evidence.py tests <cargo test output> --cargo-status N --scope S --out PATH

The files follow empirica core's "Test evidence (any language)" contract:
`.empirica/code_quality.json` carries `metrics` (a boolean or a non-negative count each) and
`.empirica/test_report.json` carries `summary` (`passed`, `failed`). Each is written atomically and
stamped when the step ends. A step that produced no results writes nothing and removes the file it
would have written, so an earlier pass cannot stand in for a run that measured nothing. When the
target directory does not exist (no `.empirica/`, as in CI) nothing is written.

The exit code is the gate: clippy passes with no warnings and no errors, tests pass when cargo test
succeeded.
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
    r"^test result: (ok"
    r"|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out",
    re.MULTILINE,
)
# How cargo test reports a target that failed, per target and in --no-fail-fast's closing list, and
# a binary killed by a signal.
_TARGET_FAILED = re.compile(r"^error: (?:test|doctest) failed, to rerun pass", re.MULTILINE)
_TARGETS_FAILED = re.compile(r"^error: (\d+) targets? failed:", re.MULTILINE)
# Terminal control sequences: CSI (colours, erase line) and OSC (hyperlinks).
_ANSI = re.compile(r"\x1b\[[0-9;:?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)")
_SIGNALLED = re.compile(r"process didn't exit successfully: .*\(signal: ")
# rustc's closing lines, which arrive as diagnostics with no span; any other spanless warning (an
# unknown lint) is a finding.
_SUMMARY = re.compile(
    r"^(?:aborting due to\b|\d+ (?:warning|error)s?(?: and \d+ (?:warning|error)s?)? emitted$)"
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
    """Write `payload` to `target`, or remove `target` when the step measured nothing. Returns what
    happened, for the log."""
    if not target.parent.is_dir():
        return f"{target.parent} does not exist: no evidence file written"
    if payload is None:
        target.unlink(missing_ok=True)
        return f"nothing measured: {target.name} not written"
    write_atomically(target, {**payload, "runner": RUNNER, "created": time.time()})
    return f"wrote {target}"


def package_name(package_id: str) -> str:
    """The package name in a cargo package id: `path+file:///…/dir#name@1.0`,
    `path+file:///…/name#1.0`, or `name 1.0 (source)`."""
    if "#" in package_id:
        url, fragment = package_id.rsplit("#", 1)
        return fragment.split("@", 1)[0] if "@" in fragment else url.rstrip("/").rsplit("/", 1)[-1]
    return package_id.split(" ", 1)[0]


def clippy_counts(stream: str, packages: set[str] | None = None) -> tuple[dict | None, list[str]]:
    """(counts, rendered diagnostics) from a cargo --message-format=json stream; counts is None when
    the build did not finish.

    Only diagnostics from `packages` count (all when None): with `--no-deps` clippy lints just the
    named packages, but cargo still prints rustc's warnings for workspace crates they depend on,
    which the old `-D warnings` gate never failed on. A diagnostic is counted once however many
    targets report it (`--all-targets` lints a lib again as its test target). rustc's summary lines
    (`N warnings emitted`, `aborting due to ...`) are not findings.
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
        if packages is not None and package_name(str(msg.get("package_id") or "")) not in packages:
            continue
        diag = msg.get("message") or {}
        spans = diag.get("spans") or []
        if not spans and _SUMMARY.match(str(diag.get("message") or "")):
            continue
        primary = next((s for s in spans if s.get("is_primary")), spans[0] if spans else {})
        key = (
            diag.get("level"),
            (diag.get("code") or {}).get("code"),
            diag.get("message"),
            primary.get("file_name"),
            primary.get("line_start"),
            primary.get("column_start"),
        )
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
    """Summed `test result:` lines, or None when they cannot describe the run.

    None when no result line was printed, when cargo failed with no failing test (a build error), or
    when more targets failed than printed a FAILED result (a binary that crashed or was killed never
    prints one, so its tests would be missing from the counts).
    """
    output = _ANSI.sub("", output)  # CARGO_TERM_COLOR=always colours `error:` and the result word
    matches = list(_TEST_RESULT.finditer(output))
    if not matches:
        return None
    rows = [tuple(int(n) for n in m.groups()[1:]) for m in matches]
    passed, failed, ignored, _measured, filtered = (sum(col) for col in zip(*rows))
    if cargo_status != 0 and failed == 0:
        return None  # a build error or a crashed binary: the counts would read as a pass
    failed_results = sum(1 for m in matches if m.group(1) == "FAILED")
    listed = max((int(n) for n in _TARGETS_FAILED.findall(output)), default=0)
    if (
        _SIGNALLED.search(output)
        or max(len(_TARGET_FAILED.findall(output)), listed) > failed_results
    ):
        return None
    return {
        "passed": passed,
        "failed": failed,
        "ignored": ignored,
        "filtered_out": filtered,
        "binaries": len(rows),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").split("\n\n")[0])
    parser.add_argument("step", choices=("clippy", "tests"))
    parser.add_argument("log", type=Path, help="the step's captured output")
    parser.add_argument("--cargo-status", type=int, required=True)
    parser.add_argument("--scope", required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument(
        "--rustfmt-ok",
        choices=("true", "false"),
        help="clippy step: whether rustfmt --check passed",
    )
    parser.add_argument(
        "--packages",
        help="clippy step: comma-separated packages whose diagnostics count (default: all)",
    )
    args = parser.parse_args(argv)
    text = args.log.read_text(errors="replace")

    if args.step == "clippy":
        owned = {name for name in (args.packages or "").split(",") if name} or None
        counts, rendered = clippy_counts(text, owned)
        for diag in rendered:
            print(diag, end="" if diag.endswith("\n") else "\n")
        payload = None
        if counts is not None:
            metrics: dict = {
                "clippy_warnings": counts["warnings"],
                "clippy_errors": counts["errors"],
            }
            if args.rustfmt_ok is not None:
                metrics = {"rustfmt_ok": args.rustfmt_ok == "true", **metrics}
            payload = {"scope": args.scope, "metrics": metrics}
            print(f"  clippy: {counts['warnings']} warning(s), {counts['errors']} error(s)")
        print(f"  evidence: {emit(args.out, payload)}")
        return (
            0
            if args.cargo_status == 0
            and counts is not None
            and counts["warnings"] == 0
            and counts["errors"] == 0
            else 1
        )

    counts = cargo_test_counts(text, args.cargo_status)
    payload = None if counts is None else {"scope": args.scope, "summary": counts}
    print(f"  evidence: {emit(args.out, payload)}")
    return 0 if args.cargo_status == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
