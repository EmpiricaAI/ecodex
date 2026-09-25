---
name: code-audit
description: "Use when the user says '/code-audit', 'audit this code', 'check code quality', 'find duplication', 'find dead code', 'code cleanup', 'technical debt audit', 'code review module', or wants a structured noetic investigation of code quality. Runs external analysis tools and a structured manual review, producing Empirica artifacts (findings, unknowns, decisions, goals) that any praxic agent can execute."
version: 1.0.0
---

# Code Audit: Noetic Investigation

**Investigate code quality. Produce structured remediation plans.**

This skill is purely noetic — it discovers, triages and plans. It does NOT make
changes. The output (findings, unknowns, decisions, goals) feeds straight into the
Empirica workflow for any praxic agent to pick up.

Open a transaction with `work_type: "audit"` before you start, and assess your own
vectors — don't paste a canned set.

---

## How to run

```
code-audit                              # the whole project
code-audit --target src/handlers/       # one directory
code-audit --target src/auth.rs         # one file
code-audit --focus duplication          # one dimension
```

State the scope you chose and what you skipped. Silent truncation reads as "covered
everything".

---

## Phase 1: Scope

If the user named a target, use it; otherwise audit the project root. In a fork or a
monorepo where most of the tree is upstream or vendored code, default to the code
this project owns — otherwise the audit drowns in someone else's code.

```bash
TARGET="${1:-.}"
rg --files "$TARGET" | rg '\.(py|rs|ts|go)$' | wc -l      # files by language of interest
tokei "$TARGET" 2>/dev/null || scc "$TARGET" 2>/dev/null   # LOC per language, if installed
```

```bash
empirica finding-log --finding "Audit scope: $TARGET — N files, N LOC, languages X/Y" --impact 0.1
```

---

## Phase 2: Automated tool passes

Run what the stack has and log aggregates as findings — not one finding per
violation. Skip any tool that isn't installed; the audit still works without it, but
say which ones you skipped.

| Dimension | Python | Rust |
|---|---|---|
| Lint / style | `ruff check "$TARGET" --statistics` | `cargo clippy -p <crate> --all-targets -- -D warnings` |
| Types / build | `pyright "$TARGET" --outputjson` | `cargo check -p <crate> --all-targets` |
| Dead code | `vulture "$TARGET" --min-confidence 80` | rustc's `dead_code` warnings; `cargo machete` for unused dependencies |
| Complexity | `radon cc "$TARGET" -s -a --min C`; `radon mi "$TARGET" -s --min B` | `scc --by-file` complexity column; clippy's `cognitive_complexity` |
| Dependency advisories | `pip-audit` | `cargo audit`, `cargo deny check` |

**Impact scoring for tool output:** cosmetic lint 0.2 · complexity violations 0.4–0.6
(radon C 0.4, D 0.6, F 0.8) · confirmed dead code 0.4 · security findings 0.8.

```bash
empirica finding-log --finding "clippy: 14 warnings in codex-foo, 9 of them needless_clone" --impact 0.3
empirica finding-log --finding "vulture: 12 unused functions (80%+ confidence)" --impact 0.4
empirica unknown-log --unknown "vulture flagged handle_legacy_sync() — verify it isn't called dynamically"
```

Dead-code tools produce false positives (entry points, dynamic dispatch, CLI
handlers, `#[cfg(test)]` helpers). Confirmed cases are findings; uncertain ones are
unknowns.

### Test freshness — could this test have failed?

Coverage asks "was this line executed?"; freshness asks **"could this test have
failed?"** A drifted test doesn't go red — it goes green over the wrong contract and
*guards* the defect. Look for:

| Pattern | What it means |
|---|---|
| stale fixture | a hand-built schema or struct literal missing fields the real one has — fixture and code agree with each other and disagree with production |
| unfalsifiable test | no assertion; passes unless something panics |
| tautological assert | `assert x == x`, `assert!(true)` — cannot fail by construction |
| snapshot pinning a defect | a snapshot (e.g. `insta`) or golden file that encodes the buggy output |

Triage, don't bulk-fix. A minimal fixture is a legitimate choice; it becomes a
finding when the missing fields arrived in a migration *after* it was written.
Record deliberate omissions as decisions so the next sweep stays quiet on them.

---

## Phase 3: Structural review (judgment)

These need your judgment — tools can't catch them.

### 3a. File size

```bash
rg --files "$TARGET" | rg '\.(py|rs)$' | xargs wc -l | sort -rn | head -20
```

