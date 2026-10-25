---
name: pre-publish
description: "Use when about to push, tag, release or publish code other people will run — 'ready to release', 'cut a release', 'before I push', 'pre-publish', 'is this safe to ship'. The ordered gate sequence: scope the diff, deterministic gates, an independent read-only sweep with every finding traced, docs and code gaps, hot-path timing, a clean re-run on the exact commit, then the cut. Carries the traps that green CI hides."
---

# Pre-publish

Read this **before** you push a release or publish, not after something ships wrong.

Deterministic gates catch what someone thought to check. An independent sweep
catches what nobody did. On one empirica release, CI was green on the exact commit
and the compliance report passed 15 of 16 checks; three read-only reviewers then
flagged 23 candidates and 9 held up under tracing, including a new destructive
command whose reference check was missing a whole store. No test could have failed
on that, because the defect was an absence.

## The project's release reference

Steps 2 and 8 need the project's own procedure: where the version lives, which
gates it runs, how it cuts, what to wait on, and how to verify what shipped.
`references/ecodex-release.md` is ecodex's. For any other project, use its release
doc or script. If it has none, write those five answers down before step 2 rather
than improvising them at the cut.

## The sequence (the order matters)

1. **Scope.** `git diff <last-tag>..HEAD --stat`. Group the changed files into 3 to
   5 areas. Note whether any hook, or the PREFLIGHT/CHECK/POSTFLIGHT path, changed:
   that decides step 5.
2. **Deterministic tier: a tool's job, not yours.** Lint, types, full tests,
   dependency CVE audit, secret scan, docs drift. Fix failures; do not waive them. A
   CVE with a published fix is a floor bump, not a note.
3. **Independent sweep.** One read-only reviewer per area, in parallel: one
   `spawn_agent` subagent per area, each with the brief in
   `references/sweep-brief.md` (`dispatch-agent` covers enriching the message). They
   do not edit, apply, or run anything that mutates. The pattern table they apply is
   Phase 2 of the `eat-the-broccoli` skill. If subagents are unavailable, take the
   areas yourself one at a time with the same brief, and say in the report that the
   sweep was not independent.
4. **Trace every finding yourself.** A reviewer's verdict is an uncalibrated
   self-report. For each finding: open the file:line, reproduce or trace it end to
   end, ask whether it is by design, then either fix it failing-test-first or log it
   as a goal or unknown. Keep three numbers and report them: candidates, confirmed,
   fixed. Say plainly which confirmed ones are still open.
5. **Hot path.** If hooks or PREFLIGHT/CHECK/POSTFLIGHT changed, time the real path
   on the last tag's copy against the new one, same inputs, 15 runs each. A per-call
   cost regression blocks the cut; a removal that was already short-circuited is
   neutral, not a saving.
6. **Gaps.** Code against docs: new verbs and flags documented, removed names still
   referenced anywhere (docs, prompts, skills, scripts), the changelog's top entry is
   this release.
7. **Re-run on the exact commit you will release.** Green CI on a neighbouring
   commit is not green on this one.
8. **Cut and verify** (the release reference). Wait for the release run by its id;
   verify the artifacts, not the job status.

## Rules earned, each from an incident

- **A second pass that is not clean means the first was wrong.** After any bulk
  repair or apply, re-run the plan. 342 of 1,727 items still disagreeing exposed a
  bug in the repair itself; the unit tests had never exercised the condition.
- **Grep for an existing guard before writing a new one.** The check the sweep
  wanted already existed one class over, with a comment explaining why it mattered.
- **A new destructive command is swept before it touches real data.** Enumerate
  what references the thing it deletes from the schema (every table with that id as
  a column, every external store), and treat an unreachable store as unknown, never
  zero.
- **A test that mocks the boundary pins the defect.** Check payloads against the
  callee's own published schema.
- **Wait by run id, never by short SHA.** A short SHA matches nothing and the wait
  never ends.
- **Do not publish on a go you inferred.** Tag and push only on the owner's explicit
  word for this release.

## Report

Done / Next / Waiting, with the counts from step 4 and the commit the release was
cut from. Anything you left open is named, not implied.
