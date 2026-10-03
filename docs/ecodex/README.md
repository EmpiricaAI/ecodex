# ecodex docs

Fork-specific documentation for ecodex, Empirica's build of
[openai/codex](https://github.com/openai/codex) with the Empirica epistemic-discipline
plugin bundled. Upstream codex's own docs stay in `docs/` (the parent directory),
untouched.

## Layout

| Path | Contains |
|---|---|
| [`INSTALL.md`](INSTALL.md) | Install + first-run guide: channels, what the first session writes, choosing a provider, updating, uninstalling, unattended runs, troubleshooting. |
| [`system-overview.md`](system-overview.md) | The three-layer architecture (L1 codex foundation / L2 empirica integration / L3 specialised ecodex code), runtime composition and file layout. |
| [`MISTRAL_SOVEREIGN.md`](MISTRAL_SOVEREIGN.md) | Wiring EU-sovereign Mistral models (Devstral / Codestral) through the translator. |
| [`cross-ai-mesh.md`](cross-ai-mesh.md) | How an ecodex session participates in the Empirica AI mesh: the native listener, the mailbox CLI, the optional Cortex MCP server. Setup, walkthrough, troubleshooting. |
| [`monitor.md`](monitor.md) | The `monitor` tool: watch a subprocess for a pattern and wake the agent on each match. |
| [`hook-events-roadmap.md`](hook-events-roadmap.md) | How ecodex's hook-event surface relates to upstream codex's: which events are upstream's, which two are ecodex's, and where each is dispatched. |
| [`epistemic-llms.md`](epistemic-llms.md) | Which models hold up under the epistemic discipline, and why the framing matters for LLM agents. |
| [`epistemic-programming.md`](epistemic-programming.md) | Epistemic programming as a paradigm, and how Empirica's transactions, claims and artifacts map onto it. |
| [`verdict-integrity-audit.md`](verdict-integrity-audit.md) | Audit of claim/falsifier verdict integrity across the transaction loop. |
| [`api/`](api/) | Plugin API contracts: [`hooks.md`](api/hooks.md), [`skills.md`](api/skills.md), [`mcp.md`](api/mcp.md), [`plugin-statusline.md`](api/plugin-statusline.md), [`plugin-writable-roots.md`](api/plugin-writable-roots.md), [`integration-tests.md`](api/integration-tests.md). |
| [`integrations/`](integrations/) | Provider and model integration: [`providers.md`](integrations/providers.md), [`model-registry.md`](integrations/model-registry.md), [`model-notes.md`](integrations/model-notes.md), [`huggingface.md`](integrations/huggingface.md), [`discipline-strengthening.md`](integrations/discipline-strengthening.md), and the historical [`branding.md`](integrations/branding.md). |
| [`experiments/`](experiments/) | Experiment designs: the ecodex-lab baseline, the shadow-corpus scoping and prevention-value experiments, and their subject manifests. |
| [`positioning/`](positioning/) | Positioning material: mesh-vs-worktrees and swarm-vs-society pages, the compliance crosswalk, deck sources and generated SVGs. |
| [`specs/`](specs/) | Design specs for parked or in-progress work. |
| [`inspection.md`](inspection.md) | Historical: the initial inspection of codex-rs (hook system, plugin marketplace, thread-scoped goals) that drove the architecture decision. Line references and counts are from that time. |
| [`architecture.md`](architecture.md) | Historical: the architecture commitments (distribution model, empirica-language strategy, goal pairing, memory interop, fork posture) as decided before the v1 build. |

## Current status

- **Plugin:** every hook event is wired through the plugin's `hooks.json` — upstream's
  `PreToolUse`, `PostToolUse`, `SessionStart`, `UserPromptSubmit`, `Stop`, `PreCompact`,
  `PostCompact`, `SessionEnd`, `SubagentStart`, `SubagentStop`, plus ecodex's
  `TaskCompleted` and `PostToolUseFailure`. `PermissionRequest` is accepted as a
  subcommand but has no handler. Several events fan out to more than one script, mirroring
  Claude Code's `settings.json`. See [`api/hooks.md`](api/hooks.md) and the crate's
  [README](../../codex-rs/codex-empirica-plugin/README.md).
- **Hook events:** two ecodex additions on top of upstream's set. See
  [`hook-events-roadmap.md`](hook-events-roadmap.md).
- **Cross-AI mesh:** the native listener wakes the session on each Cortex notification; the
  `empirica mailbox` CLI and the optional Cortex MCP server carry the content. See
  [`cross-ai-mesh.md`](cross-ai-mesh.md).
- **Distribution:** the install script, Homebrew, release tarballs, cargo and the source
  build all produce the same `ecodex`. See [`INSTALL.md`](INSTALL.md).

## Where things live

| Concern | Location |
|---|---|
| The plugin source | `codex-rs/codex-empirica-plugin/` |
| The plugin's own README (cargo convention) | `codex-rs/codex-empirica-plugin/README.md` |
| Plugin manifest and hook wiring | `codex-rs/codex-empirica-plugin/manifest.json`, `hooks.json` |
| Vendored empirica hooks, skills, agents | `codex-rs/codex-empirica-plugin/assets/` |
| The translator | `codex-rs/codex-empirica-translator/` |
| Curated model seed (the `/model` picker) | `codex-rs/models-manager/models.curated.json` |
| Installers and wrapper | `scripts/install.sh`, `ecodex/scripts/` |
| ecodex docs | `docs/ecodex/` (this directory) |
| Upstream codex docs | `docs/` (parent directory, untouched) |

## Contributing

ecodex is a product fork — quality fixes that aren't ecodex-specific should land both in
our fork and as upstream PRs against [openai/codex](https://github.com/openai/codex). See
[`../../CONTRIBUTING.md`](../../CONTRIBUTING.md).
