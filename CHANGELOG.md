# Changelog

All notable ecodex-specific changes are documented here. ecodex is a fork of [openai/codex](https://github.com/openai/codex); upstream codex changes are tracked at the [openai/codex releases](https://github.com/openai/codex/releases) page.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and version numbers follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

`scripts/release.sh` rolls the [Unreleased] section under a new version stamp on each release. Add entries to [Unreleased] as you ship; the release script promotes them.

## [Unreleased]

## [0.152.0] - 2026-09-01

### Changed
- **Upstream sync to rust-v0.152.0** (456 commits from openai/codex, one
  hop from 0.149.0; 29 conflicted files resolved by code read). Upstream
  highlights ecodex users see: new `Interrupt` hook event, restructured
  per-step settings (`StepSettings`), code-mode startup prewarm, richer
  mailbox turn-start options.
- **Hook surface now 14 events**: upstream's new `Interrupt` joins
  ecodex's `TaskCompleted` / `PostToolUseFailure` across the engine,
  schemas, TUI hooks browser, and analytics labels.
- **`ecodex --version` identifies as `ecodex`** (was `codex-cli`, the
  crate name); homebrew formula test and installer fakes updated in
  lockstep.

### Fixed
- **Provider tool-filter re-woven** into upstream's rebuilt
  responses-lite request block — upstream's version shipped unfiltered,
  which would have re-broken local llama.cpp/vLLM providers that hard-400
  on OpenAI built-in tool types.
- Preserved ecodex behaviors re-woven onto restructured upstream paths:
  T78 provider hot-swap (single commit path via `update_settings_if`;
  the turn-context duplicate swap removed), mesh wake on the new
  mailbox `TurnStartOptions` API, silent recording of hook-injected
  additionalContext, SessionEnd-clamp warning suppression for the
  bundled plugin, `monitor` tool on the new `ToolOutput` trait.
- Removed the dormant `Op::UserInputWithTurnContext` protocol variant
  (zero consumers; its own comment marked it pending cleanup).
- `Cargo.lock` regenerated with security floors re-pinned (third sync
  running the wholesale-lock regression appeared): quinn-proto 0.11.16,
  openssl 0.10.81, serde_with 3.21.0; gix kept at 0.83.
- Vendored `session-monitor-arm.py` re-synced from empirica@develop.

## [0.149.0] - 2026-08-25

### Changed
- **Upstream sync to rust-v0.149.0** (622 commits from openai/codex,
  one hop from 0.147.0; 37 conflicted files resolved by code read).
  Notable upstream changes ecodex users see: native mid-turn steering
  (`Op::TurnInput` / `RecoverTurn`), consolidated hooks engine, the
  `psp` flag migrated into the features system.
- **Steering machinery deduplicated.** Upstream 0.149 ships turn
  steering natively; ecodex's earlier steering-admission chain
  (`user_input_or_turn`, `UserMessageAdmission`, `SteerInputError`)
  was fully superseded and removed. The ecodex mesh-wake spine
  (Monitor / ntfy doorbell `inject_response_items`) is preserved,
  remapped onto upstream's new mailbox API.
- **Pinned-skill BODY auto-injection removed** (David-ratified). The
  `pinned: true` frontmatter flag survives as FRAMEWORK-skill metadata
  and the skills catalog prompt now instructs the model to proactively
  Read framework SKILL.md files (and re-Read after `/compact`);
  guaranteed-ambient framework content routes through the plugin's
  AGENTS.md channel instead of per-session injection. Rationale:
  measured pinned bodies at 40.5% of the empirica frame — the main
  driver of small-context-model failures.
- **Vendored empirica hooks re-synced** (sentinel-gate, tool-router):
  the Sentinel recovery escape now uses the stricter whole-command
  predicate (`is_safe_empirica_statement`), closing a chained-statement
  bypass where a safe leading verb could smuggle a second mutating
  statement past the gate.

### Fixed
- Preserved ecodex behaviors re-woven onto restructured upstream code
  paths: T78 provider hot-swap (`model_provider` thread-settings
  override + ArcSwap ModelClient), silent recording of hook-injected
  additionalContext (model reads it, screen stays clean — upstream's
  new envelope path would have rendered it), TUI plugin statusline
  sources, and MCP server-scope approvals (ported onto upstream's
  `ReviewDecision` rename).
- `Cargo.lock` regenerated with security floors re-pinned:
  quinn-proto 0.11.16, openssl 0.10.81, serde_with 3.21.0.

## [0.147.2] - 2026-08-18

### Added
- **Pre-sandbox practice bootstrap.** A fresh workspace (no `.git`, no
  `.empirica`) now becomes a working empirica practice at session start:
  the plugin's SessionStart hook (harness-side, before any sandboxed
  command) runs `git init` + `empirica project-init` — idempotent,
  fail-open, with refusal guards (payload-vs-process cwd mismatch,
  nested-under-ancestor-repo, `$HOME`, filesystem root) and
  `EMPIRICA_AI_ID` persisted into the fresh `project.yaml`.
- **CLI-parity CI guard.** A vendored-hooks test source-discovers every
  `empirica` subprocess argv in the vendored Python hooks and Rust core
  and checks each subcommand + flag against the real installed empirica
  CLI parser, catching wrapper/CLI drift before it becomes a silent
  capability loss.

### Changed
- **Re-vendored the empirica hook layer to develop @1.13.25-era.**
  Sentinel firewall over-gating fixes (arithmetic expansion no longer
  misparsed as command substitution; quote-aware heredoc detection;
  progressive chain splitting; `noetic-batch` always-open — no praxic
  gating weakened, verified by invariant guard + behavioral controls);
  post-compact relevance now driven by the latest PREFLIGHT
  `task_context`; optional statusline model tag. Vendored onboarding
  prose corrected for ecodex's self-provisioning model (empirica's
  `setup-claude-code` deliberately refuses ecodex).
- **Chat-translator providers no longer receive uncallable namespace
  tools**: `namespace_tools` capability now derives from
  `supports_openai_builtin_tools`, so function-only providers get
  namespace specs dropped instead of shipped broken.

