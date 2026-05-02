# ecodex Architecture Decision (T3)

**Date:** 2026-05-01 · **Branch:** `inspect/codex-rs` → carried forward on `build/v1-plugin`

Architectural commitments for the v1 ecodex build. Driven by [`inspection.md`](inspection.md) (T2 investigation findings).

## Decisions

### D1 — Distribution: dual product

Ship **both**:
- **`empirica` plugin for codex** — published to codex's plugin marketplace. Reaches anyone already on codex (developers + future non-coder market).
- **`ecodex` fork** — branded one-click install with empirica plugin pre-bundled, curated open-weights provider defaults (Llama/Qwen/DeepSeek via Ollama/vLLM), Empirica-aware UX. Sold to existing Empirica clients.

Same plugin codebase ships both. The fork is distribution + branding, not a technical fork.

### D2 — Empirica-language strategy: subprocess shellout (Option A)

Empirica's Python CLI stays canonical. The codex plugin's hooks shell out to it (matches the existing Claude Code integration pattern).

Latency analysis (from inspection): 30–270 hook fires/session × 100–300ms/fire = 1–15% session overhead. Tolerable for v1.

Options B/C/D/E (sidecar IPC, PyO3, full Rust port, AI-translated parity Rust) are deferred. They become relevant only if real-world telemetry shows per-tool-call latency unacceptable.

### D3 — Goal pairing: integrate, conditional on prototype validation passing in v1 build

Empirica project-goals pair with codex thread-goals. ThreadGoal struct (verified at `protocol/src/protocol.rs:3608`) has no metadata field, so pairing uses:
- **Embed empirica goal_id in codex thread-goal `objective` text** via stable tag prefix `[empirica:<goal_id>]`. Lossy but no upstream protocol change required.
- **Stop hook** captures thread completion and grounds empirica goal completion against codex thread outcome.
- **Token budget** flows from codex thread-goal back to empirica via PostToolUse hook payload (codex tracks `tokens_used`).

Validate the round-trip during v1 plugin build. If protocol changes prove necessary, escalate.

### D4 — Memory pipeline: parallel coexistence

Codex's Phase 1/2 memory pipeline operates on rollouts, not external sources. Empirica artifacts (findings, decisions, etc.) live in their own store under `~/.codex/empirica/` (or wherever empirica's existing storage lands per its CLI conventions).

If cross-pollination becomes valuable later, build a thin consolidator that reads empirica artifacts and writes codex-memory-format markdown. Out of scope for v1.

## Working assumptions to validate during v1

- Codex hook protocol is stable across recent codex versions (recorded as assumption with confidence 0.7).
- Plugin marketplace install path / format is stable enough to publish against (confidence 0.6).
- Subprocess shellout latency is tolerable for typical sessions (confidence 0.7 — needs runtime confirmation).

## Strategic posture (added 2026-05-02)

ecodex is a **product fork** of codex (Apache-2.0), not a derivative. Quality issues we hit — including upstream lint, bugs, sub-optimal patterns — get fixed by us in our fork **and** PR'd back upstream. We behave as good citizens of the codex ecosystem.

Implications:
- Real product ownership = caring about quality across the entire codebase, not just our additions
- Hardening flows both ways (their improvements come to us via main-sync; ours go to them via PRs)
- Public framing: "Empirica's branded build of codex with bundled defaults", not "a fork that diverges"

## Parked for future sessions (planned empirica goals)

| Goal | Spec | Status |
|---|---|---|
| Web/non-coder product on codex-app-server v2 RPC | [specs/web-product-vision.md](specs/web-product-vision.md) | planned |
| Asynchronous-ground-truth calibration research | [specs/async-calibration-research.md](specs/async-calibration-research.md) | planned |
| Cockpit (multi-instance Empirica TUI) integration | — | planned |
| Symphony (OpenAI multi-agent orchestrator) integration evaluation | — | planned |

These deferred goals share infrastructure with ecodex (same engine, same plugin system) but are scoped to different audiences and require separate architecture work. Activating them is a future-session decision.

## Next phase — v1 build

1. **Build v1 of `empirica` plugin for codex** — manifest + hook scripts (porting CC's `sentinel-gate.py` etc.) + skill registration. *In progress.*
2. **Build ecodex distribution layer** — branding swap, bundled config, curated provider defaults. *Planned (depends on #1).*
3. **Validate D3 + assumptions during v1 build** — runtime hook latency measurement, goal-pairing round-trip, plugin marketplace publish flow.