Over ~2000 lines → finding at 0.5 ("needs splitting"); over ~1000 → 0.3 ("consider
splitting"); over ~500 → note only. Respect the project's own guidance where it has
one (an `AGENTS.md` module-size rule, for example).

### 3b. Duplication

```bash
rg -n "^\s*(pub(\(crate\))? )?fn \w+|^def \w+" "$TARGET" -o \
  | sed -E 's/.*(fn|def) //' | sort | uniq -c | sort -rn | head -20
```

For each duplicate, **read both copies**. Identical, similar, or intentionally
different? Log accordingly — and when it is intentional, record why:

```bash
empirica finding-log --finding "resolve_project_root() duplicated identically in 6 files" --impact 0.6
empirica decision-log --choice "Keep separate find_project_root() in hooks" \
  --rationale "Hooks run standalone and can't import the package" --reversibility exploratory
```

### 3c. Module boundaries

Read the dependency graph. Look for circular dependencies, layer violations (a CLI
module reaching into storage internals), god modules everything imports, and crates
or packages that grow because they are the easiest place to put things.

### 3d. Naming and convention consistency

Mixed naming styles, inconsistent handler patterns, magic strings and numbers
without constants.

### 3e. Error handling — success-shaped nothing

The count of swallowed errors is the least of it. The defect that survives lint,
types, coverage and a green suite is a **failure that returns the value meaning
"measured" or "done"**: `0` for a count that errored, `[]` on a timeout,
`passed = true` on a skipped check, a stub rendering constants as a grade, a command
that prints ✅ then ❌ and exits 0.

```bash
# Python: handlers that turn a failure into an honest-looking empty value
rg -n "except .*:\s*$" -A 2 "$TARGET" | rg "return (0|\[\]|\{\}|None|False)|pass$"
# Rust: discarded or defaulted results worth reading one by one
rg -n "let _ = |\.ok\(\);|\.unwrap_or_default\(\)|\.unwrap_or\((0|false|Vec::new\(\))\)" "$TARGET"
# Either: stubs and placeholders
rg -n -i "stub|placeholder|not (yet |fully )?implemented|todo!\(|unimplemented!\(" "$TARGET"
```

For each hit, ask what the CALLER does with the value. A failure returning the same
value as an honest empty, with no separate signal, is the finding. Many hits are
deliberate degradation that logs or reports; flag the silent ones.

Then **run each command or entry point live** and read the exit code against what it
printed. A parser, a doc page and a passing test do not prove a command works.

```bash
empirica finding-log --finding "<command> exits 0 with an error line: <path>:<line> returns [] on timeout, caller renders 'no results'" --impact 0.6
```

**When a defect survived a green suite, the test is a suspect.** A test that asserts
the defective surface is green exactly where it is blind — rewrite it to the contract
and let it go red first.

### 3f. Matchers — formal grammar or natural language?

Any check keyed on **words or phrases** rather than on a grammar: word lists,
negation sets, risk-word scans, "does this text say X" predicates.

The discriminator is **formal vs natural**, not "keyword matching is bad":

| | ✅ formal | ❌ natural |
|---|---|---|
| grammar | fixed and published (SQL, an AST, an enum, a config schema) | emergent, contested |
| a token's meaning | fixed | context-dependent, drifts |
| a miss is | a parser bug — findable, fixable | invisible, and reads as a result |

The tell: *can I enumerate the tokens and be done?* Over natural language you never
can, so the list produces false negatives by construction — zero hits reads as
"nothing to find" when it means "cannot see".

```bash
rg -n "^\s*(pub )?(const|static|_?[A-Z_]+)\s*.*(WORDS|MARKERS|TERMS|PATTERNS)" "$TARGET"
```

**Before condemning one, measure it** over the real corpus it sees, with a positive
control proving the instrument is live. A measured zero beats an argument from
principle. The fix is rarely a longer list: prefer a signal the system already
produces from **behaviour** — a verdict, a test result, an observed outcome.

---

## Phase 4: Triage

| Tag | Meaning |
|-----|---------|
| `duplication` | Same code in multiple places |
| `complexity` | Function or file too complex or too large |
| `dead-code` | Unused code that should be removed |
| `consistency` | Naming, patterns, conventions don't match |
| `architecture` | Module boundaries, layering, coupling |
| `reliability` | Error handling, edge cases, silent failures |
| `security` | Input validation, injection risks |

| Score | Meaning |
|-------|---------|
| 0.1–0.3 | Cosmetic — style, minor inconsistency |
| 0.4–0.6 | Structural — duplication, complexity, dead code |
| 0.7–0.9 | Critical — bug source, security risk, architecture violation |

Connect findings as you triage: a finding that explains another is `evidence` for
it; a finding caused by an earlier decision is `caused_by` that decision.

---

## Phase 5: Remediation goals

Group related findings into goals, each independently executable by a praxic agent:

```bash
empirica goals-create --objective "Split project_commands.rs into focused modules" \
  --description "## Why
Findings <ids>: 4200 LOC, 17 unrelated handlers.
## Success
Each module under 800 LOC; all tests pass; no public API change."
```

A good goal has a clear scope, success criteria, the finding ids that justify it, and
its dependencies. One goal is one praxic transaction; the `epistemic-transaction`
skill owns the sizing guidance.

---

## Phase 6: Record decisions

Not every finding needs a fix. Record the conscious trade-offs:

```bash
empirica decision-log --choice "Accept hook utility duplication" \
  --rationale "Hooks must run standalone — a shared module is the fix, not zero copies" \
  --reversibility exploratory
```

A documented "we chose to keep this" is better than unexplained duplication, and it
keeps the next audit from re-raising it.

---

## Phase 7: Summarise

Close the transaction, then tell the user, briefly:

1. **Scope** — what was audited, and what was skipped
2. **Tool results** — aggregate numbers per tool
3. **Top findings** — by impact
4. **Goals created** — the remediation plan, with ids
5. **Decisions recorded** — what was accepted or deferred
6. **Unknowns remaining** — what needs investigation before acting

---

## Output contract

| Artifact | Purpose |
|---|---|
| **Findings** (impact-scored) | What was discovered — issues, patterns, metrics |
| **Unknowns** | What needs investigation before acting |
| **Decisions** | What was consciously accepted, deferred or prioritised |
| **Goals** | Remediation work packages, ready for any praxic agent |

## Re-running

The audit is idempotent. Running it again after remediation shows which findings are
resolved (goals completed), which persist, and what is new — regression detection
over time.
