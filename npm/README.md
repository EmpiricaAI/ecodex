# @nubaeon/ecodex

npm wrapper for [ecodex](https://github.com/Nubaeon/ecodex) — Empirica's epistemic agent environment, a fork of [openai/codex](https://github.com/openai/codex) with measured agent discipline.

## Install

```sh
npm install -g @nubaeon/ecodex
```

The postinstall hook downloads the platform-specific binary (Linux/macOS, x86_64/arm64) from the matching GitHub release. Failure to download is non-fatal — `ecodex` itself will surface a clear error if the binary isn't present, with instructions to install from source.

## Run

```sh
ecodex
```

Pure passthrough — args + stdin/stdout/stderr + exit code all forward to the underlying Rust binary.

## Why a thin wrapper?

ecodex is a Rust binary. The npm wrapper exists so `npm install -g @nubaeon/ecodex` works as a discovery + install path for the JavaScript-tooling crowd. The wrapper carries no logic of its own beyond binary selection + arg forwarding.

If you build from source (the recommended path for contributors), skip npm and use [`ecodex/scripts/install.sh`](https://github.com/Nubaeon/ecodex/blob/build/v1-plugin/ecodex/scripts/install.sh) directly.

## License

Apache-2.0. See [LICENSE](https://github.com/Nubaeon/ecodex/blob/main/LICENSE).
