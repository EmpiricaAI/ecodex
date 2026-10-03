# Discipline Strengthening: Wiring Empirica Deeper Into ecodex

**Status:** A and E are shipped. B was shipped and turned out to do nothing, so it
was withdrawn; a real lock is planned. C is held in reserve; D is rejected.
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

This recasts everything below. Making the discipline hard to switch off is not
enterprise IT stopping employees from disabling security; it is training wheels
the AI cannot unscrew from itself.

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
| **B. System requirements lock** | a `requirements.toml` that keeps `plugins."empirica@empiricaAI".enabled = true` | none as built: codex ignores that key (see below) | a requirements field codex enforces | yes, small |
| **C. Refuse to start without empirica** | `cli/src/main.rs` checks at startup that the plugin is loaded and responsive and fails fast otherwise | strongest in-process | medium | yes |
| **D. Embed empirica into codex-core** | move the logic out of the plugin layer into core or a sidecar, so it is not removable | maximum | large | yes, significant |
| **E. Strict defaults** | the `ecodex` binary turns on empirica's strict-mode settings at startup on every install path | composes with A–D; tightens behavior while the plugin is on | small | no |

## What codex's managed requirements can and cannot do

codex has machinery for settings a user cannot change. `RequirementSource` in
`codex-rs/config/src/config_requirements.rs` covers macOS MDM managed
preferences, enterprise-managed requirements, and `SystemRequirementsToml`, a
file at `/etc/codex/requirements.toml` on Unix
(`%ProgramData%\OpenAI\Codex\requirements.toml` on Windows).

For plugins, though, a requirement only constrains which MCP servers a plugin may
run (`PluginMcpRequirements` in `codex-rs/protocol/src/mcp_policy.rs` has a
single field, `mcp_servers`). There is no requirement that keeps a plugin
enabled, and codex does not reject keys it does not know, so a file setting
`enabled = true` under `[plugins."empirica@empiricaAI"]` loads without error and
changes nothing.

## Decision: A + E, with a real B to come

David confirmed A + B + E on 2026-05-02. B was built as a file and found on
2026-10-02 to be ignored by codex. David then chose to withdraw it and plan a real
lock. How each layer stands now:

**A — bundle and enable.** The plugin's assets are embedded in the `ecodex`
binary. Every interactive, `exec`, resume or fork start writes them to
`~/.codex/plugins/cache/empiricaAI/empirica/<version>/` when they are missing or
out of date, and adds `[plugins."empirica@empiricaAI"] enabled = true` to
`config.toml` when the config has no entry for the plugin. A user who sets
`enabled = false` is respected, which is exactly why A alone is the weakest layer.
This holds on every install channel (install script, Homebrew, tarball,
`cargo install`, source build).

**B — the lock, withdrawn.** The source installer's `--system` mode used to copy a
`requirements.toml` with `enabled = true` to `/etc/codex/requirements.toml` and
describe it as a lock. Because codex ignores the key, the plugin could always be
disabled. The installer no longer writes the file, and `uninstall.sh --system`
removes a copy an older install left. A real lock needs a requirement field that
codex's plugin loading obeys. That is a small fork change to the config and plugin
crates, planned but not built.

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
keys. codex hands its hooks the process environment, so the gate sees them; a
probe that dumped the hook's environment confirmed all four. Because the binary
sets them, strict mode is on for every install path, not only for builds behind
the source installer's wrapper. Keeping this in ecodex's own entrypoint, rather
than adding empirica keys to codex's config schema, keeps the discipline in
ecodex-owned code and out of the upstream config surface.

Together: empirica is on by default, and on tight, without forking codex-core. For
the AI, the calibration loop runs on every transaction. For the human
collaborator, the AI they observe works inside the discipline unless someone turns
it off in config, and today nothing stops that config change, whoever makes it.

## How the layers behave

