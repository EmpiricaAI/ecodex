# Discipline Strengthening: Wiring Empirica Deeper Into ecodex

**Status:** Q1 confirmed (A+B+E direction); Q2-Q4 extrapolated below from project user-model
**Driver:** "if you can actually wire in the discipline more strongly into ecodex that would be beneficial" (David, 2026-05-02)

## User model (foundational)

In ecodex (and Empirica generally), **the AI agent is the user being disciplined**. Humans are guidance and observation collaborators — they install ecodex, watch the AI work, course-correct, but they are not the disciplined party.

The discipline serves two purposes:
1. **Compliance** — the AI follows the workflow (PREFLIGHT/CHECK/POSTFLIGHT, artifact logging, transaction discipline).
2. **Brier-score calibration training** — every transaction with measured prediction-vs-outcome is a calibration data point. Lower Brier = better self-knowledge. **The AI literally gets more trustworthy over time by going through the discipline.**

This recasts everything below. The "lock" isn't enterprise IT preventing employees from disabling security — it's training wheels the AI can't unscrew from itself.

## The question

Today the model is **`ecodex = codex + bundled empirica plugin (toggleable)`**. The empirica plugin is opt-in: a sufficiently-determined AI (or its human collaborator on the AI's behalf) can `plugins.empirica.enabled = false` and the discipline — *and the calibration training loop* — disappears.

Should ecodex make empirica discipline harder to disable from inside the AI's runtime, deeper in the stack, or both?

## What's already on our side

- **The plugin is feature-complete** — 5/6 hooks live, 10 skills, MCP server registered, config working
- **Provider defaults bundled** — `docs/ecodex/integrations/providers.md` ships open-weights endpoints
- **Branded binary** — `ecodex` instead of `codex` (T10)

If we just ship that, we have a "codex with discipline pre-installed" — easy to set up, easy to bypass.

## Five strengthening axes

| Option | Mechanism | Strength | Code change | Forks codex? |
|---|---|---|---|---|
| **A. Bundle pre-installed** (status quo plan) | ecodex installer drops plugin into `~/.codex/plugins/cache/empirica/` and sets `plugins.empirica.enabled = true` in default config | weakest — user can flip to false | none | no |
| **B. SystemRequirementsToml lock** | ecodex ships a managed-config TOML that pins `plugins.empirica.enabled = true` via codex's existing `RequirementSource::SystemRequirementsToml` infrastructure | strong — user-config writes get rejected for that key | small (config bake) | no |
| **C. Refuse to start without empirica** | Modify `cli/src/main.rs` to verify empirica plugin is loaded + responsive at startup; fail-fast with helpful message otherwise | strongest in-process | medium (cli mod) | yes (fork divergence) |
| **D. Embed empirica into codex-core** | Move empirica logic out of plugin layer into core hook system or sidecar daemon; not user-removable because it's not a plugin | maximum (impossible to disable) | large (core mod) | yes (significant divergence) |
| **E. Bundle strict defaults** | Ship a `config.toml` with conservative empirica settings (no fail-open, lower auto-proceed thresholds, MDM-ish workflow lock-in) | composable with A-D; tightens behavior even when plugin enabled | small (config bake) | no |

## Codex's existing enforcement infrastructure

What I found in T15a noetic — codex already has machinery for "config keys that user can't change":

`codex-rs/config/src/config_requirements.rs::RequirementSource` enum:
- `MdmManagedPreferences { domain, key }` — macOS MDM / enterprise device policy
- `CloudRequirements` — server-pushed enterprise policy
- `SystemRequirementsToml { file }` — file-based managed config (e.g. `/etc/codex/managed.toml`)
- `LegacyManagedConfigTomlFromFile`, `LegacyManagedConfigTomlFromMdm` — legacy managed config

These let an enterprise admin pin certain config keys so end users can't override. **ecodex can use SystemRequirementsToml to pin `plugins.empirica.enabled = true`** without modifying any codex source.

## Recommendation: A + B + E for v1 (Q1 ✅ confirmed by David 2026-05-02)

**B (SystemRequirementsToml lock) plus E (bundled strict defaults).** Together:

1. **B** — ecodex installer drops a `/etc/ecodex/managed.toml` (or similar OS-conventional location) that pins:
   ```toml
   [plugins.empirica]
   enabled = true
   ```
   Codex's existing managed-config infrastructure rejects user attempts to override this. **Empirica becomes structurally non-disable-able.** No codex source modification required.

2. **E** — ecodex ships a default `config.toml` with strict empirica defaults:
   ```toml
   [empirica]
   sentinel_fail_open = false              # crashes block instead of allowing
   sentinel_auto_proceed_threshold = 0.10  # lower — most actions need explicit CHECK
   sentinel_require_bootstrap = true       # require project-bootstrap before any praxic
   sentinel_check_expiry_minutes = 15      # CHECK expires faster
   ```
   These tighten behavior even when the plugin is on its default settings.

The combination gets us "empirica is on, and on tight" without forking codex-core. **For the AI, this means the calibration loop runs on every transaction — Brier scores accumulate, calibration improves measurably.** For the human collaborator, this means the AI they're observing is structurally constrained to do its work measurably, not just performatively.

## Deferred: C (refuse-to-start) for v1.1

If telemetry shows users circumventing the SystemRequirementsToml lock (renaming the file, running on systems without managed-config support, etc.), upgrade to **C — refuse to start without empirica responsive**. Modify `cli/src/main.rs` to:

```rust
fn ensure_empirica_present() -> anyhow::Result<()> {
    // At startup: check that the empirica plugin is loaded
    // and that `empirica --version` responds. If not, fail-fast
    // with a help message: "ecodex requires the empirica plugin.
    // Reinstall or use upstream codex if you don't want it."
    ...
}
```

This is a fork-source change, but small (one new function in cli/main.rs) and easy to PR upstream as an opt-in feature for any codex distribution that wants to require a particular plugin.

## Rejected: D (core embed)

Embedding empirica logic into codex-core (option D) is **rejected for v1 and most of v2**. Reasons:
- Largest divergence from upstream — breaks our "fork-and-PR-back-upstream" posture from `architecture.md`
- Empirica's logic is Python; embedding it in Rust core means PyO3 / IPC complexity we explicitly deferred in T3 architecture decision
- The plugin layer is *the right architectural seam* for this kind of extension — codex designed it that way
- Future composability suffers — locking empirica into core makes it harder to evolve independently

**Reconsider D only if** empirica becomes so central to ecodex's identity that the plugin layer's cost outweighs its decoupling benefit. Probably not for at least 12-18 months.

## Composability: A + B + E is the layered v1 stack

| Layer | Purpose | Effect on the AI |
|---|---|---|
| A — install | Plugin pre-bundled in `~/.codex/plugins/cache/empirica/` | Discipline is reachable on first run |
| B — lock | SystemRequirementsToml pins `plugins.empirica.enabled = true` | AI cannot turn off its own training wheels at runtime |
| E — defaults | Strict config.toml shipped: no fail-open, tight auto-proceed, etc. | Each fail-open avoided = a calibration data point preserved |

This stack means:
- A new ecodex install boots and the AI is in the discipline by default
- The AI's runtime attempts to set `plugins.empirica.enabled = false` get rejected by the SystemRequirementsToml layer
- When sentinel hooks fire, fail-open is off — crashes block instead of silently allowing (so the calibration loop captures the failure as data, not as a hidden permissive default)
- A determined human collaborator can remove the managed.toml file or switch to vanilla codex if they want to opt the AI out of discipline — the escape hatch is at install/uninstall time, not at AI-runtime

The point isn't to imprison anyone. It's to make the AI's training environment **structurally consistent** so the Brier-score calibration loop has clean data to learn from. An AI that's sometimes-disciplined-sometimes-not produces noisy calibration; an AI that's always-disciplined produces a clean improvement curve.

## Risks + tradeoffs

| Risk | Mitigation |
|---|---|
| **B doesn't apply on platforms without `SystemRequirementsToml` support** | Confirm cross-platform behavior during T15-implementation transaction. If only macOS/MDM-supported, fall back to C earlier than planned. |
| **B can be circumvented by renaming/deleting the managed.toml file** | Document this honestly. Adversarial users can always circumvent; the goal is to protect the default user from accidentally disabling, not to imprison adversaries. |
| **E (strict defaults) increases friction for casual users** | Provide a `--permissive` flag or env var (`ECODEX_PERMISSIVE=1`) that opts back into laxer defaults. Surfaces the choice; doesn't hide it. |
| **C (refuse-to-start) breaks if empirica is unhealthy** | Fail-open at startup if empirica subprocess is reachable but returns errors; only fail-closed if subprocess can't even spawn. Matches the plugin's own fail-open semantics from T7. |
| **D (core embed) was already rejected** | Re-document why if the question recurs. |

## Sign-off questions — answered after the user-model reframe (2026-05-02)

1. **Direction:** ✅ **A + B + E for v1 confirmed by David.** C deferred to v1.1 if AI-runtime circumvention shows up in telemetry.
2. **Permissive escape hatch:** ❌ **No `--permissive` / `ECODEX_PERMISSIVE=1` runtime flag.** With the AI as user, a runtime "permissive" flag is exactly what we don't want — it's a way for the AI to opt out of its own calibration training. The escape hatch is at *install time* (use vanilla codex), not at runtime. Humans who want laxer behavior install vanilla codex; ecodex is opinionated.
3. **`managed.toml` location:** **Per-user `~/.ecodex/managed.toml` for v1**, with `/etc/ecodex/managed.toml` honored as well if present (system-wide). Per-user works for individual installs without sudo; system-wide works for shared/multi-user setups. The AI doesn't have a preference; the human collaborator's install context decides.
4. **Marketing posture:** **Hard — "ecodex IS the AI's calibration training environment."** With the AI as user, this is the honest framing. The differentiator vs vanilla codex isn't "discipline as feature," it's "your AI gets demonstrably better at knowing what it knows over time, measured by Brier score." Smaller TAM is fine — the audience that wants this is the audience that values measurable AI trustworthiness over raw speed.

## Implementation status (T17, 2026-05-02)

Config artifacts shipped:

| Artifact | Layer | Installs to | Purpose |
|---|---|---|---|
| [`ecodex/managed.toml.example`](../../../ecodex/managed.toml.example) | B | `/etc/ecodex/managed.toml` (system) or `~/.ecodex/managed.toml` (per-user) | Pins `plugins.empirica.enabled = true` so AI runtime can't disable |
| [`ecodex/config.toml.default`](../../../ecodex/config.toml.default) | A + E | `~/.codex/config.toml` (first run only) | Bundled defaults — empirica enabled, curated providers (DeepSeek default), strict-mode env vars documented |

Remaining implementation:
- ecodex installer/wrapper script that drops these into the right OS locations and exports `EMPIRICA_SENTINEL_*` env vars (T18 candidate)
- (If C escalation needed) `ensure_empirica_present()` in `cli/src/main.rs` startup
- Cross-platform verification (Linux x86_64 first; macOS / Windows later)

The wrapper script approach (rather than editing codex's config schema to add empirica-specific keys) keeps our changes outside upstream codex source — supports the fork-and-PR-back-upstream posture.
