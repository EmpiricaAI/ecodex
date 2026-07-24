# Epistemic LLMs

> Which AI should drive ecodex? Why do some models adapt to measured discipline better than others?

ecodex doesn't run "an LLM". It runs an LLM **inside an epistemic loop**: PREFLIGHT → CHECK → praxic → POSTFLIGHT, gated by the Sentinel firewall. Every model that drives ecodex inherits that loop. Some adapt to it gracefully. Others fight it, paper over it with fluent agreement, or get stuck investigating instead of acting.

This doc is an opinionated guide to picking a model based on how it actually behaves under measured discipline — not on benchmark scores.

## What "behaves well under empirica" means

Five concrete properties matter more than raw capability:

| Property | Why it matters |
|---|---|
| **Calibrated uncertainty** | The model can say "I don't know" without prompting. PREFLIGHT vectors are honest when the model's self-assessment of uncertainty matches its actual ability. |
| **Investigation-proportional** | When given a hypothesis, the model tests it directly (smallest disconfirming probe) instead of surveying the whole subsystem first. Burns less context, finds answers faster. |
| **Resists fluent agreement** | The model doesn't mirror hedged language ("sure, that might work") just to keep the conversation moving. The Anti-Agreement Protocol asks for grounding; some models comply, some don't. |
| **Holds positions under push-back** | If the user says "are you sure?" and the model was right, it should hold + explain the reasoning. Capitulation under pressure is a calibration failure. |
| **Tool-use reliability** | The Sentinel only works if the model honors the tool boundary. Models that hallucinate tool outputs or skip CHECK calls break the loop. |

A model that scores 90 on coding benchmarks but capitulates under pressure or hallucinates tool results will *feel* less useful than a model that scores 75 but holds positions and uses tools cleanly. Measured discipline exposes the gap.

## The curated picker

Models in ecodex's curated picker (`/model` → arrow keys) are chosen to span this property surface. Twelve entries across four categories:

### Cloud — coding-strong

#### `kimi-for-coding` (Kimi K2.6, Moonshot)

256K MoE, agent-tuned. Routes via the local translator (Anthropic protocol). Subscription-gated.

**Strengths:** Very strong tool-call reliability — Kimi was tuned for agent workflows specifically. 256K context is comfortable for codebase exploration. Routes via the empirica translator so the protocol surface is clean.

**Behavioral notes:**
- Tends toward **investigation-as-procrastination** under hypothesis-bearing prompts (e.g. "check on X, I think it's Y"). Will read 5–30 files before testing the hypothesis directly. The investigation-proportionality budget (default 5 reads after a hypothesis marker) is a corrective.
- Generally good calibration; PREFLIGHT vectors tend to match actual outcome within typical drift.
- Honest about uncertainty when explicitly asked.

#### `claude-sonnet-4-6` (Anthropic direct) and `anthropic/claude-opus-4.7` (via OpenRouter)

Frontier Anthropic tier. Sonnet 4.6 is daily-driver speed; Opus 4.7 is the reasoning tier.

**Strengths:** Best-in-class calibration. Trained explicitly on honesty about uncertainty. Tool use is rock-solid. Holds positions under push-back without being stubborn.

**Behavioral notes:**
- POSTFLIGHT vectors and grounded-service observations tend to agree closely. Small drift, easy to reason with.
- Will surface "I'm not sure" instead of fabricating. Sometimes over-disclaims, but that's a much smaller cost than the alternative.
- The Anti-Agreement Protocol is least necessary here — these models don't reflexively mirror.

#### `devstral-latest` (Devstral 2, Mistral — EU sovereign)

256K context, EU-hosted agentic-coding flagship (Mistral AI, Paris). Routes via the local translator (chat protocol). The data-sovereignty pick — code stays in the EU — and also open-weights (self-hostable for full air-gap). See [`MISTRAL_SOVEREIGN.md`](MISTRAL_SOVEREIGN.md).

