#!/usr/bin/env python3
"""setup-codex — empirica → ecodex import + de-Claude pipeline (maintainer tool).

Evolves sync-empirica-assets.sh from a blind copy into a smart, verifying sync:

  1. IMPORT   — per-file diff of ecodex's vendored layer against empirica@<ref>
                (hooks/lib/scripts + agents). Updates drifted files; reports
                empirica-NEW files; never deletes ecodex-only files (e.g. the
                extra native agents).
  2. DE-CLAUDE SCAN — flags *model-facing* Claude-isms in the synced content
                (agent/skill prose, hook strings the model reads) while
                allowlisting legit protocol/identifier/path references. Does NOT
                auto-rewrite — surfaces what a human (or upstream) must address.
  3. VERIFY   — py_compile changed Python + run the vendored_hooks test suite.
  4. DEPLOY   — (opt-in) copy synced files into the runtime plugin cache.

What it deliberately does NOT touch:
  - prompt-empirica.md / empirica-system-prompt.md — curated, canonical,
    already de-Claude'd. Not a sync target.
  - The hook Python's internal `claude_session_id` vars, `~/.claude` CC-compat
    paths, hook-contract docstrings — load-bearing, vendored verbatim.

Default is DRY-RUN. Pass --apply to write, --deploy to push to the cache.

Usage:
    python3 scripts/setup-codex.py                 # dry-run: show drift + flags
    python3 scripts/setup-codex.py --apply         # write drifted files
    python3 scripts/setup-codex.py --apply --deploy
    python3 scripts/setup-codex.py --ref develop --empirica ~/empirical-ai/empirica
"""
from __future__ import annotations

import argparse
import ast
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
PLUGIN = REPO / "codex-rs" / "codex-empirica-plugin"
ASSETS = PLUGIN / "assets"
DEFAULT_EMPIRICA = Path.home() / "empirical-ai" / "empirica"
EMP_PLUGIN = "empirica/plugins/claude-code-integration"

# ecodex vendored dir → empirica source subdir
DIR_MAP = {
    ASSETS / "hooks_scripts" / "hooks": f"{EMP_PLUGIN}/hooks",
    ASSETS / "hooks_scripts" / "lib": f"{EMP_PLUGIN}/lib",
    ASSETS / "hooks_scripts" / "scripts": f"{EMP_PLUGIN}/scripts",
    ASSETS / "agents": f"{EMP_PLUGIN}/agents",
}

# Hooks ecodex has deliberately RETIRED (pre-adoption dead-surface prune,
# 2026-06-24): the loop/listener install+uninstall-pickup family drove Claude
# Code's CronCreate/`/loop` + curl-listener arming, which ecodex replaces with
# the native ntfy listener (core/src/ntfy_listener.rs) + session-monitor-arm +
# the `empirica loop`/`listener` CLI. Excluded from BOTH re-vendor and the
# "NEW upstream" report so they don't creep back on the next sync.
RETIRED = {
    "loop-install-pickup.py",
    "loop-uninstall-pickup.py",
    "listener-install-pickup.py",
    "listener-uninstall-pickup.py",
}

# ── De-Claude scan ──────────────────────────────────────────────────
# A flagged string contains any of these → legit (identifier / path / hook
# contract / the genericized terminal fallback), never model-facing identity.
_ALLOW_SUBSTR = (
    "claude_session_id", "claude_task_id", "parent_claude_session_id",
    "subagent_claude_session_id", "stdin_claude_session_id", "_claude_sid",
    "existing_claude_id", "mcp__claude-in-chrome", "claude-in-chrome",
    "~/.claude", "/.claude/", ".claude/plugins", ".claude/projects",
    "claude_md", "claude.md", "settings.json", "ai_id='claude-code'",
    'ai_id="claude-code"', "'claude-code', none", "[claude-code, none]",
)