| Layer | Delivered by | Effect on the AI |
|---|---|---|
| A — install | the `ecodex` binary, at startup | the discipline is there on first run |
| E — defaults | the `ecodex` binary, at startup | strict mode is on by default on every install path |

- A new ecodex install starts with the AI inside the discipline.
- `enabled = false` in `config.toml`, or `-c plugins.empirica@empiricaAI.enabled=false`
  for one run, turns the plugin off on any install. That is the documented way to
  run unattended jobs ([`INSTALL.md`](../INSTALL.md#unattended-runs)), and also
  the gap the planned lock is meant to close for the AI.
- Failure handling has **two layers**. The Rust PreToolUse firewall
  (`codex-empirica-plugin/src/hooks/pre_tool_use.rs`) fails **closed** when the
  gate is installed but crashes or cannot be started; only a gate that is not
  installed at all fails open there. The Python `sentinel-gate.py` fails **open**
  on its own internal errors, so a rare gate exception lets the action through
  rather than blocking the user's work. That default is deliberate: the gate is
  reliable, and its `try/except` is defence in depth for unknown unknowns. Set
  `EMPIRICA_SENTINEL_FAIL_CLOSED=1` for hardened deployments that prefer a noisy
  block. The binary deliberately does not set it.
- The Sentinel applies only inside a git repository or an empirica project: that
  is what tells it what to measure. Outside one it lets commands through.

The point isn't to imprison anyone. It's to make the AI's training environment
**structurally consistent**, so the calibration loop has clean data. An AI that is
sometimes disciplined produces noisy calibration; one that always is produces a
clean improvement curve.

## Held in reserve: C (refuse to start)

If circumvention shows up, for instance an AI writing `enabled = false` into its
own config, escalate to the real lock or to **C**: a startup check in
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

1. **Direction:** A + E now, and a real B planned. C held in reserve for evidence
   of circumvention.
2. **No permissive runtime flag inside the session.** With the AI as the user, a
   switch the AI could flip mid-session is exactly a way to opt out of its own
   calibration training. Opting out is the launching human's act: `enabled = false`
   in config, or the `-c` override for one run. Individual strict settings can
   still be set to `false` in the environment by the human who runs ecodex.
3. **Lock location:** none today. The withdrawn B was system-wide only, because
   codex reads requirements from one system path and has no per-user one; a real
   lock would inherit that limit unless the field also lives somewhere per-user.
   Strict behavior (E) applies to every install regardless.
4. **Positioning:** ecodex *is* the AI's calibration training environment. The
   difference from vanilla codex is not "discipline as a feature" but "your AI
   measurably gets better at knowing what it knows."

## Risks

| Risk | Mitigation |
|---|---|
| Nothing stops `enabled = false`, including one the AI writes | Stated plainly here and in the install docs. A real lock is planned; C is the escalation. |
| Strict defaults add friction | Each setting can be set to `false` in the environment; no blanket permissive mode. |
| C would break startup if empirica is unhealthy | If C ships: fail open when empirica runs but errors, fail closed only when it cannot start at all. |

## Where it lives

| Piece | Layer | Purpose |
|---|---|---|
| `codex-rs/codex-empirica-plugin/src/provision.rs`, called from `codex-rs/cli/src/main.rs` | A | writes the plugin and enables it at startup |
| [`codex-rs/codex-empirica-plugin/assets/config/config.toml.default`](../../../codex-rs/codex-empirica-plugin/assets/config/config.toml.default) | A | the config ecodex writes when none exists |
| `codex-rs/arg0/src/lib.rs` `apply_ecodex_strict_defaults` | E | strict mode on every install path |
| [`ecodex/scripts/ecodex-wrapper.sh`](../../../ecodex/scripts/ecodex-wrapper.sh) | (source builds) | also exports the strict settings, now redundant with E, and exports `CORTEX_API_KEY` for the Cortex MCP static-key fallback |

Still open: the real lock, and the C escalation, which stays unbuilt until there
is evidence it is needed.