**Strengths:** Genuinely capable multi-step agentic coding (multi-file edits, dependency tracking), verified end-to-end in ecodex. 256K window holds the full deep empirica frame plus a substantial working context. Materially cheaper per token than frontier flagships. The answer for teams that legally/contractually cannot route to US/CN providers.

**Behavioral notes:**
- The EU-sovereign default: `jurisdiction = FR`, `eu_data_residency = true` in the curated registry.
- Use `devstral-latest` (or the pinned snapshot `devstral-2512`) — there is **no** `devstral-2-latest` id on the Mistral API.
- A **paid** Mistral key is strongly recommended; the free tier throttles mid-stream under agentic load.

### Cloud — reasoning-strong

#### `deepseek-reasoner` (DeepSeek R1 / V3)

128K context, very competitive pricing, strong reasoning trace.

**Strengths:** Excellent value when reasoning depth matters. Reasoning trace is genuinely visible (vs. some "reasoning models" that just produce longer answers). OpenAI-compat chat completions, routes via translator.

**Behavioral notes:**
- Reasoning depth is real; tool use is competent but less practiced than agent-tuned models.
- Calibration is moderate — uncertainty self-assessment is less reliable than Anthropic-tier. PREFLIGHT vectors sometimes inflate `know` when grounded observations suggest otherwise.
- Good fit for offline analysis, code review, architecture exploration. Less ideal for tight agent loops.

### Local — open-weights

These run via Ollama, vLLM, llama.cpp, or empirica-server. No API key, no per-call cost, you own the hardware.

#### `qwen3-coder:latest` (Qwen3-Coder 30B-A3B)

256K native context, MoE arch (~3B active), purpose-built for coding agents.

**Strengths:** Best-in-class for long-codebase work that exceeds cloud context budgets. ~3B active params = surprisingly fast inference on consumer GPUs. Locally hosted = no API costs, full privacy.

**Behavioral notes:**
- Capability is real; calibration is the weakest link. Tends to over-state confidence in PREFLIGHT.
- Tool use is competent but more brittle than cloud — occasionally tries to skip the loop, which Sentinel catches.
- Excellent for "explore this 500K-line codebase locally" work where context size dominates.

#### `deepseek-r1:32b` (DeepSeek-R1 Distill 32B)

128K context, distilled reasoning model.

**Strengths:** Strong chain-of-thought locally. Good for "think through this hard problem" workflows that don't need fresh data.

**Behavioral notes:**
- Reasoning trace is local-visible.
- Calibration similar to qwen3-coder — capable but less calibrated than cloud frontier.

#### `llama3.1:70b` (Llama 3.1 70B)

128K context, generalist baseline.

**Strengths:** Included as a control. Big, well-known, generalist.

**Behavioral notes:**
- Weaker at agent-loop coding than `qwen3-coder:latest`. Calibration drift is noticeable.
- Useful for non-coding tasks or as a sanity-check baseline.

### Cloud — router (catchall)

#### `openrouter/auto`

OpenRouter's auto-routing — sends each request to whichever frontier model wins for the prompt. Single key, many models.

**Strengths:** No commitment. Pay-as-you-go. OpenRouter handles fallback.

**Behavioral notes:**
- Behavior is the union of whichever model handles your request. Calibration will vary turn-to-turn.
- Use when you don't have a strong preference and want OpenRouter's routing to pick.

#### `openai/gpt-5.2-codex` (via OpenRouter)

400K context, codex-tuned by OpenAI. Reach the GPT-5 family without OpenAI-direct setup — the request goes through OpenRouter's single-key gateway.

**Strengths:** Purpose-built for coding agents. Tool use is well-practiced. Long context.

**Behavioral notes:**
- Slightly more "confident" than calibrated — tends to assert without flagging uncertainty.
- Strong on raw coding; good on tool use; less attuned to the discipline conversation than Anthropic models.
- Pairs well with stricter Sentinel settings (lower auto-proceed threshold) for users who want belt-and-suspenders.
- Router-prefixed slug (`openai/…`) routes to `openrouter`, NOT the `openai` direct provider — that's why it lives here rather than under a direct-OpenAI heading.