### Fixed
- **`writable_git` now works in linked git worktrees.** With
  `[sandbox_workspace_write] writable_git = true`, the write grant only
  covered `<project_root>/.git` — in a linked worktree that is a one-line
  gitdir pointer file, so every `git add`/`commit` failed against the real
  git state under `<main-repo>/.git` (read-only filesystem on `index.lock`).
  Write entries targeting a `.git` pointer file now expand to the resolved
  per-worktree gitdir and the common git dir, with the escalation-sensitive
  shared entries (`HEAD`, `config`, `hooks`, `info`, `modules`, sibling
  `worktrees`) pinned read-only — a sandboxed worktree session can commit,
  but cannot plant hooks/config that would execute unsandboxed in the main
  checkout, nor touch sibling worktrees' state. Submodule gitdir pointers
  get their self-contained gitdir. Verified end-to-end under the real
  bubblewrap sandbox. `writable_git = false` behavior is unchanged.

## [0.147.1] - 2026-08-13

### Fixed
- **Critical: `codex-code-mode-host` binary now shipped.** The `0.147.0`
  upstream sync made `code_mode_host` a `Stage::Stable`,
  `default_enabled: true` feature — and any model whose `models.json`
  entry declares `tool_mode: code_mode_only` (currently `gpt-5.6-sol`,
  `gpt-5.6-terra`, `gpt-5.6-luna`) has no fallback path: `effective_tool_mode`
  only degrades `CodeMode` to `Direct` on a missing host, not
  `CodeModeOnly`. `gpt-5.6-sol` is this repo's own default configured
  model, so this broke real usage, not just an edge case. `codex-cli`
  itself never needed the `v8`-backed `codex-code-mode-runtime` crate (only
  `codex-code-mode-host` does), but that binary was never added to
  `.github/workflows/release.yml`, `scripts/install.sh`, or the Homebrew
  formula — v0.147.0's published release therefore shipped without it,
  and any code-mode-only model failed every tool call with `failed to
  spawn code-mode host ... No such file or directory`. We don't build
  `codex-code-mode-host` ourselves: its `v8_enable_sandbox` feature has no
  prebuilt archive published by `rusty_v8` for any platform in at least
  its last 15 releases, so building it means compiling V8 from source
  (hours, `depot_tools`/`gn`/`ninja`). `codex-code-mode-host` is
  unmodified upstream code (verified zero ecodex commits touch
  `code-mode-host`/`-runtime`/`-protocol` since the `0.147.0` merge), and
  upstream's own `rust-v0.147.0` GitHub release already publishes the
  exact binary we need — `.github/workflows/release.yml` now fetches it
  from there per-target (`codex-rs/UPSTREAM_SYNC_TAG` records which
  upstream tag to pull from; bump it alongside future upstream syncs) and
  packages it as a fourth binary alongside
  `ecodex`/`codex-empirica-plugin`/`codex-empirica-translator`, installed
  by `scripts/install.sh` + the Homebrew formula. Verified fix end-to-end:
  `gpt-5.6-sol` exec succeeds with the fetched binary in place.

