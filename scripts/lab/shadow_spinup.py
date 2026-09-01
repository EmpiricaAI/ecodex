#!/usr/bin/env python3
"""EXP-SHADOW paired-arm spin-up for the prevention-value pilot.

Consumes a subject spec (YAML) and prepares/launches the treatment + control
arms per docs/ecodex/experiments/exp-shadow-corpus-scoping.md:

  prepare  - worktree pair (experiment/shadow-<slug>-t|c), TASK.md, per-arm
             fresh empirica practice (project-init, unique ai_id), treatment
             prior injected via `empirica finding-log --project-id`, and a
             manifest row per arm (arm registry, joinable against core's
             prevention_events without reconstruction).
  launch   - print (or exec with --exec) the `ecodex exec` command for one
             arm, with the control seat carrying EMPIRICA_PREVENTION_SHADOW=1.
  status   - manifest summary for a subject (or all).

Grading and ATE math stay out of scope (manual adversarial landing review;
research-side estimator) — this script only makes the corpus mechanical.

Subject spec (docs/ecodex/experiments/subjects/<slug>.yaml):

  slug: pipeline-exit-verdict
  pattern_key: unfalsifiable-success
  window_s: 3600
  task_md: |
    ...the brief (identical on both arms; must NOT mention the pattern)...
  prior: |
    ...treatment finding text (the catalog disambiguator)...
"""

from __future__ import annotations

import argparse
import datetime as _dt
import json
import os
import shlex
import subprocess
import sys
from pathlib import Path

try:
    import yaml
except ImportError:  # pragma: no cover - environment guard, not logic
    print("shadow_spinup: PyYAML required (pip install pyyaml)", file=sys.stderr)
    raise SystemExit(2)

REPO_ROOT = Path(__file__).resolve().parents[2]
SUBJECTS_DIR = REPO_ROOT / "docs" / "ecodex" / "experiments" / "subjects"
MANIFEST = REPO_ROOT / "docs" / "ecodex" / "experiments" / "shadow-manifest.jsonl"
ARMS = ("t", "c")  # treatment, control


def _run(cmd: list[str], *, cwd: Path | None = None, env: dict | None = None) -> str:
    """Run a command; raise with captured output on failure (no silent skips)."""
    result = subprocess.run(
        cmd, cwd=cwd, env=env, capture_output=True, text=True, check=False
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"command failed ({result.returncode}): {shlex.join(cmd)}\n"
            f"stdout: {result.stdout.strip()}\nstderr: {result.stderr.strip()}"
        )
    return result.stdout


def load_subject(slug: str) -> dict:
    spec_path = SUBJECTS_DIR / f"{slug}.yaml"
    spec = yaml.safe_load(spec_path.read_text(encoding="utf-8"))
    missing = [k for k in ("slug", "pattern_key", "window_s", "task_md", "prior") if k not in spec]
    if missing:
        raise RuntimeError(f"{spec_path}: missing keys {missing}")
    if spec["slug"] != slug:
        raise RuntimeError(f"{spec_path}: slug field {spec['slug']!r} != filename slug {slug!r}")
    # The brief must not leak the pattern to the control arm.
    lowered = spec["task_md"].lower()
    for token in (spec["pattern_key"].lower(), "anti-pattern", "broccoli"):
        if token in lowered:
            raise RuntimeError(
                f"{spec_path}: task_md leaks the pattern to both arms (found {token!r})"
            )
    return spec


def arm_ai_id(slug: str, arm: str) -> str:
    return f"shadow-{slug}-{arm}"


def worktree_path(slug: str, arm: str) -> Path:
    return REPO_ROOT.parent / f"ecodex-{arm_ai_id(slug, arm)}"


