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

Today the model is **`ecodex = codex + bundled empirica plugin (toggleable)`**. The empirica plugin is opt-in: a sufficiently-determined AI (or its human collaborator on the AI's behalf) can `plugins."empirica@nubaeon".enabled = false` and the discipline — *and the calibration training loop* — disappears.

Should ecodex make empirica discipline harder to disable from inside the AI's runtime, deeper in the stack, or both?

## What's already on our side

- **The plugin is feature-complete** — 5/6 hooks live, 10 skills, MCP server registered, config working
- **Provider defaults bundled** — `docs/ecodex/integrations/providers.md` ships open-weights endpoints
- **Branded binary** — `ecodex` instead of `codex` (T10)

If we just ship that, we have a "codex with discipline pre-installed" — easy to set up, easy to bypass.

## Five strengthening axes

| Option | Mechanism | Strength | Code change | Forks codex? |
|---|---|---|---|---|
| **A. Bundle pre-installed** (status quo plan) | ecodex installer drops plugin into `~/.codex/plugins/cache/nubaeon/empirica/0.1.0/` and sets `plugins."empirica@nubaeon".enabled = true` in default config | weakest — user can flip to false | none | no |
| **B. SystemRequirementsToml lock** | ecodex ships a `requirements.toml` that pins `plugins."empirica@nubaeon".enabled = true` via codex's existing `RequirementSource::SystemRequirementsToml` infrastructure | strong — user-config writes get rejected for that key | small (config bake) | no |
| **C. Refuse to start without empirica** | Modify `cli/src/main.rs` to verify empirica plugin is loaded + responsive at startup; fail-fast with helpful message otherwise | strongest in-process | medium (cli mod) | yes (fork divergence) |
| **D. Embed empirica into codex-core** | Move empirica logic out of plugin layer into core hook system or sidecar daemon; not user-removable because it's not a plugin | maximum (impossible to disable) | large (core mod) | yes (significant divergence) |
| **E. Bundle strict defaults** | The `ecodex` binary defaults the strict-mode empirica env vars (`EMPIRICA_SENTINEL_REQUIRE_BOOTSTRAP`, `EMPIRICA_SENTINEL_COMPACT_INVALIDATION`, `EMPIRICA_SENTINEL_CHECK_EXPIRY`, `EMPIRICA_CALIBRATION_FEEDBACK`) to `"true"` at startup on every install path | composable with A-D; tightens behavior even when plugin enabled | small (arg0 default) | no |

## Codex's existing enforcement infrastructure

What I found in T15a noetic — codex already has machinery for "config keys that user can't change":

`codex-rs/config/src/config_requirements.rs::RequirementSource` enum:
- `MdmManagedPreferences { domain, key }` — macOS MDM / enterprise device policy
- `CloudRequirements` — server-pushed enterprise policy
- `SystemRequirementsToml { file }` — file-based managed config (codex hardcodes `/etc/codex/requirements.toml` on Unix)
- `LegacyManagedConfigTomlFromFile`, `LegacyManagedConfigTomlFromMdm` — legacy managed config

These let an enterprise admin pin certain config keys so end users can't override. **ecodex can use SystemRequirementsToml to pin `plugins."empirica@nubaeon".enabled = true`** without modifying any codex source.

## Recommendation: A + B + E for v1 (Q1 ✅ confirmed by David 2026-05-02)

**B (SystemRequirementsToml lock) plus E (bundled strict defaults).** Together:

1. **B** — the `--system` installer drops a `requirements.toml` at `/etc/codex/requirements.toml` (the only managed-config path codex hardcodes on Unix) that pins:
   ```toml
   [plugins."empirica@nubaeon"]
   enabled = true
   ```
   Codex's existing managed-config infrastructure rejects user attempts to override this. **Empirica becomes structurally non-disable-able.** No codex source modification required. This lock is **system-only**: per-user (`--user`) installs cannot enforce it without an upstream change, so they ship without it.

2. **E** — the `ecodex` binary defaults the strict-mode empirica env vars at startup (in `codex-rs/arg0/src/lib.rs::apply_ecodex_strict_defaults()`, called after `load_dotenv()` with `${VAR:-true}` semantics — a real env var or `.env` entry still wins):
   ```sh
   EMPIRICA_SENTINEL_REQUIRE_BOOTSTRAP=true   # require project-bootstrap before any praxic
   EMPIRICA_SENTINEL_COMPACT_INVALIDATION=true # invalidate CHECKs after context compaction
   EMPIRICA_SENTINEL_CHECK_EXPIRY=true         # 30-minute CHECK expiry (MAX_CHECK_AGE_MINUTES=30)
   EMPIRICA_CALIBRATION_FEEDBACK=true          # surface Brier trajectory in PREFLIGHT/CHECK
   ```
   These are **env vars consumed by the vendored `sentinel-gate.py`**, not `[empirica]` TOML keys (`sentinel_fail_open` / `sentinel_auto_proceed_threshold` knobs do not exist in the plugin). Because the binary sets them, strict mode is default-ON on **every** install path — curl, brew, `cargo install`, manual binary, and source — not just the source-build wrapper.

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
| A — install | Plugin pre-bundled in `~/.codex/plugins/cache/nubaeon/empirica/0.1.0/` | Discipline is reachable on first run |
| B — lock | SystemRequirementsToml (`/etc/codex/requirements.toml`, `--system` only) pins `plugins."empirica@nubaeon".enabled = true` | AI cannot turn off its own training wheels at runtime |
| E — defaults | The `ecodex` binary defaults the strict-mode `EMPIRICA_SENTINEL_*` env vars to `true` at startup | Strict mode is on by default on every install path, not just the source wrapper |

This stack means:
- A new ecodex install boots and the AI is in the discipline by default
- The AI's runtime attempts to set `plugins."empirica@nubaeon".enabled = false` get rejected by the SystemRequirementsToml layer (on `--system` installs)
- Failure handling is a **two-layer** reality: the Rust PreToolUse firewall fails **closed** when the gate is present but unrunnable/crashes (only a genuinely *absent* gate fails open there), while the Python `sentinel-gate.py` fails **open** on its own internal errors — a rare gate exception lets the action through rather than blocking the user's work. ecodex keeps this fail-open default deliberately: the gate is reliable and its `try/except` is defence-in-depth for unknown-unknowns, so blocking a tool call on a gate glitch would restrict workflow for no real gain. Set `EMPIRICA_SENTINEL_FAIL_CLOSED=1` for hardened deployments that prefer a noisy block. (Making the rare fail-open path emit a visible-but-calm notice — informative without blocking — is a tracked empirica follow-up.)
- A determined human collaborator can remove the `requirements.toml` file or switch to vanilla codex if they want to opt the AI out of discipline — the escape hatch is at install/uninstall time, not at AI-runtime

The point isn't to imprison anyone. It's to make the AI's training environment **structurally consistent** so the Brier-score calibration loop has clean data to learn from. An AI that's sometimes-disciplined-sometimes-not produces noisy calibration; an AI that's always-disciplined produces a clean improvement curve.

## Risks + tradeoffs

| Risk | Mitigation |
|---|---|
| **B doesn't apply on platforms without `SystemRequirementsToml` support** | Confirm cross-platform behavior during T15-implementation transaction. If only macOS/MDM-supported, fall back to C earlier than planned. |
| **B can be circumvented by renaming/deleting the `/etc/codex/requirements.toml` file** | Document this honestly. Adversarial users can always circumvent; the goal is to protect the default user from accidentally disabling, not to imprison adversaries. |
| **E (strict defaults) increases friction for casual users** | Provide a `--permissive` flag or env var (`ECODEX_PERMISSIVE=1`) that opts back into laxer defaults. Surfaces the choice; doesn't hide it. |
| **C (refuse-to-start) breaks if empirica is unhealthy** | Fail-open at startup if empirica subprocess is reachable but returns errors; only fail-closed if subprocess can't even spawn. Matches the plugin's own fail-open semantics from T7. |
| **D (core embed) was already rejected** | Re-document why if the question recurs. |

## Sign-off questions — answered after the user-model reframe (2026-05-02)

1. **Direction:** ✅ **A + B + E for v1 confirmed by David.** C deferred to v1.1 if AI-runtime circumvention shows up in telemetry.
2. **Permissive escape hatch:** ❌ **No `--permissive` / `ECODEX_PERMISSIVE=1` runtime flag.** With the AI as user, a runtime "permissive" flag is exactly what we don't want — it's a way for the AI to opt out of its own calibration training. The escape hatch is at *install time* (use vanilla codex), not at runtime. Humans who want laxer behavior install vanilla codex; ecodex is opinionated.
3. **Lock-file location (resolved):** The shipped file is **`requirements.toml`** (template [`ecodex/requirements.toml.example`](../../../ecodex/requirements.toml.example)), installed **only** to `/etc/codex/requirements.toml` by `--system` installs — codex hardcodes that as the sole managed-config path on Unix. Per-user (`--user`) installs get **no lock** (there is no `~/.ecodex/` or per-user managed-config path). Strict *behavior* is still on for per-user installs because the `ecodex` binary sets the `EMPIRICA_SENTINEL_*` env-var defaults regardless of scope; only the structural enabled-lock is system-only.
4. **Marketing posture:** **Hard — "ecodex IS the AI's calibration training environment."** With the AI as user, this is the honest framing. The differentiator vs vanilla codex isn't "discipline as feature," it's "your AI gets demonstrably better at knowing what it knows over time, measured by Brier score." Smaller TAM is fine — the audience that wants this is the audience that values measurable AI trustworthiness over raw speed.

## Implementation status (T17–T18, 2026-05-02)

Config artifacts and install/uninstall flow shipped:

| Artifact | Layer | Installs to | Purpose |
|---|---|---|---|
| [`ecodex/requirements.toml.example`](../../../ecodex/requirements.toml.example) | B | `/etc/codex/requirements.toml` (`--system` only) | Pins `plugins."empirica@nubaeon".enabled = true` so AI runtime can't disable |
| [`codex-rs/arg0/src/lib.rs`](../../../codex-rs/arg0/src/lib.rs) `apply_ecodex_strict_defaults()` | E | runs at every binary startup, all install paths | Defaults the `EMPIRICA_SENTINEL_*` env vars to `true` (`${VAR:-true}`) — the primary strict-mode mechanism |
| [`ecodex/config.toml.default`](../../../ecodex/config.toml.default) | A | `~/.codex/config.toml` (first run only) | Bundled defaults — empirica enabled, curated providers (DeepSeek default), strict-mode env vars documented |
| [`ecodex/scripts/install.sh`](../../../ecodex/scripts/install.sh) | A+B | runs at install | Drops `requirements.toml` (`--system`), wrapper, and binary; preserves existing user config |
| [`ecodex/scripts/ecodex-wrapper.sh`](../../../ecodex/scripts/ecodex-wrapper.sh) | E (legacy) | runs at every invocation (source-build path) | Historically exported `EMPIRICA_SENTINEL_*`; now the binary sets these on all paths. The wrapper still handles cortex mesh-auth, but is no longer the sole/required strict-mode mechanism |
| [`ecodex/scripts/uninstall.sh`](../../../ecodex/scripts/uninstall.sh) | (cleanup) | runs at uninstall | Removes `requirements.toml` (system) + wrapper + binary; preserves user config (unless `--purge`) |

### Install flow

```sh
# Per-user install (no sudo)
cd <ecodex-source>
(cd codex-rs && cargo build --release -p codex-cli)   # if not yet built
./ecodex/scripts/install.sh                           # default --user
```

This drops:
- `~/.local/lib/ecodex/bin/ecodex` (the real Rust binary — sets the strict-mode env defaults itself at startup)
- `~/.local/bin/ecodex` (wrapper that exec's the real one; also handles cortex mesh-auth)
- `~/.codex/config.toml` (only if absent — won't clobber existing user config)
- **No lock file** on `--user` installs — codex only honors `/etc/codex/requirements.toml`, which requires `--system`.

### System-wide install

```sh
sudo ./ecodex/scripts/install.sh --system
```

Same artifacts but to `/usr/local/{bin,lib}/`, plus the lock at `/etc/codex/requirements.toml`.

### Uninstall

```sh
./ecodex/scripts/uninstall.sh           # removes ecodex; preserves ~/.codex/config.toml
./ecodex/scripts/uninstall.sh --purge   # also removes ~/.codex/config.toml (with backup)
```

Removing the `/etc/codex/requirements.toml` lock (system installs) unlocks the `plugins."empirica@nubaeon".enabled` config key so the user can choose to disable the plugin (or switch to vanilla codex entirely).

### Remaining

- (If C escalation needed) `ensure_empirica_present()` in `cli/src/main.rs` startup — fail-fast if empirica plugin is missing/unresponsive at startup
- macOS install variant (paths differ — Homebrew conventions, `~/Library/Application Support/ecodex/`?)
- Windows install variant (much different — installer convention TBD)
- Live integration smoke test against a real codex/ecodex run (T19 candidate)
- npm/cargo distribution wrappers (T20 candidate, if we publish)

Setting the strict-mode env defaults in ecodex's own `arg0` entrypoint (rather than editing codex's config schema to add empirica-specific keys) keeps the discipline behavior in ecodex-owned code and out of the upstream config surface — supports the fork-and-PR-back-upstream posture, and makes strict mode default-ON on every install path without depending on an install-time wrapper.
