---
name: code-docs-align
description: "Use when the user says '/code-docs-align', 'check if docs match code', 'verify docstrings', 'find stale comments', 'audit TODOs', 'check ref-doc accuracy', 'documentation accuracy', or wants to verify that documentation, doc comments, inline comments and reference docs actually reflect the current code. Checks ACCURACY — do the docs match what the code does?"
version: 1.0.0
---

# Code-Docs Alignment: Documentation Accuracy Investigation

**Verify that documentation matches code. Find stale, misleading or phantom docs.**

This skill is purely noetic — it discovers mismatches between documentation and code.
It does NOT fix anything. The output (findings, unknowns, goals) feeds into the
Empirica workflow for praxic remediation.

**Why this matters:** for AI-assisted work and for anyone evaluating a codebase,
stale documentation is worse than missing documentation — it actively misleads.
`code-audit` checks code quality; doc-coverage tools check whether docs exist. This
skill checks the gap between them: **do the docs match the code?**

---

## How to run

```
code-docs-align                              # the whole project
code-docs-align --target src/handlers/       # one directory
code-docs-align --focus docstrings           # one dimension
code-docs-align --focus todos                # TODO/FIXME audit only
code-docs-align --focus ref-docs             # reference docs only
```

---

## Phase 0: PREFLIGHT

Open a transaction with `work_type: "docs"` before investigating, and assess your
own vectors. A pasted vector set is not a low reading, it is a fabricated one. If you
have already read the docs you're about to audit, your `know` reflects that — and
you can declare those reads as `read` claims in PREFLIGHT and proceed without a
separate CHECK.

---

## Phase 1: Scope

Use the target the user named; otherwise the project root. Prioritise what changed
recently — that is where docs go stale.

```bash
TARGET="${1:-.}"
rg --files "$TARGET" | rg '\.(py|rs|md)$' | wc -l
git log --name-only --format="" HEAD~20..HEAD | sort -u | rg '\.(py|rs|md)$'
empirica docs-assess --output json 2>/dev/null   # registered reference docs, if any
```

```bash
empirica finding-log --finding "Docs-align scope: $TARGET — N files, N recently changed, N ref-docs" --impact 0.1
```

---

## Phase 2: Doc-comment accuracy

For each high-priority file (recently changed or large), read the code and its
documentation together — Python docstrings, Rust `///` and `//!` comments — and
compare. This needs judgment, not pattern matching.

1. **Parameters** — documented but not in the signature (phantom, 0.6); in the
   signature but undocumented (missing, 0.3); documented type disagrees with the
   annotation (0.4).
2. **Returns** — wrong return description, or a documented return on a function
   that returns nothing (0.6).
3. **Errors** — documented errors or panics that are never raised (0.5); raised but
   undocumented (0.3).
4. **Behavioural claims** — "validates X" with no validation code, "returns None if
   not found" when it actually raises or panics (0.7).

| Mismatch | Impact | Why |
|---|---|---|
| Phantom parameter | 0.6 | Actively misleading |
| Missing parameter | 0.3 | Incomplete, not misleading |
| Wrong return description | 0.6 | Actively misleading |
| Stale errors/raises clause | 0.5 | Moderately misleading |
| Behavioural mismatch | 0.7 | Dangerously misleading |

```bash
empirica finding-log --finding "Phantom param 'timeout' documented on connect() at db/client.rs:45 — removed in the batch-3 cleanup" --impact 0.6
empirica unknown-log --unknown "process_batch() docs mention 'retry_count' — intentional passthrough or stale?"
```

---

## Phase 3: Inline comment staleness

Comments that reference patterns, functions or behaviour that no longer exist:

- **References to removed code** — names of functions or types that no longer exist,
  "see also X" where X was deleted, paths to moved or removed files.
- **Contradicted behaviour** — "temporary workaround" for code that has been in place
  for months; "TODO: remove after migration" where the migration is done.
- **Stale section headers** — `// --- Legacy handlers ---` where the legacy code is
  gone; module docs describing features that moved elsewhere.

For each symbol a comment names, check that it still exists (`rg -n "fn name|def
name|struct Name"`).