def prepare(slug: str, *, base: str = "main") -> None:
    spec = load_subject(slug)
    for arm in ARMS:
        wt = worktree_path(slug, arm)
        branch = f"experiment/{arm_ai_id(slug, arm)}"
        if wt.exists():
            raise RuntimeError(f"{wt} already exists — refuse to clobber a prepared arm")
        _run(["git", "worktree", "add", str(wt), "-b", branch, base], cwd=REPO_ROOT)
        (wt / "TASK.md").write_text(spec["task_md"], encoding="utf-8")
        _run(["git", "add", "TASK.md"], cwd=wt)
        _run(["git", "commit", "-q", "-m", f"lab: TASK.md for {arm_ai_id(slug, arm)}"], cwd=wt)

        # Fresh practice per arm (Q2: fresh-practice randomization unit).
        env = dict(os.environ, EMPIRICA_AI_ID=arm_ai_id(slug, arm))
        _run(["empirica", "project-init"], cwd=wt, env=env)

        if arm == "t":
            # Treatment prior lands BEFORE the actor's first PREFLIGHT, as an
            # ordinary finding in the fresh practice — the same surface a real
            # practice's recall would present.
            _run(
                [
                    "empirica", "finding-log",
                    "--project-id", arm_ai_id(slug, arm),
                    "--finding", spec["prior"].strip(),
                    "--impact", "0.7",
                ],
                cwd=wt,
            )

        row = {
            "subject": slug,
            "pattern_key": spec["pattern_key"],
            "arm": "treatment" if arm == "t" else "control",
            "ai_id": arm_ai_id(slug, arm),
            "worktree": str(wt),
            "branch": branch,
            "window_s": spec["window_s"],
            "prepared_at": _dt.datetime.now(_dt.UTC).isoformat(),
            "launched_at": None,
        }
        with MANIFEST.open("a", encoding="utf-8") as fh:
            fh.write(json.dumps(row) + "\n")
        print(f"prepared {row['arm']:9s} arm: {wt}")


def launch_command(slug: str, arm: str, *, model: str | None) -> tuple[list[str], dict]:
    spec = load_subject(slug)
    wt = worktree_path(slug, arm)
    if not wt.exists():
        raise RuntimeError(f"{wt} not prepared — run `prepare {slug}` first")
    env = dict(os.environ, EMPIRICA_AI_ID=arm_ai_id(slug, arm))
    if arm == "c":
        # Control seat: same emission code path, shadow-tagged rows.
        env["EMPIRICA_PREVENTION_SHADOW"] = "1"
    cmd = [
        "ecodex", "exec",
        "-C", str(wt),
        "-s", "workspace-write",
        "-c", "sandbox_workspace_write.writable_git=true",
    ]
    if model:
        cmd += ["-m", model]
    cmd.append(
        "Read TASK.md in this worktree and complete the task. Work in this "
        "worktree only and follow your normal engineering discipline."
    )
    _ = spec  # spec validated above; window_s is pre-registered in the manifest
    return cmd, env


def launch(slug: str, arm: str, *, model: str | None, execute: bool) -> None:
    cmd, env = launch_command(slug, arm, model=model)
    extra = {k: v for k, v in env.items() if k.startswith("EMPIRICA_")}
    printable = " ".join(f"{k}={v}" for k, v in sorted(extra.items()))
    print(f"{printable} {shlex.join(cmd)}")
    if execute:
        _stamp_launch(slug, arm)
        raise SystemExit(subprocess.run(cmd, env=env, check=False).returncode)


def _stamp_launch(slug: str, arm: str) -> None:
    arm_name = "treatment" if arm == "t" else "control"
    rows = [json.loads(line) for line in MANIFEST.read_text(encoding="utf-8").splitlines() if line]
    for row in rows:
        if row["subject"] == slug and row["arm"] == arm_name and row["launched_at"] is None:
            row["launched_at"] = _dt.datetime.now(_dt.UTC).isoformat()
    MANIFEST.write_text("".join(json.dumps(r) + "\n" for r in rows), encoding="utf-8")


def status(slug: str | None) -> None:
    if not MANIFEST.exists():
        print("no manifest yet — nothing prepared")
        return
    rows = [json.loads(line) for line in MANIFEST.read_text(encoding="utf-8").splitlines() if line]
    for row in rows:
        if slug and row["subject"] != slug:
            continue
        launched = row["launched_at"] or "not launched"
        print(f"{row['subject']:28s} {row['arm']:9s} {row['ai_id']:32s} {launched}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)
    p_prep = sub.add_parser("prepare", help="create both arms for a subject")
    p_prep.add_argument("slug")
    p_prep.add_argument("--base", default="main")
    p_launch = sub.add_parser("launch", help="print/exec the launch command for one arm")
    p_launch.add_argument("slug")
    p_launch.add_argument("arm", choices=ARMS)
    p_launch.add_argument("--model", default=None)
    p_launch.add_argument("--exec", action="store_true", dest="execute")
    p_status = sub.add_parser("status", help="manifest summary")
    p_status.add_argument("slug", nargs="?")
    args = parser.parse_args()

    if args.cmd == "prepare":
        prepare(args.slug, base=args.base)
    elif args.cmd == "launch":
        launch(args.slug, args.arm, model=args.model, execute=args.execute)
    elif args.cmd == "status":
        status(args.slug)


if __name__ == "__main__":
    main()
