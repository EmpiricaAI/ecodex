---
name: Bug report
about: Something in ecodex isn't working
title: "[bug] "
labels: bug
---

## What happened

<!-- Concrete description. What did you expect, what did you see? -->

## How to reproduce

```sh
# commands you ran
```

## Layer affected

Which layer does the bug live in? (See README "What ecodex adds on top of codex" for the L1/L2/L3 model.)

- [ ] **L1 — upstream codex** (will be reported separately to openai/codex)
- [ ] **L2 — empirica plugin** (`codex-rs/codex-empirica-plugin/`)
- [ ] **L3 — ecodex-specific** (translator, curated providers, install script, statusline, koru-spiral, etc.)
- [ ] Not sure

## Environment

- `ecodex --version`:
- Install path: <!-- Homebrew / direct binary / cargo install --git / source build -->
- OS: <!-- Linux distro+version, macOS version -->
- Provider in use: <!-- DeepSeek / Qwen / Kimi / Ollama / OpenRouter / etc. -->

## `empirica diagnose-ecodex` output

<details>
<summary>doctor output</summary>

```
<!-- paste the full output here -->
```
</details>

## Relevant logs

<!-- If the issue involves the agent loop: paste relevant lines from ~/.codex/log/codex-tui.log -->
<!-- If the issue involves the plugin: paste from ~/.codex/log/empirica-plugin.log -->
