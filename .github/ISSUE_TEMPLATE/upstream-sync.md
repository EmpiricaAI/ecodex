---
name: Upstream sync
about: Track a sync of upstream openai/codex changes into ecodex
title: "[upstream-sync] "
labels: upstream-sync
---

## Upstream range

- Last synced commit: <!-- e.g. 1234abcd or v0.x.y -->
- Target commit: <!-- the upstream HEAD or release we want to pull -->
- Commits in range: <!-- gh api repos/openai/codex/compare/{last}...{target} or similar -->

## Why now

<!-- What's driving this sync? -->

- [ ] Routine scheduled sync
- [ ] Specific upstream fix we want (link the upstream PR/commit)
- [ ] Security advisory upstream
- [ ] Required for a feature we're building

## Risk assessment

Areas where ecodex diverges from upstream that need careful merge attention:

- [ ] Session/turn lifecycle (T78 ArcSwap<ModelClient> hot-swap pattern)
- [ ] Plugin trust (Tx-AT ECODEX_AUTO_TRUSTED_PLUGIN_IDS allowlist in discovery.rs)
- [ ] Hook dispatcher (Tx-AE/AR hook output translation)
- [ ] Translator integration points
- [ ] Statusline (ecodex-specific rendering)
- [ ] config.toml.default + curated_models.rs
- [ ] Build profiles (fast-release)
- [ ] Other: <!-- specify -->

## Sync checklist

- [ ] `git fetch upstream && git log --oneline last..target -- <path>` for each diverged area
- [ ] Resolve conflicts; preserve ecodex divergences listed above
- [ ] `cargo build --release -p codex-cli -p codex-empirica-plugin -p codex-empirica-translator`
- [ ] `cargo test --lib -p codex-cli -p codex-empirica-plugin -p codex-empirica-translator`
- [ ] `empirica diagnose-ecodex` clean
- [ ] Manual smoke test: pick a curated model, run a turn that uses tools
- [ ] CHANGELOG.md `[Unreleased]` entry with summary + risk notes
- [ ] PR with all of the above documented in the description