```bash
empirica finding-log --finding "Stale comment at session_resolver.rs:42 references get_identity_dir(), removed in batch-4" --impact 0.4
```

---

## Phase 4: TODO/FIXME audit

```bash
rg -n "TODO|FIXME|HACK|XXX" "$TARGET"
```

For each: is the work already done (stale TODO → finding, 0.4)? Is it untracked
work that should be a goal (→ unknown)? Was it consciously deferred (check the
decision log; skip if recorded)? Is it a stub placeholder that has since been fleshed
out (→ finding, 0.2)?

```bash
empirica finding-log --finding "Stale TODO at memory_gap_detector.py:23 — feature implemented in batch-2" --impact 0.4
empirica unknown-log --unknown "TODO at firewall.rs:89 'implement rate limiting' — planned or deferred?"
```

---

## Phase 5: Reference-doc alignment

For each reference document (the project's `docs/`, README, architecture notes, and
any registered with `empirica docs-assess`):

1. Read the document end to end.
2. Extract the file paths, functions, types and commands it names.
3. Check each still exists — a quick dead-path pass:
   `rg -o "src/[A-Za-z0-9_./-]+" doc.md | sort -u | while read p; do [ -e "$p" ] || echo "DEAD $p"; done`
4. Check code examples still compile or run in principle (imports, signatures).
5. Check architectural claims against the code, not against memory.

| Issue | Impact |
|---|---|
| Dead file path | 0.7 |
| Dead function / type | 0.6 |
| Stale code example | 0.5 |
| Outdated architectural claim | 0.7 |

When remediation follows, rewrite each affected document as a whole rather than
patching the lines you flagged — a doc fixed line by line keeps its stale framing
elsewhere.

---

## Phase 6: Meta-check — the instructions themselves

Check `AGENTS.md` files, skill `SKILL.md` files and plugin configuration for
references to commands, flags or workflows that have changed:

1. Commands named in skills and `AGENTS.md` still exist (`empirica --help`,
   `<tool> --help`).
2. Flags mentioned still exist on those commands.
3. Hook scripts reference valid tools and events.

```bash
empirica finding-log --finding "SKILL.md for code-audit names 'empirica status --full' — no such flag" --impact 0.5
empirica unknown-log --unknown "AGENTS.md references --type on project-search — verify the flag exists"
```

---

## Phase 7: Triage and goals

| Tag | Meaning |
|-----|---------|
| `phantom-param` | Documented parameter doesn't exist in code |
| `missing-param` | Code parameter missing from documentation |
| `stale-todo` | TODO describes work that is already done |
| `dead-ref-doc` | Reference doc names code that doesn't exist |
| `stale-comment` | Comment describes removed or changed behaviour |
| `wrong-raises` | Errors/raises section doesn't match code |
| `meta-drift` | SKILL.md / AGENTS.md references stale commands |
| `behavioral-mismatch` | Documented behaviour doesn't match actual behaviour |

Group related findings into goals, one per documentation surface, with a task per
document:

```bash
empirica goals-create --objective "Rewrite docs/architecture.md against current module layout" \
  --description "Findings <ids>: 5 dead symbol references, 2 outdated claims."
```

One goal is one praxic transaction; the `epistemic-transaction` skill owns sizing.

---

## Phase 8: Summarise

Tell the user: the scope checked, doc-comment accuracy, stale comments, TODO status,
reference-doc health, instruction drift, goals created (with ids), and the unknowns
that need a human judgment.

---

## Output contract

| Artifact | Purpose |
|---|---|
| **Findings** (impact-scored) | Documentation–code mismatches, stale content |
| **Unknowns** | Ambiguous cases needing human judgment |
| **Decisions** | Conscious choices to keep certain doc patterns |
| **Goals** | Remediation work packages |

## Key design principle

This skill uses **judgment**, not just pattern matching. A phantom parameter might be
an intentional passthrough. A stale TODO might be a conscious deferral. A behavioural
claim might be approximately right. **When uncertain, log an unknown, not a finding**
— false positives erode trust in the audit, and a well-calibrated unknown is worth
more than a noisy finding.

## Re-running

Idempotent. Running it again after remediation shows which mismatches are resolved,
which persist, and what regressed from recent changes.
