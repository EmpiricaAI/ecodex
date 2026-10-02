# Discipline Strengthening: Wiring Empirica Deeper Into ecodex

**Status:** decided and shipped (A + B + E). C is held in reserve; D is rejected.
**Origin:** "if you can actually wire in the discipline more strongly into ecodex
that would be beneficial" (David, 2026-05-02).

## User model (foundational)

In ecodex (and Empirica generally), **the AI agent is the user being
disciplined**. Humans are guidance and observation collaborators — they install
ecodex, watch the AI work, course-correct, but they are not the disciplined party.

The discipline serves two purposes:

1. **Compliance** — the AI follows the workflow (PREFLIGHT/CHECK/POSTFLIGHT,
   artifact logging, transaction discipline).
2. **Calibration training** — every transaction with a measured prediction and
   outcome is a calibration data point. Better calibration means better
   self-knowledge: **the AI becomes more trustworthy over time by going through
   the discipline.**

This recasts everything below. The "lock" isn't enterprise IT preventing
employees from disabling security — it's training wheels the AI can't unscrew
from itself.

## The question

On its own, ecodex is codex with the empirica plugin enabled. Anything enabled in
config can be disabled in config: an AI (or its human on its behalf) can set
`plugins."empirica@empiricaAI".enabled = false`, and the discipline — *and the
calibration loop* — disappears.

Should ecodex make the discipline harder to disable from inside the AI's runtime,
deeper in the stack, or both?

## Five strengthening axes

| Option | Mechanism | Strength | Code change | Forks codex? |
|---|---|---|---|---|
| **A. Bundle and enable** | ecodex installs the plugin and enables it by default | weakest — config can switch it off | small | no |
| **B. System requirements lock** | a `requirements.toml` pins `plugins."empirica@empiricaAI".enabled = true` through codex's existing managed-requirements support | strong — user-config writes to that key are rejected | none (a file) | no |
| **C. Refuse to start without empirica** | `cli/src/main.rs` checks at startup that the plugin is loaded and responsive and fails fast otherwise | strongest in-process | medium | yes |
| **D. Embed empirica into codex-core** | move the logic out of the plugin layer into core or a sidecar, so it is not removable | maximum | large | yes, significant |
| **E. Strict defaults** | the `ecodex` binary turns on empirica's strict-mode settings at startup on every install path | composes with A–D; tightens behavior while the plugin is on | small | no |

## Codex's existing enforcement

codex already has machinery for config keys a user cannot change.
`RequirementSource` in `codex-rs/config/src/config_requirements.rs` covers macOS
MDM managed preferences, enterprise-managed requirements, and
`SystemRequirementsToml` — a file at `/etc/codex/requirements.toml` on Unix
(`%ProgramData%\OpenAI\Codex\requirements.toml` on Windows) whose keys override
user config. **ecodex uses that file to pin the plugin on** without modifying any
codex source.

## Decision: A + B + E

Confirmed by David on 2026-05-02. How each layer is delivered now:

**A — bundle and enable.** The plugin's assets are embedded in the `ecodex`
binary. Every interactive, `exec`, resume or fork start writes them to
`~/.codex/plugins/cache/empiricaAI/empirica/<version>/` when they are missing or
out of date, and adds `[plugins."empirica@empiricaAI"] enabled = true` to
`config.toml`. A user who sets `enabled = false` is respected, which is exactly
why A alone is the weakest layer. This holds on every install channel (install
script, Homebrew, tarball, `cargo install`, source build).

**B — the lock.** [`ecodex/requirements.toml.example`](../../../ecodex/requirements.toml.example)
pins:

```toml
[plugins."empirica@empiricaAI"]
enabled = true
```

The source installer's `--system` mode installs it to
`/etc/codex/requirements.toml`; codex's managed-requirements layer then rejects
any attempt to override the key. **The lock is system-wide only.** codex has no
per-user requirements path, so per-user installs (and the prebuilt channels) ship
without it.

**E — strict defaults.** At startup, before anything else runs, the `ecodex`
binary sets these to `true` unless they are already set
(`apply_ecodex_strict_defaults` in `codex-rs/arg0/src/lib.rs`, after `.env` is
loaded, so a real environment variable or a `~/.codex/.env` entry still wins):

```sh
EMPIRICA_SENTINEL_REQUIRE_BOOTSTRAP=true    # project-bootstrap before any praxic action
EMPIRICA_SENTINEL_COMPACT_INVALIDATION=true # CHECKs are invalid after a context compaction
EMPIRICA_SENTINEL_CHECK_EXPIRY=true         # a CHECK expires after 30 minutes
EMPIRICA_CALIBRATION_FEEDBACK=true          # PREFLIGHT/CHECK show the calibration record
```

These are environment variables read by the vendored `sentinel-gate.py`, not TOML
keys. Because the binary sets them, strict mode is on for every install path, not
only for builds behind the source installer's wrapper. Keeping this in ecodex's
own entrypoint, rather than adding empirica keys to codex's config schema, keeps
the discipline in ecodex-owned code and out of the upstream config surface.

