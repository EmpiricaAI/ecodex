<!-- Thanks for contributing to ecodex! -->

## Summary

<!-- One or two sentences. What does this PR change and why? -->

## Layer

Which layer is this PR primarily touching?

- [ ] **L1 — upstream codex** (probably should be sent to openai/codex first; link upstream PR if it exists)
- [ ] **L2 — empirica plugin** (`codex-rs/codex-empirica-plugin/`)
- [ ] **L3 — ecodex-specific** (translator, curated providers, install script, statusline, etc.)
- [ ] **Docs / CI / infra**

## Changes

<!-- Bulleted list of the meaningful changes. Skip the rote ("updated tests"). -->

- 
- 

## Test plan

- [ ] `cd codex-rs && cargo build --release -p codex-cli -p codex-empirica-plugin -p codex-empirica-translator`
- [ ] `cd codex-rs && cargo test --lib -p codex-cli -p codex-empirica-plugin -p codex-empirica-translator`
- [ ] `cd codex-rs && cargo clippy --workspace --all-targets`
- [ ] Manual smoke test:
- [ ] `empirica diagnose-ecodex` (where applicable)

## Risk

<!-- What could break? Backward compat concerns? Breaking changes for the install script / config / hot-swap path? -->

## Related

<!-- Issues, upstream PRs, prior commits. -->
