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

For each cargo-audit advisory, this asks the same question a human would:
"does anything actually depend on this?" via `cargo tree -i <pkg> --target
all` (the tool verified against ground truth during the 0.146.0 release --
a hand-rolled `cargo metadata` graph walk gives FALSE POSITIVES here
because it doesn't account for platform/feature-gated edges that `cargo
tree`'s own resolution correctly prunes).

Exit code: 1 if any advisory has a real reverse-dependent (SHIPPED), 0
otherwise (including when only phantom/orphan-lockfile advisories exist).
"""

from __future__ import annotations

import json
import subprocess
import sys


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
    """True if anything in the workspace's resolved graph actually depends
    on this exact (name, version) -- across all platform targets."""
    result = run(["cargo", "tree", "-i", f"{name}@{version}", "--target", "all"])
    if result.returncode != 0:
        # Package not in the lockfile at all (shouldn't happen -- audit
        # read the same lockfile) or a cargo error. Treat conservatively
        # as shipped so we don't silently swallow a real advisory.
        sys.stderr.write(f"warning: `cargo tree -i {name}@{version}` failed, treating as shipped:\n{result.stderr}\n")
        return True
    return "nothing to print" not in result.stdout and "nothing to print" not in result.stderr


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

    print(f"cargo-audit: {len(vulns)} advisory(ies) on Cargo.lock ({len(shipped)} shipped, {len(phantom)} phantom/lockfile-only)")

    if phantom:
        print(f"\nPHANTOM ({len(phantom)}) -- zero reverse-dependents in the resolved workspace graph, does not fail this gate:")
        for v in phantom:
            print(f"  - {v['advisory']['id']}  {v['package']['name']} {v['package']['version']}  ({v['advisory']['title']})")

    if shipped:
        print(f"\nSHIPPED ({len(shipped)}) -- has real reverse-dependents, FAILS this gate:")
        for v in shipped:
            print(f"  - {v['advisory']['id']}  {v['package']['name']} {v['package']['version']}  ({v['advisory']['title']})")
            print(f"    {v['advisory']['url']}")
        return 1

    print("\nNo shipped-dependency vulnerabilities.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
