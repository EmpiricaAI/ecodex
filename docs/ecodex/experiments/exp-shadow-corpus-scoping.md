# EXP-SHADOW pilot corpus — subject selection + spin-up automation

**Status:** SCOPING for pilot execution. Follows
[`prevention-value-exp-shadow-design.md`](prevention-value-exp-shadow-design.md)
and its SER convergence (Q1 `window_s` per-corpus pre-registered, Q2 fresh
practice per subject, Q3 pilot of 12–15 paired subjects). Both blockers are
cleared: core's `prevention_events` emission wiring is live with proof rows,
and `shadow=true` control-arm emission rides `EMPIRICA_PREVENTION_SHADOW=1`
on control seats through the same code path — no ecodex-side hook.

**Scopes:** goal `5221a493-242c-49d2-84ba-597928087598` (pilot).

---

## 1. Subject candidates (15) — all drawn from logged ecodex incidents

Selection rule: each subject is a **real incident from this practice's
artifact history**, re-cast as a self-contained `TASK.md` where the cataloged
anti-pattern is a *live risk on the natural solution path* — not mentioned in
the brief. The treatment arm's prior is the pattern's broccoli-catalog
disambiguator (`broken-if` / `by-design-if`), surfaced as a pre-logged
finding in the subject's fresh practice. The control arm gets the identical
brief cold.

| # | subject slug | pattern_key (broccoli family) | Task shape (what TASK.md asks for) | Incident provenance |
|---|---|---|---|---|
| 1 | `pipeline-exit-verdict` | unfalsifiable-success (§B) | Write a CI gate script that runs a build and reports pass/fail | `EXIT=$?` after a grep pipeline reported the *grep's* exit as the build verdict — bit this practice twice, 4 in-house instances confirmed in the verdict-integrity audit |
| 2 | `absence-needs-positive-control` | negative-assertion-without-positive-control (§B) | Verify a symbol/feature is absent from a foreign ref/repo and report | cwd-relative git-pathspec greps returned empty ⇒ "no consumers" twice before a positive control caught the dead instrument |
| 3 | `hermetic-env-dead-instrument` | exemption-reports-clean-forever (§B/§D) | Add a hermetic test job that installs a tool into a venv and asserts on its CLI | CI's venv bin was never on PATH; the conformance test silently skipped for its whole life until un-silenced |
| 4 | `release-tag-coupling` | two-sources-of-truth-drift (§C) | Add a pinned external-artifact fetch to a release workflow | `UPSTREAM_SYNC_TAG` stayed at the previous base across a version bump — nothing couples it to the workspace version |
| 5 | `clean-merge-not-proof` | silent-truncation / indistinguishable-incompleteness (§D) | Merge a long-lived branch where rename-detection silently drops a file | 0.149.0 merge silently deleted an ecodex-owned file with no conflict marker; only full-workspace build surfaced it |
| 6 | `worktree-git-pointer` | authority-on-the-wrong-field (§E) | Grant sandboxed write access to a repo's git state | `writable_git` covered `<root>/.git` — a one-line gitdir *pointer* in linked worktrees; every commit failed on the real gitdir |
| 7 | `fallback-masks-sessionend` | fallback-masks-primary (§B) | Add a shutdown-path warning/cleanup step with a degraded fallback | SessionEnd-clamp incident (named in the design doc §3.1) |
| 8 | `cwd-deleted-freeze` | unrecoverable-gate (§E) | Make a long-running server resilient to its launch directory disappearing | cwd-deletion session freeze: `current_dir()` ENOENT on every config refresh, session never recovers |
| 9 | `partial-release-assets` | partial-success-as-success (§B) | Add a new required binary to an existing multi-artifact release pipeline | v0.147.0 shipped without `codex-code-mode-host`; every surface (workflow, installer, formula) missed it and the release read green |
| 10 | `lockfile-wholesale-regress` | one-predicate-two-questions / trust-the-input (§C) | Resolve a lockfile merge conflict during a dependency sync | taking the incoming lock wholesale silently reverted a pinned security floor — twice, across two separate syncs |
| 11 | `display-vs-bin-name` | semantic-drift (§C) | Rebrand a forked CLI's user-facing identity | `bin_name` fixed help/usage; `--version` kept printing the crate name for three releases |
| 12 | `update-channel-authority` | authority-on-the-wrong-field (§E) | Point a fork's update-check at its own release channel | issue #16: doctor + TUI update actions still targeted upstream's releases and recommended the wrong package |
| 13 | `tag-prefix-parse` | encoding-quoting-mangle (§C) | Parse latest-version from GitHub release tags across repos | `extract_version_from_latest_tag` stripped a `rust-v` prefix that only exists on one repo's tags |
| 14 | `pipefail-curl-grep` | boundary/encoding + unfalsifiable-success (§C/§B) | Write an installer step that resolves "latest version" from a remote index under `set -o pipefail` | install.sh's `curl \| grep -m1 \| sed` broke when grep closed the pipe early — curl's write error failed the whole resolution |
| 15 | `deploy-cache-staleness` | deploy-staleness (§E) | Ship hook-script updates into a runtime cache consumed by a live tool | vendored-hooks cache path is version-pinned while contents re-vendor freely; box-runs-old-code is the practice's #1 recurring root cause |

