# @nubaeon/ecodex

> **npm is not a canonical distribution channel for ecodex.** This wrapper exists in the repo for future use but is not currently published. See canonical install paths below.

## Canonical install

| Channel | Command |
|---|---|
| Homebrew (Mac/Linux) | `brew install nubaeon/tap/ecodex` |
| Direct binary | Download from [GitHub Releases](https://github.com/Nubaeon/ecodex/releases) |
| Cargo (Rust devs, source build) | `cargo install --git https://github.com/Nubaeon/ecodex codex-cli` |
| Build from source | `git clone … && ./ecodex/scripts/install.sh` |

ecodex is a Rust binary serving the open-weights operator audience (Llama / Qwen / DeepSeek / Kimi via Ollama, vLLM, OpenRouter, direct cloud APIs). That audience reaches for cargo, brew, or curl — not `npm install -g`. The npm postinstall pattern also carries a security tax (arbitrary node at user privilege) we don't want to charge users without strong reason.

## What this directory is

A thin spawn wrapper (`bin/ecodex.js`) + postinstall downloader (`scripts/postinstall.js`) + `package.json` for `@nubaeon/ecodex`. Code lives here so we can flip distribution on later if the audience expands. `scripts/release.sh --publish-npm` exercises this path for testing.

If you actively want the npm route despite the above, the pipeline still supports it — but it's an experimental opt-in, not a recommended install path.

## License

Apache-2.0. See [LICENSE](https://github.com/Nubaeon/ecodex/blob/main/LICENSE).