def _emitted_strings(tree: ast.AST):
    """Yield (lineno, value) for str literals that are NOT docstrings.

    Comments/docstrings are invisible to the model; emitted string literals
    (additionalContext, nudge text, f-string templates) are what it reads.
    """
    skip = set()  # docstrings + f-string parts (counted via their JoinedStr)
    for node in ast.walk(tree):
        if isinstance(node, (ast.Module, ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            body = getattr(node, "body", [])
            if (body and isinstance(body[0], ast.Expr)
                    and isinstance(getattr(body[0], "value", None), ast.Constant)
                    and isinstance(body[0].value.value, str)):
                skip.add(id(body[0].value))
        elif isinstance(node, ast.JoinedStr):
            for v in node.values:
                if isinstance(v, ast.Constant):
                    skip.add(id(v))
    for node in ast.walk(tree):
        if isinstance(node, ast.Constant) and isinstance(node.value, str) and id(node) not in skip:
            yield node.lineno, node.value
        elif isinstance(node, ast.JoinedStr):  # f-string: join its literal parts
            parts = [v.value for v in node.values
                     if isinstance(v, ast.Constant) and isinstance(v.value, str)]
            if parts:
                yield node.lineno, " ".join(parts)


def declaude_flags(path: Path, text: str) -> list[tuple[int, str]]:
    """Return [(lineno, snippet)] of MODEL-FACING Claude-isms worth surfacing.

    .md  → prose the model reads; any standalone 'claude' (minus allowlist).
    .py  → AST-scan string literals only (NOT comments/docstrings, which the
           model never sees); flag any 'claude' in an emitted string.
    """
    flags: list[tuple[int, str]] = []
    if path.suffix == ".md":
        for i, line in enumerate(text.splitlines(), 1):
            low = line.lower()
            if "claude" in low and not any(a in low for a in _ALLOW_SUBSTR):
                flags.append((i, line.strip()[:100]))
        return flags
    # .py — only emitted string literals (comments/docstrings excluded by AST)
    try:
        tree = ast.parse(text)
    except SyntaxError:
        return flags
    seen = set()
    for lineno, val in _emitted_strings(tree):
        low = val.lower()
        if "claude" not in low:
            continue
        snippet = " ".join(val.split())[:100]
        key = (lineno, snippet)
        if key in seen:
            continue
        # 'setup-claude-code' in an emitted hint IS a real ecodex de-Claude
        # target (should say setup-codex) — flag it explicitly.
        if "setup-claude-code" in low:
            seen.add(key)
            flags.append(key)
            continue
        # otherwise allow the genericized fallback literal + CC-compat paths +
        # identifiers; flag only genuine model-facing prose.
        if "claude-code" in low or ".claude" in low or any(a in low for a in _ALLOW_SUBSTR):
            continue
        seen.add(key)
        flags.append(key)
    return flags


# ── git source access ───────────────────────────────────────────────
def git_show(emp: Path, ref: str, relpath: str) -> bytes:
    r = subprocess.run(
        ["git", "-C", str(emp), "show", f"{ref}:{relpath}"],
        capture_output=True,
    )
    if r.returncode != 0:
        detail = r.stderr.decode("utf-8", "replace").strip()
        raise RuntimeError(f"git show failed for {ref}:{relpath}: {detail}")
    return r.stdout


def git_ls(emp: Path, ref: str, reldir: str) -> list[str]:
    r = subprocess.run(
        ["git", "-C", str(emp), "ls-tree", "--name-only", ref, f"{reldir}/"],
        capture_output=True, text=True,
    )
    if r.returncode != 0:
        detail = r.stderr.strip()
        raise RuntimeError(f"git ls-tree failed for {ref}:{reldir}: {detail}")
    return [Path(p).name for p in r.stdout.split("\n") if p.strip() and not p.endswith("/")]


# ── main pipeline ───────────────────────────────────────────────────
def main() -> int:
    ap = argparse.ArgumentParser(description="empirica → ecodex import + de-Claude pipeline")
    ap.add_argument("--apply", action="store_true", help="write drifted files (default: dry-run)")
    ap.add_argument("--deploy", action="store_true", help="copy synced files to runtime cache")
    ap.add_argument("--ref", default="develop", help="empirica git ref to import from")
    ap.add_argument("--empirica", default=str(DEFAULT_EMPIRICA), help="empirica repo path")
    ap.add_argument("--no-verify", action="store_true", help="skip py_compile + tests")
    args = ap.parse_args()

    emp = Path(args.empirica).expanduser()
    if not (emp / ".git").exists():
        print(f"✗ empirica repo not found at {emp}", file=sys.stderr)
        return 2

    missing_targets = [path for path in DIR_MAP if not path.is_dir()]
    if missing_targets:
        for path in missing_targets:
            print(f"✗ vendored target directory missing: {path}", file=sys.stderr)
        return 2

    drifted: list[Path] = []
    deployable: list[Path] = []
    new_upstream: list[str] = []
    all_flags: dict[str, list[tuple[int, str]]] = {}

    print(f"setup-codex: importing from {emp.name}@{args.ref}  ({'APPLY' if args.apply else 'dry-run'})\n")

    try:
        upstream_files = {
            eco_dir: set(git_ls(emp, args.ref, emp_reldir))
            for eco_dir, emp_reldir in DIR_MAP.items()
        }
    except RuntimeError as exc:
        print(f"✗ cannot inspect empirica source: {exc}", file=sys.stderr)
        return 2

    for eco_dir, emp_reldir in DIR_MAP.items():
        eco_files = {p.name for p in eco_dir.iterdir() if p.is_file()}
        emp_files = upstream_files[eco_dir]

        # files ecodex vendors → sync if drifted
        for name in sorted(eco_files):
            if name in RETIRED:
                continue  # deliberately retired — never re-vendor
            if name not in emp_files:
                continue  # ecodex-only (e.g. native agents) — leave untouched
            try:
                blob = git_show(emp, args.ref, f"{emp_reldir}/{name}")
            except RuntimeError as exc:
                print(f"✗ cannot read empirica source: {exc}", file=sys.stderr)
                return 2
            target = eco_dir / name
            deployable.append(target)
            cur = target.read_bytes()
            if cur != blob:
                drifted.append(target)
                if args.apply:
                    target.write_bytes(blob)
            # scan the (post-sync) content for model-facing claude-isms
            try:
                scan_text = blob.decode("utf-8", "replace")
                f = declaude_flags(target, scan_text)
                if f:
                    all_flags[str(target.relative_to(REPO))] = f
            except Exception:
                pass

        # empirica-NEW files ecodex doesn't vendor yet (retired ones suppressed)
        for name in sorted(emp_files - eco_files):
            if name.startswith(".") or name.endswith((".pyc",)) or name in RETIRED:
                continue
            new_upstream.append(f"{emp_reldir}/{name}")

    # ── skills: SCAN-ONLY (not in DIR_MAP on purpose) ──
    # Skills are a *snapshot* layer — ecodex vendors them once and de-Claudes
    # in place; they are NOT re-synced from empirica (no DIR_MAP entry), so a
    # human's de-Claude edits here are durable. But they ARE the largest
    # model-facing surface (pinned skill bodies + descriptions inject into the
    # model's context every session), and the DIR_MAP-only scan above silently
    # skipped them — giving false "clean" reports. Scan the local copies so any
    # Claude-ism (regression or newly-vendored skill) surfaces here too.
    skills_dir = PLUGIN / "skills"
    if skills_dir.exists():
        for md in sorted(skills_dir.rglob("*.md")):
            try:
                f = declaude_flags(md, md.read_text("utf-8", errors="replace"))
                if f:
                    all_flags[str(md.relative_to(REPO))] = f
            except Exception:
                pass

    # ── report ──
    rel = lambda p: str(p.relative_to(REPO))
    print(f"DRIFTED ({len(drifted)}):")
    for p in drifted:
        print(f"  {'updated' if args.apply else 'would update'}  {rel(p)}")
    if not drifted:
        print("  (in sync)")

    if new_upstream:
        print(f"\nNEW upstream files ecodex doesn't vendor ({len(new_upstream)}):")
        for n in new_upstream:
            print(f"  + {n}")

    if all_flags:
        total = sum(len(v) for v in all_flags.values())
        print(f"\nDE-CLAUDE FLAGS — model-facing Claude-isms ({total} in {len(all_flags)} files):")
        for fpath, fl in all_flags.items():
            print(f"  {fpath}")
            for ln, txt in fl[:6]:
                print(f"    :{ln}  {txt}")
            if len(fl) > 6:
                print(f"    … +{len(fl) - 6} more")
        print("  ↳ verbatim hook internals are excluded; these are prose the model reads.")
    else:
        print("\nDE-CLAUDE FLAGS: none (no model-facing Claude-isms in synced content)")

    # ── verify ──
    if args.apply and not args.no_verify:
        changed_py = [p for p in drifted if p.suffix == ".py"]
        if changed_py:
            print("\nVERIFY py_compile:")
            r = subprocess.run([sys.executable, "-m", "py_compile", *map(str, changed_py)])
            print("  OK" if r.returncode == 0 else "  ✗ FAILED")
            if r.returncode != 0:
                return 1
        print("VERIFY vendored_hooks tests:")
        r = subprocess.run(
            [sys.executable, "-m", "pytest",
             "codex-rs/codex-empirica-plugin/tests/vendored_hooks/", "-q"],
            cwd=REPO,
        )
        if r.returncode != 0:
            print("  ✗ tests failed")
            return 1

    # ── deploy ──
    if args.apply and args.deploy:
        cache = Path.home() / ".codex" / "plugins" / "cache" / "nubaeon" / "empirica"
        vers = sorted([d for d in cache.glob("*/") if d.is_dir()]) if cache.exists() else []
        if vers:
            cdir = vers[-1] / "hooks_scripts"
            print(f"\nDEPLOY → {cdir}:")
            deployed = 0
            for p in deployable:
                # map assets/hooks_scripts/<sub>/<f> → cache/hooks_scripts/<sub>/<f>
                try:
                    sub = p.relative_to(ASSETS / "hooks_scripts")
                    dest = cdir / sub
                    cache_differs = not dest.exists() or dest.read_bytes() != p.read_bytes()
                    if dest.parent.exists() and cache_differs:
                        dest.write_bytes(p.read_bytes())
                        print(f"  deployed {sub}")
                        deployed += 1
                except ValueError:
                    pass  # agents/ not under hooks_scripts → skip cache deploy
            if not deployed:
                print("  (already in sync)")
        else:
            print("\nDEPLOY: no runtime cache found, skipped")

    if args.apply:
        stamp_vendor_vintage(emp, args.ref)

    print(f"\nDone. {'Wrote' if args.apply else 'Dry-run —'} {len(drifted)} file(s)"
          f"{'.' if args.apply else '; re-run with --apply to write.'}")
    return 0


def stamp_vendor_vintage(emp: Path, ref: str) -> None:
    """Record the vendored empirica vintage in the plugin manifest.

    `empiricaVendorVersion` (+ source commit) is an extra manifest field —
    codex's RawPluginManifest ignores unknown keys, so this is diagnostics
    metadata only. The `version` field (which drives the plugin cache path)
    is deliberately NOT rolled here; that is a separate decision.
    """
    import json as _json

    manifest_path = REPO / "codex-rs" / "codex-empirica-plugin" / "manifest.json"
    try:
        pyproject = subprocess.run(
            ["git", "show", f"{ref}:pyproject.toml"],
            cwd=emp, capture_output=True, text=True, check=True,
        ).stdout
        version = next(
            line.split('"')[1] for line in pyproject.splitlines()
            if line.strip().startswith("version =")
        )
        commit = subprocess.run(
            ["git", "rev-parse", ref], cwd=emp,
            capture_output=True, text=True, check=True,
        ).stdout.strip()
    except (subprocess.CalledProcessError, StopIteration, IndexError) as exc:
        print(f"  ⚠ vintage stamp skipped: {exc}", file=sys.stderr)
        return
    manifest = _json.loads(manifest_path.read_text(encoding="utf-8"))
    stamp = {"empiricaVendorVersion": version, "empiricaVendorCommit": commit[:12]}
    if all(manifest.get(k) == v for k, v in stamp.items()):
        return
    manifest.update(stamp)
    manifest_path.write_text(_json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"  stamped manifest vintage: empirica {version} @ {commit[:12]}")


if __name__ == "__main__":
    raise SystemExit(main())
