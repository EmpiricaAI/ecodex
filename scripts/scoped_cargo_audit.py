#!/usr/bin/env python3
"""Scoped cargo-audit: only fail the gate for advisories that actually ship.

`cargo audit` scans the whole Cargo.lock. On a large fork-of-a-fork
workspace that includes a lot of code we don't ship (upstream codex crates,
platform/feature-gated deps, dev/test-only deps), that surfaces advisories
on crates that never compile into the ecodex binary at all -- a Cargo.lock
phantom entry, not a real exposure. (Concretely: RUSTSEC-2026-0185 /
quinn-proto 0.11.14, found 2026-08-04 -- 0 reverse-dependents anywhere in
the resolved workspace graph, only present in Cargo.lock. See finding
805bac8d in the ecodex project.)

For each cargo-audit advisory, this asks whether one of the three release roots
depends on it through normal/build edges. A workspace-wide inverse tree is not
authoritative: it includes dev dependencies and crates ecodex does not ship.

A shipped advisory whose fix ecodex cannot take on its own (it needs an upstream
dependency to move a major version) can be accepted in cargo_audit_accepted.json,
each entry with its id, crate, reason, the date it was recorded and when to check it
again. An accepted advisory is listed and does not fail the gate. An entry that no
shipped advisory matches any more is STALE and fails it, so the list is pruned when
upstream moves instead of quietly accepting a future reappearance.

Exit code: 1 if a shipped advisory is not accepted, or an accept entry is stale; 2
if cargo audit or the accept list cannot be read; 0 otherwise (including when only
phantom/orphan-lockfile advisories exist).
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

SHIPPED_ROOTS = (
    "codex-cli",
    "codex-empirica-plugin",
    "codex-empirica-translator",
)
DEFAULT_ACCEPTED = Path(__file__).with_name("cargo_audit_accepted.json")
_ENTRY_FIELDS = ("id", "crate", "reason", "recorded", "recheck")


def load_accepted(path: Path) -> dict[str, dict]:
    """The accept list by advisory id. Raises ValueError for a malformed list or entry."""
    data = json.loads(path.read_text())
    entries = data.get("accepted") if isinstance(data, dict) else None
    if not isinstance(entries, list):
        raise ValueError(f"{path}: expected an object with an `accepted` list")
    accepted: dict[str, dict] = {}
    for n, entry in enumerate(entries, start=1):
        fields = entry if isinstance(entry, dict) else {}
        missing = [f for f in _ENTRY_FIELDS if not str(fields.get(f, "")).strip()]
        if missing:
            raise ValueError(f"{path}: entry {n} needs {', '.join(missing)}")
        accepted[entry["id"]] = entry
    return accepted


def run(cmd: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True, check=False)


def cargo_audit_json() -> dict:
    result = run(["cargo", "audit", "--json"])
    if result.returncode not in (0, 1):
        # cargo-audit exits 1 when it finds vulnerabilities -- that's data,
        # not a tool failure. Anything else is a real problem.
        sys.stderr.write(f"cargo audit failed:\n{result.stderr}\n")
        sys.exit(2)
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError:
        sys.stderr.write(f"non-JSON output from cargo audit:\n{result.stdout[:2000]}\n")
        sys.exit(2)


def has_reverse_dependents(name: str, version: str) -> bool:
    """True if a shipped release root depends on this exact package."""
    package = f"{name}@{version}"
    for root in SHIPPED_ROOTS:
        command = [
            "cargo",
            "tree",
            "-p",
            root,
            "--edges",
            "normal,build",
            "-i",
            package,
            "--target",
            "all",
        ]
        result = run(command)
        if result.returncode != 0:
            sys.stderr.write(
                f"warning: `{' '.join(command)}` failed, treating as shipped:\n{result.stderr}\n"
            )
            return True
        output = result.stdout + result.stderr
        if "nothing to print" not in output:
            return True
    return False


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").split("\n\n")[0])
    parser.add_argument("--accepted", type=Path, default=DEFAULT_ACCEPTED, help="the accept list")
    args = parser.parse_args(argv)
    try:
        accepted = load_accepted(args.accepted) if args.accepted.exists() else {}
    except (OSError, ValueError) as error:
        sys.stderr.write(f"accept list unreadable: {error}\n")
        return 2

    audit = cargo_audit_json()
    vulns = audit.get("vulnerabilities", {}).get("list", [])

    shipped, phantom = [], []
    for v in vulns:
        pkg = v["package"]
        (shipped if has_reverse_dependents(pkg["name"], pkg["version"]) else phantom).append(v)

    print(
        f"cargo-audit: {len(vulns)} advisory(ies) on Cargo.lock "
        f"({len(shipped)} shipped, {len(phantom)} phantom/lockfile-only)"
    )

    if phantom:
        print(
            f"\nPHANTOM ({len(phantom)}) -- no release-root normal/build "
            "reverse-dependents; does not fail this gate:"
        )
        for v in phantom:
            print(
                f"  - {v['advisory']['id']}  {v['package']['name']} "
                f"{v['package']['version']}  ({v['advisory']['title']})"
            )

    def is_accepted(v: dict) -> bool:
        entry = accepted.get(v["advisory"]["id"])
        return entry is not None and entry["crate"] == v["package"]["name"]

    taken = [v for v in shipped if is_accepted(v)]
    failing = [v for v in shipped if not is_accepted(v)]
    matched = {v["advisory"]["id"] for v in taken}
    stale = [e for advisory_id, e in accepted.items() if advisory_id not in matched]

    if taken:
        print(
            f"\nACCEPTED ({len(taken)}) -- shipped, accepted in {args.accepted.name}; "
            "does not fail:"
        )
        for v in taken:
            entry = accepted[v["advisory"]["id"]]
            print(
                f"  - {v['advisory']['id']}  {v['package']['name']} {v['package']['version']}  "
                f"(recorded {entry['recorded']}; re-check {entry['recheck']})"
            )

    if failing:
        print(f"\nSHIPPED ({len(failing)}) -- has real reverse-dependents, FAILS this gate:")
        for v in failing:
            print(
                f"  - {v['advisory']['id']}  {v['package']['name']} "
                f"{v['package']['version']}  ({v['advisory']['title']})"
            )
            print(f"    {v['advisory']['url']}")

    if stale:
        print(
            f"\nSTALE ({len(stale)}) -- accepted in {args.accepted.name} but no shipped advisory "
            "matches; remove these entries:"
        )
        for entry in stale:
            print(f"  - {entry['id']}  {entry['crate']}")

    if failing or stale:
        return 1
    print("\nNo unaccepted shipped-dependency vulnerabilities.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