Together: empirica is on, and on tight, without forking codex-core. For the AI,
the calibration loop runs on every transaction. For the human collaborator, the
AI they observe is structurally constrained to do its work measurably, not just
performatively.

## How the layers behave

| Layer | Delivered by | Effect on the AI |
|---|---|---|
| A — install | the `ecodex` binary, at startup | the discipline is there on first run |
| B — lock | `/etc/codex/requirements.toml` (system installs) | the AI cannot turn off its own training wheels at runtime |
| E — defaults | the `ecodex` binary, at startup | strict mode is on by default on every install path |

- A new ecodex install starts with the AI inside the discipline.
- On a system install, the AI's attempts to set
  `plugins."empirica@empiricaAI".enabled = false` are rejected.
- Failure handling has **two layers**. The Rust PreToolUse firewall fails
  **closed** when the gate is present but cannot run (only a genuinely *absent*
  gate fails open there). The Python `sentinel-gate.py` fails **open** on its own
  internal errors, so a rare gate exception lets the action through rather than
  blocking the user's work. That default is deliberate: the gate is reliable, and
  its `try/except` is defence in depth for unknown unknowns. Set
  `EMPIRICA_SENTINEL_FAIL_CLOSED=1` for hardened deployments that prefer a noisy
  block. The binary deliberately does not set it.
- The escape hatch is at install time, not at AI runtime: a human can remove
  `/etc/codex/requirements.toml` or switch to vanilla codex to opt the AI out.

The point isn't to imprison anyone. It's to make the AI's training environment
**structurally consistent**, so the calibration loop has clean data. An AI that is
sometimes disciplined produces noisy calibration; one that always is produces a
clean improvement curve.

## Held in reserve: C (refuse to start)

If AI-runtime circumvention shows up (the lock file renamed, systems without
managed-requirements support), escalate to **C**: a startup check in
`cli/src/main.rs` that the plugin is loaded and `empirica` responds, failing fast
with "ecodex requires the empirica plugin; reinstall, or use upstream codex if you
don't want it." It is a fork-source change, but small, and could go upstream as an
opt-in feature for any codex distribution that wants to require a plugin.

## Rejected: D (core embed)

- Largest divergence from upstream; breaks the fork-and-PR-back posture.
- Empirica's logic is Python; embedding it in Rust core means PyO3 or IPC
  complexity, deliberately deferred.
- The plugin layer is the architectural seam codex designed for this kind of
  extension.
- Locking empirica into core makes it harder to evolve independently.

Reconsider D only if the plugin layer's cost comes to outweigh its decoupling
benefit.

## Decisions on the open questions

1. **Direction:** A + B + E. C held in reserve for evidence of circumvention.
2. **No permissive runtime flag.** With the AI as the user, a runtime "permissive"
   switch is exactly a way for the AI to opt out of its own calibration training.
   The escape hatch is at install time. Individual strict settings can still be
   set to `false` in the environment by the human who runs ecodex.
3. **Lock location:** `requirements.toml` at the system path only; per-user
   installs get no lock, but strict behavior (E) applies to them all the same.
4. **Positioning:** ecodex *is* the AI's calibration training environment. The
   difference from vanilla codex is not "discipline as a feature" but "your AI
   measurably gets better at knowing what it knows."

## Risks

| Risk | Mitigation |
|---|---|
| The lock does nothing on per-user and prebuilt installs | Stated plainly; A and E still apply. An upstream per-user requirements path would close it. |
| The lock can be circumvented by deleting `/etc/codex/requirements.toml` | Documented. The goal is to protect the default user from disabling it by accident, not to imprison an adversary. |
| Strict defaults add friction | Each setting can be set to `false` in the environment; no blanket permissive mode. |
| C would break startup if empirica is unhealthy | If C ships: fail open when empirica runs but errors, fail closed only when it cannot start at all. |

## Where it lives

| Piece | Layer | Purpose |
|---|---|---|
| `codex-rs/codex-empirica-plugin/src/provision.rs`, called from `codex-rs/cli/src/main.rs` | A | writes the plugin and enables it at startup |
| [`codex-rs/codex-empirica-plugin/assets/config/config.toml.default`](../../../codex-rs/codex-empirica-plugin/assets/config/config.toml.default) | A | the config ecodex writes when none exists |
| [`ecodex/requirements.toml.example`](../../../ecodex/requirements.toml.example) | B | the lock template |
| [`ecodex/scripts/install.sh`](../../../ecodex/scripts/install.sh) `--system` / [`uninstall.sh`](../../../ecodex/scripts/uninstall.sh) | B | install and remove the lock on source builds |
| `codex-rs/arg0/src/lib.rs` `apply_ecodex_strict_defaults` | E | strict mode on every install path |
| [`ecodex/scripts/ecodex-wrapper.sh`](../../../ecodex/scripts/ecodex-wrapper.sh) | (source builds) | also exports the strict settings, now redundant with E, and passes the cortex key for mesh installs |

Still open: a per-user lock needs an upstream change, and the C escalation is
unbuilt until there is evidence it is needed.