### Dependencies
- Post-0.147.0 Dependabot triage (49 alerts, Rust-ecosystem subset only —
  npm/pnpm alerts are upstream's own JS tooling lockfile, not reviewed
  here): 4 real, shipped-in-`codex-cli` advisories fixed by `cargo update`
  within existing compatible ranges — `quinn-proto` 0.11.14 → 0.11.16
  (RUSTSEC-2026-0185, re-introduced by taking upstream's Cargo.lock
  wholesale during the 0.147.0 merge; was already fixed pre-sync),
  `webbrowser` 1.0.6 → 1.2.2 (RUSTSEC-2026-0257, argument injection —
  reachable via `codex-login`'s OAuth browser-open flow), `openssl`
  0.10.75 → 0.10.81 (8 GHSA advisories across AES key-wrap, buffer
  bounds, and callback-length checks), `serde_with` 3.17.0 → 3.21.0
  (KeyValueMap panic on empty sequence/map). Post-fix `cargo audit`:
  0 vulnerabilities, 6 pre-existing `unsound`/`yanked` warnings (no CVE).
  Two advisories remain genuinely blocked, both already tracked as their
  own goals: `hickory-proto` (capped by `rama-dns`'s own `^0.25` pin, a
  transitive dep we don't control) and `opentelemetry_sdk` (workspace
  0.31→0.32 bump blocked on `tracing-opentelemetry` not yet targeting
  `opentelemetry` 0.32 in any release).

## [0.147.0] - 2026-08-12

### Changed
- **Upstream base sync `0.146` → `0.147`.** Merged upstream `rust-v0.147.0`
  (1530 files touched). Notable composite areas: skill-catalog rendering
  moved from `core-skills` into a new `ext/skills` extension crate (ported
  ecodex's `pinned: true` framework-skill lifecycle text + frontmatter
  parsing to the new location); provider representation changed from a
  plain `ModelProviderInfo` struct to a `SharedModelProvider` trait object
  (adapted the T78 provider hot-swap feature accordingly); a
  `defer_loading`/`namespace_tools`-aware responses-lite tool encoding path
  was added upstream (composed with ecodex's `filter_tools_for_provider`).

### Fixed
- **`install.sh` broken pipe under `set -o pipefail`.** Version-resolution
  (`curl ... | grep -m1 ... | sed ...`) broke the documented one-liner
  installer for every fresh user, not just version-drift recovery — `grep -m1`
  closes its end of the pipe as soon as it matches, while curl is often still
  writing, and curl's resulting "Failure writing output" exit propagated
  through `pipefail`/`set -e` to kill the script. Fixed by capturing curl's
  output into a variable first.

### Added
- **`scripts/release.sh --verify-install`** — an opt-in release-gate step
  that runs `install.sh` against a scratch prefix pointed at the just-cut
  release and checks the installed binary reports the right version. Catches
  the bug above (and future ones) before it reaches a user's machine.

## [0.146.0] - 2026-08-02

Upstream base sync `0.145` → `0.146` (127 upstream commits). Upstream codex is
alpha-only above `0.137`, so ecodex ships a clean `0.146.0` on the pinned
`0.146.0-alpha.10.1` base.

### Added
- **Root [`ARCHITECTURE.md`](ARCHITECTURE.md)** — a top-level architecture map
  (fork boundary, the three moving parts, the harness/enforcement layer, the
  de-Claude pipeline, known tensions), in the shape of Empirica's.

### Changed
- **First-launch welcome is now a static Empirica mark** by default instead of an
  animation; the koru-spiral (and upstream variants) remain available on `Ctrl+.`.
- **Re-vendored the Empirica plugin to 1.13.0.** Notable: `sentinel-gate` gains a
  read-only-by-naming-convention rule that stops the firewall over-gating read
  verbs (41/279 CLI verbs were being denied pre-CHECK), plus additional
  credential redaction (Authorization headers, `token <hex>`, `user:token@host`
  git URLs).

### Fixed
- **Dead-end noise.** The `tool-failure` hook now filters operational noise
  (timeouts, SIGTERM/SIGKILL, connection-refused, DNS) and success-markers before
  recording a dead-end, and redacts credentials — previously any tool failure
  ≥20 chars became a permanent "avoid re-trying" dead-end. Fix re-vendored from
  Empirica (the vendored copy needed the re-vendor; a package upgrade doesn't
  reach it).
- **Amazon Bedrock config.** `supports_openai_builtin_tools` had a serde default
  of `true` but a derived `Default` of `false`, so a bedrock provider from a
  config that omitted it failed the "only `base_url`/`auth`/`http_headers`/`aws`
  may change" validation. Normalized before the check.
- **`exec` provider routing.** OpenAI-family models (`gpt-*`/`chatgpt-*`/`o1|o3|o4`)
  passed with headless `-m` now route to the `openai` provider even when the
  persisted default points elsewhere — parity with the interactive `/model`
  switch (so gpt-5.6 works over ChatGPT-subscription auth in `exec`).

### Removed
- Dead `SessionServices.environment_manager` field (write-only; superseded by
  `turn_environments.environment_manager()` upstream).

### Dependencies
- Dependabot triage (55 alerts): the shipped Rust binary has ~0 actionable live
  vulnerabilities — 41 npm/pip alerts are docs/tooling deps (not in the binary),
  and the Rust dependabot alerts are not corroborated by `cargo-audit` for the
  pinned versions. The sole live `cargo-audit` advisory (`quinn-proto`) is not in
  the built dependency graph.
- Merged Dependabot **#14** (`quinn-proto` 0.11.14 → 0.11.16, clears the phantom
  `RUSTSEC-2026-0185` advisory from `Cargo.lock`) and **#15**
  (`datamodel-code-generator` dev-dep in `/sdk/python`). Post-merge `cargo-audit`
  reports **0 vulnerabilities**.

### Documentation
- New [`docs/ecodex/integrations/model-notes.md`](docs/ecodex/integrations/model-notes.md)
  — cross-model wiring/verdict notes (the deciding axis is auth: OAuth-subscription
  vs API-key; OpenAI subs work via OAuth, Mistral is API-key-only). Records our
  Devstral reliance (best non-OpenAI model in ecodex-lab) and the OpenRouter
  caching caveat (unreliable/expensive → prefer provider-direct).
- Refreshed `MISTRAL_SOVEREIGN.md` (Le Chat subs ≠ API access; current Devstral 2
  / Small 2 slugs). Drift fixes: fake `devstral-2-latest` → `devstral-latest`
  (config default + curated registry), broken `empirica mcp` → `empirica-mcp`
  ref, stale `ECODEX_VERSION` install example → `v0.146.0`.

## [0.145.0] - 2026-07-24

### Changed
- **Version scheme now tracks the upstream codex base version.** ecodex jumps
  from `0.2.7` to `0.145.0` — the [openai/codex](https://github.com/openai/codex)
  release this build is derived from. This is *not* a leap in ecodex features;
  it aligns the version ecodex reports as its client version so OpenAI's backend
  (which gates models on the Codex client version) accepts it. Going forward,
  releases are `0.145.x` (ecodex patches on this base), then the new base
  (`0.146.x`, …) on each upstream re-sync. `ecodex --version` now reports
  `0.145.0`.

### Fixed
- **gpt-5.6 (and gpt-5.5 / gpt-5.4) now work on the OpenAI-direct path.** The
  ChatGPT-Codex backend was rejecting requests with `400 "requires a newer
  version of Codex"` because ecodex reported its fork version (`0.2.x`) as the
  client version — below OpenAI's per-model gate. The version-scheme change
  above makes ecodex report the codex base version, which clears the gate;
  live-verified over ChatGPT-subscription auth. Fixes #9.
- **Homebrew / install plumbing** (thanks **@FrancisFerrero**, #11):
  `sync-homebrew.sh` now runs on stock macOS bash 3.2 (was broken by a bash-4
  `declare -A`); `release.sh --publish-homebrew` generates the prebuilt formula
  via `sync-homebrew.sh` instead of a divergent source-build formula that
  dropped the plugin + translator binaries; and `install.sh`'s dependency
  preflight no longer short-circuits the `curl`/`tar`/`uname` checks on Linux.

### Dependencies
- Bump `gix` 0.81.0 → 0.83.0 (#10).

## [0.2.7] - 2026-07-24

### Fixed
- **gpt-5.6 (and all bare frontier OpenAI) models returned a 404 when selected
  from `/model`.** The picker changed only the model name, never
  `model_provider`, so an OpenAI-family preset selected while a custom provider
  (deepseek by default) was active was sent to the wrong endpoint. The provider
  now resolves on selection (`provider_for_model()`) and switches alongside the
  model — in-session hot-swap plus persisted — routing bare `gpt-5.x` / `o*` ids
  to the built-in `openai` provider. This wires the last mile of the existing
  T78 hot-swap pipeline (`provider_for_slug` was dead code).

### Added
- **Strict-mode discipline defaults now apply on every install path.** The
  `EMPIRICA_SENTINEL_*` calibration-loop defaults (bootstrap-before-praxic,
  compact-invalidation, 30-minute CHECK expiry, calibration feedback) were
  previously exported only by the source-build wrapper, so `curl`/Homebrew/
  `cargo install`/manual-binary users silently ran with strict mode OFF — the
  value-prop over stock codex absent on the paths people actually use. The
  `ecodex` binary now defaults them at startup (`arg0`, `${VAR:-true}` so a real
  env var or `.env` still wins). Non-blocking by design; the sentinel's
  crash-handling stays fail-open so a rare gate glitch never blocks work.
- **`[model_providers.openai]`** in the bundled default config — frontier models
  are a first-class `/model` option (open-weights stays the lead, not a lock-out).

### Documentation
- **Trajectory-wide docs/code alignment audit.** Rewrote `providers.md` around
  the translator (upstream removed `wire_api="chat"`; chat providers route via
  `:18080`); corrected `api/hooks.md` (live-hook status table + two-layer
  fail-closed firewall semantics + `EMPIRICA_HOOKS_DIR` resolution); refreshed
  `hook-events-roadmap.md` (11 upstream + 2 divergent events; `Feature::PluginHooks`
  removed); fixed `api/mcp.md` (`mcpServers`, `empirica-mcp`), `api/skills.md`
  (`diagnose`/`onboard` skills + the `pinned` field), `discipline-strengthening.md`
  (`requirements.toml`, arg0 mechanism, 30-min expiry), `epistemic-llms.md`
  (12 curated entries), and the README version stamp.

## [0.2.6] - 2026-07-22

### Added
- **gpt-5.6 model family** via a 146-commit upstream re-sync
  (`bed0c5e74c..upstream/main`). Our prior merge base predated gpt-5.6, so users
  selecting it hit "asking for a newer upstream binary"; ecodex now carries the
  `gpt-5.6-sol` / `gpt-5.6-terra` / `gpt-5.6-luna` / `gpt-5.6-pro` registry
  entries plus the protocol/model-info currency they depend on. 12 conflicts
  resolved preserving the ecodex L3 surface: `pinned` skills ported into the new
  upstream `codex-skills` crate (upstream extracted `SkillMetadata` out of
  `core-skills`), `writable_git` adapted to the new `FileSystemSandboxEntry`
  constructor, and the T78 `ArcSwap<ModelClient>` hot-swap realigned to the
  updated `ModelClient::new` signature.
- **Prebuilt-binary install pipeline** — non-devs can now install without a Rust
  toolchain or a 10–25 min compile:
  - `.github/workflows/release.yml` cross-builds stripped binaries for macOS
    (arm64/x64) and Linux (arm64/x64, glibc) on every `v*.*.*` tag and
    attaches per-target `.tar.gz` + `.sha256` to the release.
  - `scripts/install.sh` — `curl … | bash` one-liner that detects your platform,
    downloads the matching tarball, verifies its checksum, and installs to
    `~/.local/bin`.
  - The Homebrew formula now **downloads the prebuilt binary** instead of
    `cargo install` (`packaging/homebrew/ecodex.rb` + `scripts/sync-homebrew.sh`
    to fill per-target checksums from a release).

### Changed
- **Install docs are now prebuilt-first + honest** (README + `docs/ecodex/INSTALL.md`).
  The install script and Homebrew are marked "no compile"; the stale
  Linux-x86_64-only "Direct binary" table is corrected to the full macOS+Linux
  arm64/x64 matrix. The cargo + source-build paths are clearly labelled as the
  developer (compiles) paths.

### Fixed
- Silenced 3 cosmetic startup warnings (bundled-empirica `SessionEnd` timeout
  clamp now debug-logs; TUI update-check + Homebrew-cask URLs point at
  `EmpiricaAI/ecodex`, killing the false "update available" banner).

## [0.2.5] - 2026-07-20

### Added
- **Upstream codex sync 2026-07** (`bed0c5e74c`): 674-commit forward-port of
  openai/codex onto ecodex's plugin layer. Notable new upstream surface now
  carried: the `SessionEnd` hook event, `cloud` / `exec-server` /
  `remote-control` subcommands, `features` flag inspection, paginated
  thread-history legacy views, audio output for dynamic tools + code mode, the
  `use_responses_lite` request path, and UUIDv7/InputAudio protocol additions.
  All ecodex integrations re-reconciled against the new session semantics (the C
  tool-fix, T78 `ArcSwap<ModelClient>` hot-swap, pinned skills, native
  ntfy/monitor, curated L3 model registry, `writable_git`). The merge's biggest
  hazard — ecodex and upstream *independently* adding `SessionEnd` — was
  de-duplicated across the hook engine/schema/registry (upstream's form kept;
  ecodex-only `TaskCompleted` + `PostToolUseFailure` preserved).
- **Event-driven mesh wake→act loop.** A mesh-woken practitioner now polls its
  inbox and reacts autonomously instead of greeting/orienting. On a doorbell
  wake the native ntfy listener polls the inbox itself and inlines the actual
  messages into the wake notice — taking per-model instruction-obedience off the
  critical path — and the SessionStart hook leads with pending mesh messages
  rather than a greeting. Verified model-agnostic across GLM-5.2, Kimi-K2.6, and
  MiniMax-M2.7.
- **`writable_git` sandbox flag** (`[sandbox_workspace_write] writable_git`,
  default `false`). Lets trusted/autonomous projects `git commit` while the
  workspace-write sandbox stays on: it registers an explicit `.git` write rule
  that suppresses the default `.git` read-only protection. `.agents`/`.codex`
  remain read-only. Default-off preserves the existing protection everywhere.
- **Cargo target-dir disk guard** (`scripts/cargo-cache-guard.sh`) —
  threshold-gated, build-safe pruning of Rust build artifacts (never runs while
  a build is active; only removes regenerable artifacts). Wired into the
  post-build install step and a periodic cron.

### Fixed
- **Local / llama.cpp providers rejected every turn** with `'type' of tool must
  be 'function'` (Task C). ecodex sent its OpenAI-builtin tool schemas
  (`web_search`, `namespace`/mcp, `tool_search`) to providers that only accept
  plain function tools, so a fresh local model couldn't complete a single turn.
  Added `ModelProviderInfo.supports_openai_builtin_tools` (default `true`; `false`
  for OSS/llama.cpp providers) and `filter_tools_for_provider`, which drops the
  non-function `ToolSpec` variants for those providers before serialization.
  Function + freeform tools pass through untouched; order preserved. Verified
  end-to-end against the local lab (the rejection error cleared). (`b15060be30`)
- **The Sentinel gated the receive-side mesh CLI.** `empirica mailbox
  poll`/`show`/`reply`/`archive` were missing from the Sentinel's tiered
  whitelist, so a mesh-woken idle practitioner was denied *"No open transaction"*
  the moment it tried to check its inbox. Added (`poll`/`show` → Tier 1,
  `reply`/`archive` → Tier 2); converged with empirica canonical.
- **Hook `additionalContext` dumped to the terminal.** The EWM protocol block and
  other hook context rendered as a wall of developer-role text every
  session/turn. It is now injected silently (the model reads it from history; the
  screen stays clean). Gate/deny reasons stay visible.
- **Proposal IDs were truncated in the SessionStart inbox-lead** (`[:26]`), which
  broke `empirica mailbox reply --parent-id`. Full IDs are now shown.

### Changed
- **Re-vendored the empirica hooks to 1.12.28** (`31aa4731f4`, via
  `f0d96db758` for 1.12.27). `session-init.py` now carries empirica's
  split-brain project-persistence fix (PR#357): explicit `--project-id` pinning
  at session create, headless-gated `active_work.json` read, trajectory_path
  healers that tolerate both `<root>` and `<root>/.empirica` forms, and a loud
  `split_brain_corrected` signal instead of a silent heal. A de-Claude pass
  genericized the model-facing hook/skill prose flagged by `setup-codex.py`.
- Re-vendored the empirica hooks to canonical `@1cefa8df3`: SessionStart
  inbox-lead, arm-by-replacement monitor management, terse EPP pushback pointer,
  and the full `sentinel-gate.py`.

## [0.2.4] - 2026-07-02

### Fixed
- **The firewall was dark on every fresh install.** The plugin's `hooks.json`
  carried a top-level `_comment` doc field, but codex's `HooksFile` parser is
  `deny_unknown_fields` — so it rejected the whole file and **no hooks ran**
  (no Sentinel, no session init). v0.2.3 shipped with this. The field is
  removed; every hook loads.
- **`permissionDecision:allow` spam on every allowed tool.** codex's PreToolUse
  contract only accepts `deny` (with a reason); a bare `allow` is "unsupported"
  and fails open with a noisy `hook (failed)` line. The sentinel's allow path
  emitted it on every noetic tool call. The translate layer now **omits**
  `permissionDecision` when the gate allows, so codex proceeds cleanly.
- **A fresh model couldn't bootstrap its own session.** `session-init` emits the
  new session_id + a ready-to-fill PREFLIGHT template as
  `hookSpecificOutput.additionalContext`, but the SessionStart translator read
  only the flat Claude-Code `context` field and dropped it — so the model never
  learned its session_id and couldn't open a transaction. The translator now
  reads the codex-native nested shape.
- **Practitioner identity is now carried into the sandboxed shell.** empirica
  keys per-practitioner calibration/Brier off `EMPIRICA_INSTANCE_ID`, which the
  plugin set for hook subprocesses only. When the model ran `empirica ...` from
  the sandboxed shell, the exec path injected `CODEX_THREAD_ID` but not
  `EMPIRICA_INSTANCE_ID`, so empirica resolved a `None` practitioner — breaking
  `project_path` resolution and per-thread calibration. The exec env now mirrors
  the codex thread id as `EMPIRICA_INSTANCE_ID` (the thread id **is** the
  practitioner id).

### Changed
- **Cortex mesh auth is opt-in and automatic for mesh installs.** The `ecodex`
  wrapper exports `CORTEX_API_KEY` from `~/.empirica/credentials.yaml` when
  present, so mesh installs authenticate regardless of shell rc. OSS-only users
  (no cortex key) are unaffected.
- **Org migration `Nubaeon` → `EmpiricaAI`.** Repo, Homebrew tap, and
  documentation URLs now point at the canonical `EmpiricaAI` org
  (`brew install EmpiricaAI/tap/ecodex`, `github.com/EmpiricaAI/ecodex`). Old
  `Nubaeon` URLs redirect.

## [0.2.3] - 2026-06-25

### Fixed
- **Firewall now fails CLOSED on a broken gate.** The PreToolUse Sentinel
  firewall previously let a tool call through if the gate *ran but exited an
  unexpected code* (e.g. a Python traceback → exit 1) or *could not be spawned*
  — codex treats any exit code other than 2 as allow. A broken firewall now
  re-emits as a deny (exit 2 + stderr); only a genuinely **absent** (uninstalled)
  gate still fails open, so an un-gated install isn't bricked.
- **Advisory `ask` no longer fails open.** codex has no PreToolUse `ask`
  decision (it treats `ask` as unsupported and runs the tool). The empirica gate
  emits `ask` for an advisory carry-over-INVESTIGATE nudge, which therefore
  leaked genuinely-praxic tools through. The translate layer now normalizes
  `ask` → `deny` at the codex boundary (the gate's reason is preserved; the
  upstream CC gate keeps `ask` for its interactive human-override path).
- **Reason-less deny guard.** codex fails a `deny` open when it carries no
  reason; every translated deny is now guaranteed a non-empty reason.

### Added
- **Vendored-firewall drift-guard** (`scripts/check_vendored_firewall.py` + CI
  job): asserts the vendored gate retains its security-critical invariants (the
  recovery escape hatch, codex-native `permissionDecision` emission) so a future
  re-vendor or edit can't silently drop them. Invariant-presence check, not a
  byte-diff (the vendored hooks are intentionally genericized).
- **End-to-end firewall guard tests**: encode codex's block contract and assert
  the sentinel → translate → codex chain blocks for every decision that must
  block — the regression net for both the v0.2.0 silent break and the `ask`
  fail-open.

## [0.2.2] - 2026-06-24

### Fixed
- **empirica MCP server now actually starts** (`f6525b23f2`): the plugin's `mcp_servers.json` launched `empirica mcp`, which is not a valid subcommand — so even with v0.2.1's manifest `mcpServers`-key fix, the empirica MCP server never started. The real server is a separate binary, **`empirica-mcp`**. Runtime-verified via an MCP `initialize` handshake (`serverInfo: empirica 1.27.1`, tools capability). The `mcp__empirica__*` tools are now reachable in ecodex.

### Changed
- **Curated ecodex CI** (`ddb9bb1d76`, `de8fe8b95c`): replaced openai/codex's inherited CI workflows — which tested codex's *repo invariants* (TUI↔core boundary, Bazel/Cargo clippy parity, npm staging from openai's own release runs, README ASCII house-style) and didn't even run cargo, so ecodex's own code went untested on push — with a lean CI that builds + tests the crates ecodex owns (`codex-empirica-plugin`, `codex-empirica-translator`) on the pinned 1.95.0 toolchain. Removed 6 inherited openai workflows; the new CI immediately caught + fixed an env-var-race test flake.

## [0.2.1] - 2026-06-24

### Fixed
- **Sentinel gating restored** (`83c635e9b4`) — the headline fix. In v0.2.0 the PreToolUse firewall did **not** actually block tool calls: the plugin's hook-output translation layer (added in T81 to strip `suppressOutput`) read only the legacy top-level `decision` field, so the sentinel's `hookSpecificOutput.permissionDecision: "deny"` was silently dropped and praxic actions ran despite a deny — model-agnostic. `translate_pre_tool_use` now carries the codex-native `permissionDecision` (`allow`/`deny`/`ask`) through, maps legacy `block`/`approve` → `deny`/`allow` (codex's enum rejects `block`/`approve`), and pulls the reason from either shape. Covered by regression tests on the exact sentinel output shape, and runtime-confirmed (a praxic tool with no open transaction is denied end-to-end).
- **empirica MCP server now loads** (`83c635e9b4`): the plugin manifest declared `mcp_servers` (snake_case) where codex's `camelCase` manifest schema expects `mcpServers`; the key silently resolved to `None`, so the `mcp__empirica__*` tools were unavailable.
- **`tar` 0.4.45 → 0.4.46** (`a4902bca42`): upstream archive-extraction security fix; `tar` ships in the binary via the plugin archive-extraction path.

### Changed
- **De-Claude pass on the always-on skills + scanner** (`4cf4fa754d`): the pinned `epistemic-persistence-protocol` and `epistemic-transaction` skills (injected into model context every session) no longer refer to the running model as "Claude" / "Claude Desktop" / "Claude Code Tasks" — ecodex runs non-Claude models. The `setup-codex` de-Claude scanner now also covers the `skills/` directory (previously an unscanned blind spot).
- **Retired dead Claude-Code-only loop/listener machinery** (`dbe7082801`, −1002 LOC): removed the `loop`/`listener` install+uninstall-pickup hooks (built on `CronCreate`/`/loop`, which don't exist in codex) and the `loop-cron` / `inbox-listener` skills — superseded by the native ntfy listener (`ntfy_listener.rs`) + `session-monitor-arm` + the `empirica loop` CLI. A `setup-codex` skip-list keeps them retired across syncs.
- **Dependabot disabled** (`f9ff24a322`): the inherited openai/codex config scanned the whole upstream dependency surface; security posture for the shipped binary now relies on periodic `cargo audit`.

## [0.2.0] - 2026-06-23

### Added
- **Mistral as the EU data-sovereignty cloud route** (`c2457d0d6e`): adds Mistral (Devstral coding models) as a curated provider tagged `jurisdiction.eu_data_residency`, giving teams under GDPR / EU AI Act constraints a hosted non-US route alongside the existing OpenRouter / direct-API / local options. Wired into `config.toml.default`, `models.curated.json`, and the curated-models registry. See `docs/ecodex/integrations/providers.md`.
- **Harness-integrity guards on the vendored hook layer** — three of empirica's integrity checks, adapted for ecodex's vendored Python hooks:
  - **SQL schema-reference guard** (#1, `7d79941a6f`): a test that fails if the vendored hooks reference DB columns/tables absent from the schema, catching schema drift between empirica and the vendored copy before it ships.
  - **Import-budget gate** (#3, `7d620f2a5a`): bounds the import surface of the hot-path hook scripts so a heavyweight import can't silently regress per-call latency.
  - **Compliance crosswalk** (#4, `56aa7d4c20`): a published asset mapping ecodex's controls to EU AI Act / GDPR / ISO 42001 obligations (`docs/ecodex/positioning/compliance-crosswalk.{md,html}`).

### Changed
- **2026-06 upstream codex sync merged** (`a0208269ca`, `0befb393f3`): large forward-port of openai/codex onto ecodex's plugin layer. Notable upstream surface now carried includes per-turn and thread-level multi-agent mode, remote exec-environment connection lifecycle + snapshots, token-budget-driven compaction (with budget-expiry turn aborts), UUIDv7 context-window lineage IDs, indexed/cached web-search modes, protected-resource OAuth discovery, and a clock current-time tool. ecodex's integrations (plugin writable-roots, provider hot-swap, the `monitor` tool) were re-reconciled against the new session semantics.
- **PR#138 sentinel rush-guard fix re-vendored** (`d31ef316b5`): re-syncs 25 drifted hook/lib/script files from empirica `develop`, landing the rush-guard recovery-verb hoist so check / postflight / `*-log` calls aren't blocked when the noetic window is still fresh.

### Fixed
- **Model picker stuck on single-effort curated models + unsent queued input** (`f6657fa948`): the TUI treated a single supported reasoning-effort as "no choice" with the wrong comparison (`== 1` rather than `<= 1`), wedging the picker for curated models that expose one effort level, and could drop queued input.
- **Release-gate test reconciliation** (`2ee947032b`, `5ad69972ce`…`5c8e274879`): app-server integration tests fixed for the merged signatures and ecodex branding — `mcp_server_status` made robust under load via bounded test parallelism (after reverting an ineffective read-timeout bump), `executor_mcp` updated to the `ecodex` binary name, `command_exec` switched to `sh -c` (non-login shell) to avoid login-profile network-probe noise, and plugin-manifest test fixtures gained the upstream-added `statusline` / `writable_roots` / `pinned` fields.
- **Compliance report green (11/11, score 1.0)** (`2ee947032b`): added a minimal `pyproject.toml [tool.ecodex] version` mirror so the Python-centric `release_chain` check reads a true version for the Rust workspace, excluded the positioning visual-gen scripts from lint, and refactored (rather than config-dodged) the SQL schema-reference test's complexity violation.

## [0.1.0] - 2026-06-02

### Added
- **L3 model registry — curated seed + provider discovery** (`codex-models-manager` + `cli`, commits `aec472f4b8` / `db37c97461` / `b93d47da5c`): ecodex now ships a curated model registry so the picker is useful out of the box. Three-layer slug resolution (`model_info_from_slug`): exact-curated entry (bundled `models.curated.json` ∪ `~/.codex/models.user.json`) → family-prefix table → generic fallback. Lean seed schema (curated/epistemic fields only — context, tools, `reasoning.supported`, routes, `jurisdiction`, `calibration_tier`, `last_verified`); codex-runtime fields inherited from the runtime template at `enrich()` time. Seed: 11 entries (8 deep-research 3-vote-verified non-GPT coding models — Kimi K2.6, Qwen3.7/3.6 Max, DeepSeek-R1, gpt-oss 120B/20B, Qwen3-Coder-30B, GLM-5.1, MiniMax-M2.7 — plus 2 EU-sovereignty Mistral Devstral entries tagged `jurisdiction.eu_data_residency`). New `ecodex models list` (show resolved registry) and `ecodex models refresh [--provider ID] [--dry-run] [--no-filter]` (probe each `[model_providers.*]` `/v1/models`, curated-families filter + non-coding/unstable-variant exclusion + latest-per-line collapse, write `models.user.json`). Discovery proven live (OpenRouter + local: 366 discovered → 81 kept). `calibration_tier` ships `unmeasured` for every entry by design — populated from grounded usage, never asserted. Local serving backends documented + exemplified: Ollama/LM Studio (built-in), llama.cpp `:8080` + vLLM `:8000` (added to `config.toml.default`). See `docs/ecodex/integrations/model-registry.md`. 43/43 models-manager tests.
- **Native ntfy mesh wake-listener** (`codex-rs/core/src/ntfy_listener.rs`, branch `feature/native-ntfy-listener` merged `54bf8b6ea6`): native Rust held-connection ntfy stream loop (reqwest, reconnect/backoff, doorbell wake via `inject_response_items` mirroring `monitor.rs`) + session-boot wiring (creds-gated, `cfg!(test)`-guarded) + shutdown abort. Decouples the mesh push transport from the empirica Python CLI so ecodex wakes on mesh events natively. Config + credentials loader, subscribe-URL builder, ai_id resolution. codex-core 1826/0 (+14 listener tests).
- **Translator multi-upstream router** (`codex-empirica-translator`, commit `dcec5b4099`): one translator process now serves N upstream providers, routing per-request via first-match-wins glob on the incoming Responses request's `model` field. New `--upstreams-config <TOML>` flag; existing single-upstream flags preserved for backwards compatibility. Sample config at `examples/upstreams.toml` covers Kimi (Anthropic mode), DeepSeek/Qwen/GLM/Anthropic/OpenRouter as commented templates. New `Upstream` + `UpstreamRouter` public API. `/healthz` now lists configured upstreams inventory. 33/33 tests pass.
- **Plugin multi-script fan-out** (`codex-empirica-plugin`, commit `630b7b2ab7`): empirica plugin now mirrors Claude Code's `~/.claude/settings.json` multi-handler-per-event wiring. New `run-hook EVENT SCRIPT.py` generic dispatcher subcommand; `hooks.json` declares sibling scripts per event (UserPromptSubmit fires 6 scripts, SessionStart 4, PostToolUse 2). Closes the "compulsion gap" where tool-router.py's siblings (`context-shift-tracker`, `loop/listener install/uninstall pickups`) were never firing. Vendored previously-missing `session-monitor-arm.py`.
- **Hook event schema additions** (`codex-rs/protocol/src/protocol.rs` + 12 other files, commit `7bcf85c3b8`): `HookEventName` enum extended with 7 new variants matching CC's wider surface — `PreCompact`, `PostCompact`, `SessionEnd`, `SubagentStart`, `SubagentStop`, `TaskCompleted`, `PostToolUseFailure`. Plugins can declare handlers in `hooks.json` today.
- **TaskCompleted dispatch site** (`codex-rs/hooks/src/events/task_completed.rs` + 6 wiring sites): first of the 7 PR2 dispatch sites. Informational hook fires at the normal agent-done lifecycle point (`codex-rs/core/src/session/turn.rs:570`, after Stop's continuation flow, before the legacy AfterAgent notify). Plugin handlers attach POSTFLIGHT-enforcement here without changing Stop semantics. Pattern proven for the remaining 6 events — see `docs/ecodex/hook-events-roadmap.md` "Dispatch pattern (minimal sibling)" section.
- **PostToolUseFailure dispatch site** (`codex-rs/hooks/src/events/post_tool_use_failure.rs` + 7 wiring sites): second PR2 dispatch site. Fires when a tool invocation fails (non-zero exit, exception, timeout) — sibling to `PostToolUse` which only fires on success. Dispatch at `codex-rs/core/src/tools/registry.rs:434`. Payload: tool_name + tool_input + error_message + duration_ms. Plugin handlers attach dead-end-logging here.
- **PreCompact + PostCompact dispatch sites** (`codex-rs/hooks/src/events/{pre,post}_compact.rs` + 9 wiring sites): third and fourth PR2 dispatch sites. Both fire from `codex-rs/core/src/tasks/compact.rs:CompactTask::run` — the single entry point that branches into the three compaction implementations (local/remote/remote_v2). PreCompact awaits synchronously before compaction runs (natural block via `.await` — no continuation/should_block semantics needed). PostCompact fires after with `success: bool`. Plugins snapshot epistemic state to `~/.empirica/breadcrumbs` on PreCompact, restore on PostCompact.
- **SessionEnd dispatch site** (`codex-rs/hooks/src/events/session_end.rs` + 7 wiring sites): fifth PR2 dispatch site. Fires at the start of `shutdown()` in `codex-rs/core/src/session/handlers.rs`, before `abort_all_tasks`. Plugin handlers run final POSTFLIGHT + capture session snapshot + curate the rollout while history/tasks/MCP are still alive. Payload carries `turn_count` so plugins can apply session-summary thresholds.
- **SubagentStart + SubagentStop dispatch sites** (`codex-rs/hooks/src/events/subagent_{start,stop}.rs` + 9 wiring sites): sixth and seventh PR2 dispatch sites. SubagentStart fires in the parent session after `spawn_agent` tool succeeds (`codex-rs/core/src/tools/handlers/multi_agents/spawn.rs:~176`). SubagentStop fires in the subagent's own session when it calls `report_agent_job_result` (`codex-rs/core/src/tools/handlers/agent_jobs/report_agent_job_result.rs:handle()`). Asymmetric session contexts reflect codex's threading model — plugins correlate parent→child via `child_thread_id` (Start) + `session_id` (Stop).
- **`monitor` tool primitive** (`codex-rs/core/src/monitor.rs` + tool handler + spec registration, commit `5a1ae1658c`): new agent-callable tool that arms a watch on a background subprocess. On each line matching the supplied regex, a `<task-notification>` message is injected into the agent's pending input via `Session::inject_response_items` — sub-second wake on background events. Parity with Claude Code's `Monitor` tool. Bundled spawn+watch in a single tool call (one atomic API). Per-session registry on `SessionServices`; `shutdown()` aborts all entries. Closes the wake-on-event gap that prevented non-Claude models from participating fully in the Empirica AI mesh. See `docs/ecodex/monitor.md`.
- **Inherited-transaction-bug fix** (`codex-rs/codex-empirica-plugin/assets/hooks_scripts/hooks/session-init.py`, commit `b8c9f02ece`): SessionStart now auto-postflights orphaned open transactions with reason `session-pickup auto-close` instead of silently adopting them. Vectors are carried forward from the orphaned state with `completion=1.0`. Prevents cross-session calibration corruption (previous adoption logic continued the old measurement window across unrelated work). Fallback to legacy adoption on auto-postflight failure preserves data.
- **Statusline sync from CC vendored copy** (`codex-rs/codex-empirica-plugin/assets/hooks_scripts/scripts/statusline_empirica.py`, commit `8f2ee46b5c`): pulls in three CC improvements ecodex was missing — praxic-phase emoji changed `⚙` → `🔨` (east-asian-width fix preventing digit overlap on some terminals), threshold info no longer rendered in live statusline (Sentinel-scoped, not actionable mid-call), caller-side threshold fetch removed (saves a DB round-trip per render tick).

### Configuration (no code change)
- **Cortex MCP server wiring guidance for `~/.codex/config.toml`**: ecodex agents can now reach the same `mcp__cortex__*` tools that CC sessions use by adding a `[mcp_servers.cortex]` block with the streamable_http transport and a bearer-token env var. Documented in `docs/ecodex/cross-ai-mesh.md`. With the new `monitor` tool above, this completes the cross-AI mesh story — ecodex participates as a first-class peer.

### Notes
- Goal `f0004294` complete: **7/7 PR2 dispatch sites shipped** this session. All 7 hook events declarable in `hooks.json` AND wired to fire at their lifecycle points. Plugin authors can attach handlers and they will execute.
- Cross-AI mesh participation now structurally complete in ecodex: cortex MCP integration (call mesh tools) + `monitor` primitive (wake on mesh events) + all PR2 dispatch sites (lifecycle hook coverage). Demonstrating an end-to-end mesh interaction between a CC session and an ecodex session running a non-Claude model is the open-source proof point that empirica is a cross-platform layer, not a Claude-specific framework.

## [0.0.1] - 2026-05-10

### Added
- **Tx-BC** (`scripts/release.sh`): Phase 1 of the release pipeline — version bump, CHANGELOG roll, commit, tag. Replaces the prior placeholder stub.
- **Tx-BB**: rustdoc on `codex-empirica-plugin` (47% → 100%) and `codex-empirica-translator` (56% → 64%); overall owned-crate docs coverage 53.6% → 75.0%.
- **Tx-BA** (`empirica rust-docs-assess`): Rust-aware docstring-coverage measurement on Cargo.toml workspace members. Replaces docpistemic for forks where Python-discovery inflates the denominator with upstream noise.
- **Tx-AY**: `[features] plugin_hooks=true` + `plugins=true` seeded by `install.sh` on first install. Cargo.toml recognized by empirica's repo_hygiene `version_file` check.
- **Tx-AW** (`scripts/install.sh`): `--fast` flag (T79) flips builds to `[profile.fast-release]`. Idempotent `[features]` block append for existing configs. `rm`-then-`cp` pattern dodges ETXTBSY on running ecodex sessions. Post-install `pgrep` warning when in-flight sessions need restart.
- **Tx-AT**: empirica plugin auto-trusted on first install via `ECODEX_AUTO_TRUSTED_PLUGIN_IDS` allowlist in upstream's hook discovery (codex-rs/hooks/src/engine/discovery.rs). Restores pre-PR-#20321 first-install runnability.
- **Tx-AO**: upstream codex sync — 162 commits forward-ported. T78 `ArcSwap<ModelClient>` hot-swap pattern preserved against upstream's reverted `client_session` shape.
- **Tx-AI**: plugin manifest `writableRoots` contribution surface. Plugins declare cross-cwd writable scope; codex's SandboxPolicy honors them at session bootstrap.
- **Tx-AG**: investigation-proportionality budget enforcement. `tool-router.py` arms a per-session counter on hypothesis-bearing prompts; `sentinel-gate.py` denies Read/Grep/Glob after the configured limit (default 5) until the next user prompt resets.
- **Tx-AJ**: `EMPIRICA_SENTINEL_FAIL_CLOSED` env opt-in for hardened deployments — outermost catch flips from fail-open allow to fail-closed deny.
- **CONTRIBUTING.md**: three-layer contribution model (L1 codex foundation / L2 empirica plugin / L3 ecodex-specific) with concrete routing.
- **docs/ecodex/INSTALL.md**: prerequisites, --user vs --system, providers config, hot-swap semantics, idempotent reinstall.

### Changed
- **README.md**: sharper transaction-lifecycle phrasing (PREFLIGHT / CHECK / POSTFLIGHT defined inline with the noetic↔praxic distinction). New Glossary section. Relationship-to-codex section now names the one upstream patch we maintain (Tx-AT allowlist).

### Fixed
- **Tx-AU**: workspace test fixtures resolved post-Tx-AO — `TurnStartParams` literal in v2/tests.rs gained `model_provider`; `LoadedPlugin` test fixture gained extension fields; 7 conflict markers in core-skills/loader_tests.rs resolved.
- **Tx-AV**: `cargo clippy --workspace --all-targets` from 28 errors → 0 after rust 1.93's stricter lints landed via the upstream sync.

## [0.0.0]

Pre-release development. Not yet versioned. The full pre-versioning history is in the git log and on the [build/v1-plugin branch](https://github.com/EmpiricaAI/ecodex/commits/build/v1-plugin).

[Unreleased]: https://github.com/EmpiricaAI/ecodex/compare/v0.152.0...HEAD
[0.152.0]: https://github.com/EmpiricaAI/ecodex/compare/v0.152.0...v0.152.0
[0.149.0]: https://github.com/EmpiricaAI/ecodex/compare/v0.149.0...v0.149.0
[0.147.2]: https://github.com/EmpiricaAI/ecodex/compare/v0.147.1...v0.147.2
[0.147.1]: https://github.com/EmpiricaAI/ecodex/compare/v0.147.0...v0.147.1
[0.147.0]: https://github.com/EmpiricaAI/ecodex/compare/v0.147.0...v0.147.0
[0.146.0]: https://github.com/EmpiricaAI/ecodex/compare/v0.145.0...v0.146.0
[0.145.0]: https://github.com/EmpiricaAI/ecodex/compare/v0.2.7...v0.145.0
[0.2.7]: https://github.com/EmpiricaAI/ecodex/compare/v0.2.6...v0.2.7
[0.2.6]: https://github.com/EmpiricaAI/ecodex/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/EmpiricaAI/ecodex/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/EmpiricaAI/ecodex/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/EmpiricaAI/ecodex/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/EmpiricaAI/ecodex/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/EmpiricaAI/ecodex/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/EmpiricaAI/ecodex/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/EmpiricaAI/ecodex/compare/v0.0.1...v0.1.0
[0.0.1]: https://github.com/EmpiricaAI/ecodex/compare/v0.0.0...v0.0.1
[0.0.0]: https://github.com/EmpiricaAI/ecodex/releases/tag/v0.0.0
