# Compliance Crosswalk — ecodex / Empirica → EU AI Act · GDPR · ISO/IEC 42001

**Thesis:** compliance here is not a binder of policy prose. Every row below maps
a regulatory requirement to a **deterministic check that already runs** — in CI,
on `release --prepare`, and on demand via `empirica compliance-report`. The
evidence is a command's exit code and a git-anchored artifact, not an
attestation. That is the difference between *claiming* a control exists and
*producing the check that proves it ran*.

> Scope honesty up front: this is a **crosswalk**, not a certification. It shows
> which deterministic mechanisms map to which regulatory articles, and where
> coverage is real vs. partial vs. absent. "Maps to Art. X" means "this check is
> evidence toward that article's intent," not "ecodex is certified under Art. X."
> Conformity assessment is a legal process; this is the engineering substrate
> that makes one tractable.

---

## How to read this — universal invariant + substrate-specific extractor

The crosswalk decomposes into two parts (a framing developed jointly with the
empirica practice while porting the harness-integrity guards):

- **Universal invariant** — the *structure*: a fixed set of deterministic checks,
  each pinned to specific EU AI Act / GDPR / ISO 42001 articles. This is
  canonical and machine-readable — it lives in empirica core's `REGULATORY_MAP`
  (`empirica/cli/command_handlers/compliance_report_commands.py`), not in this
  document. This doc renders it; it does not author it.
- **Substrate-specific extractor** — the *check inventory*: which of those checks
  actually run for a given codebase, plus any the codebase adds on its own
  surface. empirica core's extractor is its Python package; ecodex's is the
  vendored hook layer + the Rust crate + the codex integration surface.

The interesting output of any two harnesses adopting the same invariant is the
**coverage delta** per article — what each covers that the other does not. That
delta (§4) is an artifact neither harness had alone.

---

## 1. The crosswalk

Status legend:
- **Inherited** — ecodex runs this check via the installed empirica core
  (`empirica compliance-report`), no ecodex-specific code needed.
- **Inherited + extended** — runs via core *and* ecodex adds a check on its own
  surface that strengthens the same article.