Coverage check: §B ×5, §C ×4, §D ×1, §E ×4, mixed ×1 — all four hunt
families represented; no pattern_key appears more than twice.

## 2. Arm mechanics per subject

> **CORRECTED 2026-09-05 (David).** The pilot's first attempt spun each arm
> up as its own top-level registered practice (`ecodex-shadow-*`) launched by
> detached `ecodex exec` from the ecodex main session. That was wrong twice
> over: it polluted the workspace registry with per-arm practices, and running
> `empirica` from inside an arm worktree **switched the orchestrating harness's
> own practice binding** away from `ecodex` — the root cause of the whole
> cross-project-write mess. The correct model: **ecodex orchestrates and never
> switches practice; the arms run within the `ecodex-lab` practice** (the
> existing live lab harness), guided over the mesh. This supersedes the
> design doc's Q2 (`fresh practice per subject`), which was converged with core
> — see the Q2-revision note routed to core.

1. `git worktree add` pair under the `ecodex-lab` practice:
   `experiment/shadow-<slug>-t` (treatment) and `-c` (control), identical
   `TASK.md`. Both worktrees resolve to the **`ecodex-lab`** practice — no new
   per-arm practice is minted.
2. **Isolation via worktree + fresh session, not fresh practice** (revised Q2):
   each arm is a distinct `ecodex-lab` session in its own worktree; the
   per-session context is the isolation boundary. **Contamination tension,
   stated not hidden:** running treatment then control as sequential sessions
   of one `ecodex-lab` practice means they share the lab's accumulated
   trajectory — the exact crossover risk the original Q2 avoided. David's
   structural call accepts this in exchange for not polluting the registry and
   not switching the orchestrator; the mitigation (subject ordering, or
   resetting lab trajectory between arms) is an open corpus-design detail for
   the rework, and the change is on record with core.
3. **Orchestration is ecodex over the mesh, never in-session cd.** ecodex (this
   practice) prepares worktrees + `TASK.md` and guides via `cortex_collab`/
   proposal to the live `ecodex-lab` session (ntfy doorbell wake). The
   orchestrator **must not** run `empirica` from inside an arm worktree — that
   is what repoints the harness binding.
4. **Treatment prior injection:** logged to the `ecodex-lab` practice's own
   store from the orchestrator via `finding-log --project-id ecodex-lab` (now
   that cross-project routing is fixed at ae5191bc0, both lanes land in the
   target), verified present in both the sessions.db row and the eidetic
   collection before the arm session runs.
5. **Control seat:** the `ecodex-lab` arm session launches with
   `EMPIRICA_PREVENTION_SHADOW=1`; no prior present. Everything else identical.
6. Actor: same model on both arms (per-subject constant; pilot default =
   the lab configured model via the working `-c model_provider=openai -m
   gpt-5.6-sol` route), under full transaction discipline.
7. `window_s` (Q1, pre-registered per corpus BEFORE emission): pilot uses
   **wall-clock session cap = 3600s per subject arm**, so `window_s = 3600` on
   every row — a window equal to the session cap means "prevented within the
   subject's whole run", the only observable scope this corpus has.

## 3. Spin-up automation (the actual build item)

One script, `scripts/lab/shadow_spinup.py` (new; sibling of
`lab_stall_monitor.py`), consuming a subject spec:

```yaml
# docs/ecodex/experiments/subjects/<slug>.yaml
slug: pipeline-exit-verdict
pattern_key: unfalsifiable-success
task_md: |
  <the brief>
prior: |
  <the treatment finding text — the catalog disambiguator>
window_s: 3600
```

> **NEEDS REWORK (2026-09-05).** The committed `shadow_spinup.py` implements
> the superseded per-arm-practice model (it calls `project-init` per worktree
> and launches detached `ecodex exec` with `EMPIRICA_AI_ID=ecodex-shadow-*`).
> Under the corrected within-`ecodex-lab` model it must NOT mint practices or
> switch bindings. The rework is gated on studying the lab's live-session /
> ntfy-doorbell orchestration mechanism (how the existing lab is woken and
> guided over the mesh) and is tracked as its own task, not done in the
> teardown pass.

Responsibilities (corrected model): worktree-pair creation under `ecodex-lab`,
`TASK.md` write, treatment-prior injection into the `ecodex-lab` store via
`--project-id ecodex-lab` with both-lane read-back verify, control-seat env
(`EMPIRICA_PREVENTION_SHADOW=1`), guiding the live `ecodex-lab` session over
the mesh to run the arm (no `project-init`, no detached exec that switches the
orchestrator binding), stall monitoring (reuse `lab_stall_monitor.py`), a
post-run actor-resolution guard (assert the actor's own artifacts keyed to the
`ecodex-lab` project, not an md5 collection — see the md5-fallback finding),
and an end-of-run manifest row joinable against core's `prevention_events`.

Explicitly NOT automated in the pilot: task grading (landing review stays
manual + adversarial, as in lab rounds) and ATE math (research-side per the
design doc's non-claims).

## 4. Pilot execution order

Randomize subject order once (seeded), interleave t/c launches, ≤2 subjects
per session to keep landing review honest. 15 subjects ≈ 8 sessions at lab
throughput. After the pilot: per-pattern base incidence + variance go back
through the SER for main-run n (Q3).
