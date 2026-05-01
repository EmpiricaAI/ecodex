# Web Product Vision — Empirica for Non-Coders

**Status:** planned exploration · Created 2026-05-01 · Deferred until ecodex (dev-side) MVP ships

## What this is

A web-frontend product built on `codex-app-server` v2 RPC, targeted at **non-coder knowledge workers**: paralegals, accountants, marketers, analysts, customer-service operators, sales teams. Same engine as ecodex, different surface, different default tools, different persona.

## What this isn't

- Not a competitor to general-purpose chatbots (ChatGPT/Claude.ai). Differentiator is the calibration layer.
- Not a replacement for ecodex. ecodex stays focused on developers.
- Not a full SaaS in v0. Could be a desktop app shipping `codex-app-server` + an Electron/Tauri shell.

## Why this is viable

Codex's architecture is more general than its current product:

| Component | Already generic | Already code-shaped |
|---|---|---|
| `codex-app-server` v2 RPC (typed, TS-exported) | ✅ | |
| Plugin system + MCP integration | ✅ | |
| Multi-provider routing (Ollama, LMStudio, etc.) | ✅ | |
| Realtime WebRTC (voice) | ✅ | |
| Memory pipeline | ✅ | |
| Sandbox model | ✅ | (assumes filesystem workspace) |
| Default tool stack (`shell`, `apply_patch`) | | ✅ |
| Default system prompt / persona | | ✅ |
| Bundled skills | | ✅ |
| TUI | | ✅ |

So the build is **frontend + domain plugins + persona swap** — not engine replacement.

## The calibration wedge

For coders, ground truth is built in: tests pass, builds succeed, lint clean, git diffs commit. Empirica calibration grounds against these.

For non-coders, **there is no `pytest` for "did this email land well"**. AI assistants confidently hallucinate; users have no signal that says "this draft is wrong" until consequences arrive.

Empirica's calibration model — *self-assessed vectors compared against deterministic-service observations, divergence is the signal* — is **domain-agnostic**. The services change; the framework doesn't. That makes Empirica a natural trust layer for non-code work where confidence-without-evidence is the failure mode.

The catch: ground-truth signals in non-code domains arrive **asynchronously**. POSTFLIGHT happens now (with self-assessment), grounded confirmation arrives later (recipient reply, customer satisfaction score, document acceptance). See `async-calibration-research.md` for the research direction.

## Architecture sketch

```
[Web frontend (chat + voice)]
      ↕ (typed RPC over HTTP/WS)
[codex-app-server v2]
      ↕
[codex agent runtime: agent loop, tool dispatch, sandbox, memory]
      ↕
[Domain plugins: replace shell/apply_patch with non-code tools]
[Empirica plugin: PreToolUse/PostToolUse/SessionStart/Stop hooks → empirica CLI subprocess]
[MCP servers: GMail, Calendar, GDrive, Notion, Slack, etc.]
[Multi-provider: Ollama for local privacy, hosted models for capability]
```

## Three target personas

| Persona | Daily friction | Plugin stack |
|---|---|---|
| **Paralegal** | Contract review, redlining, clause comparison | docs MCP, document-diff plugin, citation-validator plugin |
| **Customer-service ops lead** | Triage backlog, draft replies, escalation routing | inbox MCP, sentiment-classifier plugin, escalation-rule plugin |
| **Marketing analyst** | Campaign reporting, content briefs, competitor synthesis | analytics MCP, web-search plugin, brief-template plugin |

Each persona is a different default plugin set + persona prompt + empirica configuration. Same engine.

## Open questions

1. **Frontend technology** — Electron + React (familiar)? Tauri + Vue (smaller binary)? Plain web (web access only)? Affects deployment model (desktop app vs SaaS).
2. **Authentication & multi-tenancy** — Codex assumes single-user local install. SaaS multi-tenant is a different operational model.
3. **Sandboxing for non-code** — codex's sandbox assumes a local workspace. For non-coder use, sandbox model needs rethinking (network-only access? per-tool permission UI?).
4. **Pricing model** — per-seat? usage-based? bundled with codex license?
5. **Plugin marketplace integration** — does this product reuse codex's marketplace, or have its own curated set?
6. **Voice interface depth** — full WebRTC pipeline ready out of box, or wait for v2?
7. **Empirica plugin UX** — non-coders won't read calibration vectors directly. The calibration data needs to surface as "trust score" / "confidence indicator" / domain-meaningful signals.

## Deferred decisions (require deeper exploration)

- Whether this ships as branded ecodex variant (`ecodex-pro` for non-coders) or as separate product
- Whether it's open-source like ecodex or commercial-only
- Whether to lead with vertical (e.g. legal-only) or broad (multi-domain from day one)
- How tightly to couple async calibration research to v0 — could ship without it (sync-only calibration) and add async later

## Why log this now

Captured while the strategic context is fresh. Returning to this in a future session is much harder if we don't park the thinking now. Linked empirica goal: "Future: web/non-coder product on codex-app-server v2 RPC". Status: planned. Will activate after ecodex dev-side MVP ships.
