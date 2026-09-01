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

1. `git worktree add` pair: `experiment/shadow-<slug>-t` (treatment) and
   `experiment/shadow-<slug>-c` (control), identical `TASK.md`.
2. **Fresh practice per arm** (Q2): `empirica project-init` inside each
   worktree with a unique `ai_id` (`shadow-<slug>-t|c`) — no shared artifact
   history, which is the contamination the design doc demonstrated is real.
3. **Treatment prior injection:** `empirica finding-log --project-id
   shadow-<slug>-t` with the pattern's catalog disambiguator BEFORE the
   actor's first PREFLIGHT — it arrives as ordinary recalled context, the
   same surface a real practice would have.
4. **Control seat:** launch env carries `EMPIRICA_PREVENTION_SHADOW=1`; no
   prior logged. Everything else identical.
5. Actor: same model on both arms (per-subject constant; model choice is a
   corpus-level decision, pilot default = the current lab configured model),
   launched via the lab's `ecodex exec` harness under full transaction
   discipline (Sentinel vendored + firing — verified live previously).
6. `window_s` (Q1, pre-registered per corpus BEFORE emission): pilot uses
   **wall-clock session cap = 3600s per subject arm**, so
   `window_s = 3600` on every row. Rationale: ecodex lab tasks complete in
   10–60 min; a window equal to the session cap makes "prevented within the
   window" mean "prevented within the subject's whole run", which is the
   only observable scope a worktree-per-subject corpus has.

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

Responsibilities: worktree-pair creation, `TASK.md` write, per-arm
`project-init` (+ `EMPIRICA_AI_ID`), treatment-prior injection, control-seat
env, actor launch, stall monitoring (reuse `lab_stall_monitor.py`), and an
end-of-run manifest row (subject, arm, session_id, commit, wall time) so the
corpus is joinable against core's `prevention_events` without reconstruction.

Explicitly NOT automated in the pilot: task grading (landing review stays
manual + adversarial, as in lab rounds) and ATE math (research-side per the
design doc's non-claims).

## 4. Pilot execution order

Randomize subject order once (seeded), interleave t/c launches, ≤2 subjects
per session to keep landing review honest. 15 subjects ≈ 8 sessions at lab
throughput. After the pilot: per-pattern base incidence + variance go back
through the SER for main-run n (Q3).
