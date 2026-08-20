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

Exit code: 1 if any advisory has a real reverse-dependent (SHIPPED), 0
otherwise (including when only phantom/orphan-lockfile advisories exist).
"""

from __future__ import annotations

import json
import subprocess
import sys

SHIPPED_ROOTS = (
    "codex-cli",
    "codex-empirica-plugin",
    "codex-empirica-translator",
)


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


def main() -> int:
    audit = cargo_audit_json()
    vulns = audit.get("vulnerabilities", {}).get("list", [])

    if not vulns:
        print("cargo-audit: 0 advisories. Clean.")
        return 0

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

    if shipped:
        print(f"\nSHIPPED ({len(shipped)}) -- has real reverse-dependents, FAILS this gate:")
        for v in shipped:
            print(
                f"  - {v['advisory']['id']}  {v['package']['name']} "
                f"{v['package']['version']}  ({v['advisory']['title']})"
            )
            print(f"    {v['advisory']['url']}")
        return 1

    print("\nNo shipped-dependency vulnerabilities.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