- **ecodex-native** — a determinism ecodex adds that has no empirica-core analog
  (the codex fork's specific surface).
- **Partial** — runs but coverage is known-incomplete (tracked goal).

| Check (deterministic) | EU AI Act | GDPR | ISO/IEC 42001 | ecodex status |
|---|---|---|---|---|
| Static analysis (ruff) | Art. 9 — risk management / code quality | — | 6.1.2 — source code quality | Inherited |
| Cyclomatic complexity (radon) | Art. 15(1) — accuracy / maintainable code | — | 8.4 — complexity management | Inherited |
| Type checking (pyright) | Art. 15(1) — type-safe operations | — | 8.4 — correctness guarantees | Inherited |
| Test suite (pytest) | Art. 15(3) — robustness / functional verification | — | 8.5 — testing & validation | **Inherited + extended** (vendored-hook suite) |
| Dependency audit (pip-audit) | Art. 15(4) — supply-chain security | Art. 32 — dependency integrity | A.7.5 — third-party components | Inherited |
| SAST security scan (semgrep OWASP) | Art. 15(4) — OWASP scanning | Art. 25 — data protection by design | 8.4 — secure coding | Inherited |
| Secret/credential scan (trufflehog) | Art. 15(4) — credential leak prevention | Art. 32 — secret management | A.7.5 — credential hygiene | Inherited |
| Technical documentation (docs-assess) | Art. 11 + Annex IV — technical documentation | — | 7.5.1 — documented information | **Partial** (coverage goal: 37.5% → ≥75%) |
| Tech-doc link integrity | Art. 11 + Annex IV — cross-reference accuracy | — | 7.5.3 — control of documented info | Inherited |
| Release-chain integrity | Art. 10 — release traceability | — | 8.6 — deployment verification | Inherited |
| Repository hygiene (git) | Art. 10 — version control / traceability | — | 7.5 — configuration management | Inherited |
| AI contribution transparency (git attribution) | Art. 50 — AI-generated content disclosure | — | A.8.4 — provenance tracking | Inherited |
| Decision audit trail (rationale coverage) | Art. 13 — interpretable output | Art. 22(3) — right to explanation | 9.1.2 — decision traceability | Inherited |
| Epistemic discipline trajectory | Art. 17 — quality management / process discipline | — | 9.1.3 — process effectiveness | Inherited |
| Epistemic transaction trail | Art. 12 — record-keeping / AI audit trail | Art. 30 — records of processing | 9.1 — monitoring & measurement | Inherited |
| Grounded calibration | Art. 14 — human oversight / self-assessment accuracy | — | 9.2 — internal audit | Inherited |

**Why "Inherited" is the headline, not a hedge.** ecodex shares empirica core
via the installed binary, so 14 of these 16 checks apply to ecodex *for free* —
the same `compliance-report` that audits empirica audits ecodex. The
regulatory-relevant determinism is a **property of the substrate**, inherited
across the fork boundary, not re-implemented per project. (The import-budget
guard, §3, proves the analogous inheritance for runtime hygiene: empirica core's
lazy-import discipline holds across ecodex's Rust spawn boundary with no ecodex
code.)

---

## 2. The empirica-specific articles — where the substrate is the control

Most static-analysis rows above are table stakes any serious codebase can claim.
The rows that are **distinctive to the Empirica substrate** are the ones where the
*control mechanism itself* is the epistemic loop:

- **Art. 12 (record-keeping) / GDPR Art. 30 (records of processing)** ← the
  epistemic transaction trail. Every PREFLIGHT→CHECK→POSTFLIGHT cycle is a
  timestamped, git-notes-anchored record of what the AI knew, decided, and did.
  Record-keeping isn't bolted on; it *is* how the system runs.
- **Art. 13 (interpretability) / GDPR Art. 22(3) (right to explanation)** ←
  the decision audit trail. `decision-log` with rationale + reversibility makes
  the "why" of an automated choice a first-class, queryable artifact.
- **Art. 14 (human oversight)** ← grounded calibration + the ECO gate. The
  Sentinel measures the AI's self-assessment against deterministic evidence
  (tests, git, artifact ratios); divergence is the oversight signal. Praxic
  proposals are ECO-gated — a human decision sits on the critical path for
  consequential action by construction.
- **Art. 50 (AI disclosure) / ISO A.8.4 (provenance)** ← AI contribution
  transparency. Commits carry AI co-authorship; artifacts carry an
  `epistemic_source` tag (intuition vs. search). Provenance is recorded at write
  time, not reconstructed.

This is the positioning point: for the articles that are *hard* — the ones about
auditability, oversight, and explanation of automated decisions — the Empirica
substrate doesn't add a compliance feature, it makes the requirement structurally
unavoidable.

---

## 3. Where ecodex ADDS determinism beyond core (ecodex-native)

ecodex is a fork of `openai/codex` running on the Empirica substrate. Its own
surface contributes determinism that empirica core has no analog for — and the
shared invariant predicts exactly where:

| ecodex-native mechanism | What it deterministically guarantees | Maps to |
|---|---|---|
| **Vendored-hook SQL schema-ref guard** (`#1`) | Every static query the governance hooks issue validates against the real schema — a silently-dead audit/firewall query fails CI | Art. 15(1) accuracy · Art. 12 (protects the record-keeping path itself) · ISO 8.5 |
| **Vendored-hook import-budget gate** (`#3`) | The per-tool-call hook spawn (the Sentinel firewall) stays free of heavy eager imports — bounds the latency of the control that enforces the other controls | Art. 15(3) robustness · ISO 8.4 |
| **Rust spawn / sandbox boundary** | Tool execution crosses an explicit sandbox + writable-roots boundary before any hook or model action | Art. 15(4) cybersecurity · GDPR Art. 25 by-design |
| **Model-routing transparency** (OpenRouter / provider config) | Which model served a turn is explicit config, not hidden — the disclosed-AI surface names the actual model | Art. 50 disclosure · Art. 13 transparency |

The meta-level worth naming: guards `#1` and `#3` are **controls on the controls**.
They don't directly satisfy a regulatory article about the *product*; they
guarantee that the mechanisms which *do* satisfy those articles (the audit trail,
the oversight firewall) cannot silently rot. A dead audit-log query or a
3-second-slower firewall is a compliance regression that looks green — these
guards are what make "the record-keeping path works" a tested claim rather than
a hopeful one.

---

## 4. Coverage delta — ecodex vs empirica core

Same invariant, two extractors. The delta:

- **Identical (14 checks):** every code-quality, security, documentation,
  transparency, and epistemic-audit row is inherited 1:1. ecodex covers the same
  articles as empirica core for these — by sharing the substrate, not by copying
  the checks. Coverage delta: **zero, by inheritance.**
- **ecodex stronger:**
  - *Art. 15 (robustness/accuracy)* — ecodex adds two integrity guards over the
    governance layer itself (`#1`, `#3`) that core does not run on its hooks.
  - *Art. 15(4) (cybersecurity)* + *GDPR Art. 25* — the Rust sandbox/exec
    boundary is an enforcement point core (a Python CLI) has no equivalent for.
  - *Art. 50 (disclosure)* — explicit model-routing config surfaces the serving
    model per turn.
- **ecodex weaker / in progress:**
  - *Art. 11 + Annex IV (technical documentation)* — `tech_docs` coverage is
    partial (37.5%, tracked goal to ≥75%). The codex fork inherits a large
    upstream surface whose docs aren't all ecodex-authored; an auto-discovery
    exclude effort is scoped to measure the *user-facing* surface honestly rather
    than inflate the score.

**Net:** ecodex's coverage is core's coverage **plus** a hardened
governance-integrity and execution-boundary layer, **minus** a documentation gap
that is measured (not hidden) and actively closing. That honest delta — including
the gap — is itself the Art. 17 / ISO 9.1.3 process-discipline artifact: the
system reports where it falls short.

---

## 5. Reproduce it

```bash
# Run the full deterministic check set against this repo and emit the
# machine-readable regulatory mapping:
empirica compliance-report --output json

# The vendored-hook integrity guards (#1, #3) that harden the substrate:
scripts/test-vendored-hooks.sh
```

The crosswalk above is the human-readable rendering of what those commands
produce. If a row's check fails, the corresponding regulatory claim is
*withdrawn automatically* — which is the entire point: compliance posture is a
computed property of the codebase, refreshed every run, not a snapshot that
drifts out of date the moment it's signed.

---

*Sources: empirica core `REGULATORY_MAP` (`compliance_report_commands.py`,
@0f8506427); ecodex vendored-hook integrity guards (`#1` SQL schema-ref
`7d79941a6f`, `#3` import-budget `7d620f2a5a`). Article references are empirica
core's canonical mappings, not independent legal interpretation.*