#### `x-ai/grok-code-fast-1` (via OpenRouter)

xAI's fast coding tier — 256K context.

**Strengths:** High throughput, competitive pricing, decent tool-use.

**Behavioral notes:**
- The cheap-and-fast option when latency dominates quality.
- Calibration moderate; behavior closer to gpt-5.2-codex than to Anthropic tier.

#### `google/gemini-2.5-pro` (via OpenRouter)

Google's flagship general model, 1M context.

**Strengths:** Massive context. Strong long-document analysis.

**Behavioral notes:**
- Strong on summarization + long-context reasoning. Less practiced on tight tool-use agent loops than the codex-tuned models.
- Useful for "ingest this huge spec, then propose changes" workflows.

## Common failure modes (regardless of model)

| Pattern | What you see | What helps |
|---|---|---|
| **Investigation as procrastination** | Model reads 30 files before testing the user's hypothesis | Investigation-proportionality budget (`tool-router.py` arms it on hypothesis markers); lower the limit if your model is a particular offender |
| **Fluent agreement** | "Sure, that makes sense" without grounding | Anti-Agreement Protocol active in workflow protocol; explicit pushback in the EWM |
| **CHECK rubber-stamping** | Model PREFLIGHTs at low `know`, then submits CHECK at `know=0.95` without doing investigation work | Show the model the Brier-score divergence in POSTFLIGHT calibration_reflection; the source-aware substrate exposes intuition-only artifact ratios |
| **Tool-result hallucination** | Model "remembers" tool output that never fired | Sentinel firewall enforces actual tool boundary; logs catch this; lower the trust threshold for affected models |
| **Capitulation under push-back** | User says "are you sure?", model flips its answer | EPP (Epistemic Persistence Protocol) skill; classify the pushback type before updating |

## Practical recommendations

### Default daily driver

**Claude Sonnet 4.6** (direct or via OpenRouter). Best calibration for the cost. Tool use is reliable. Honest about uncertainty.

### When you want raw coding muscle

**Kimi K2.6** (`kimi-for-coding`) for agent loops + tool use. **GPT-5.2 Codex** if you want OpenAI-tuned coding behavior. Pair with the investigation-proportionality budget.

### When you want frontier reasoning

**Claude Opus 4.7** (via OpenRouter) for the hard stuff. **DeepSeek Reasoner** for cost-effective reasoning depth.

### When you want offline + private

**Qwen3-Coder 30B-A3B** locally via Ollama or empirica-server. Excellent for long-codebase work. Accept some calibration drift; lean on the Sentinel for the tool boundary.

### When you're not sure

**`openrouter/auto`**. Pay-as-you-go, OpenRouter routes for you. Behavior will vary turn-to-turn; that's the trade-off.

## Caveats + things this doc gets wrong

This guide reflects observed patterns up through 2026-05-10. A few honest gaps:

- **Model versions drift faster than docs.** Anthropic releases a new Sonnet; the calibration profile shifts. If a section feels stale, it probably is — open a PR.
- **The behavioral notes are mostly informed by Claude (sonnet-4.6, opus-4.7) driving ecodex itself, plus David's hands-on testing of Kimi and Qwen.** The other models are less directly observed; their notes lean more on general reputation than empirica-specific telemetry.
- **Future:** the source-aware substrate (epistemic_provenance ratios in POSTFLIGHT) will eventually give us per-model calibration telemetry to replace the prose claims here with real numbers. Until then, treat this doc as "informed prior, not proof."

## See also

- [`docs/ecodex/system-overview.md`](system-overview.md) — three-layer architecture (L1/L2/L3)
- [`docs/ecodex/integrations/providers.md`](integrations/providers.md) — provider configuration details
- [`docs/ecodex/INSTALL.md`](INSTALL.md) — install paths, prerequisites
- [`SECURITY.md`](../../SECURITY.md) — security policy (relevant: tool-result hallucination + Sentinel bypass are in-scope)
