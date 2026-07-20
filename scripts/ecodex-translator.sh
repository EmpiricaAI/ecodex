#!/usr/bin/env bash
# ecodex-translator — launch codex-empirica-translator with provider keys sourced
# from ~/.empirica/credentials.yaml (the canonical key store; values never printed).
#
# WHY THIS EXISTS: codex is Responses-API-only (upstream removed the chat wire,
# commit d2394a2494). So chat-completions providers (Mistral, DeepSeek, GLM, Qwen)
# and Anthropic-surface ones (Kimi, Claude) can NOT be talked to directly — they
# route through this translator (codex Responses -> CIF -> provider chat/anthropic).
# ecodex does NOT auto-spawn it, so start this before launching ecodex against any
# such provider. Local providers that speak Responses natively (Ollama, llama.cpp
# on the Strix) do NOT need the translator.
#
# Usage:
#   ecodex-translator                              # foreground, 127.0.0.1:18080
#   ecodex-translator --bind 0.0.0.0:18080         # custom bind
#   nohup ecodex-translator >~/.codex/translator.log 2>&1 &   # background
#
# Config: routes live in $ECODEX_TRANSLATOR_UPSTREAMS
# (default ~/.codex/translator-upstreams.toml). Provider keys read from
# credentials.yaml — add an `export <X>_API_KEY=...` line below per upstream.
set -euo pipefail

CREDS="${EMPIRICA_CREDENTIALS:-$HOME/.empirica/credentials.yaml}"
UPSTREAMS="${ECODEX_TRANSLATOR_UPSTREAMS:-$HOME/.codex/translator-upstreams.toml}"
BIND_DEFAULT="127.0.0.1:18080"

# Locate the translator binary (PATH, then common build/install locations).
BIN="$(command -v codex-empirica-translator || true)"
if [[ -z "$BIN" ]]; then
  for cand in "$HOME/.local/bin/codex-empirica-translator" \
              "$HOME/empirical-ai/ecodex/codex-rs/target/release/codex-empirica-translator"; do
    [[ -x "$cand" ]] && { BIN="$cand"; break; }
  done
fi
[[ -n "$BIN" ]] || { echo "ecodex-translator: binary not found — build/install codex-empirica-translator" >&2; exit 1; }
[[ -f "$UPSTREAMS" ]] || { echo "ecodex-translator: upstreams config not found at $UPSTREAMS" >&2; exit 1; }

# Read a nested key from credentials.yaml (value never echoed). Empty on any error
# so the translator fails-loud at startup rather than sending an empty key.
read_key() {
  python3 -c "import yaml;d=yaml.safe_load(open('$CREDS')) or {};s=d.get('$1');print((s or {}).get('$2','') if isinstance(s,dict) else '')" 2>/dev/null || true
}

# Provider keys — one export per active upstream in translator-upstreams.toml.
export MISTRAL_API_KEY="$(read_key mistral api_key)"
# export KIMI_API_KEY="$(read_key kimi api_key)"
# export DEEPSEEK_API_KEY="$(read_key deepseek api_key)"

# Inject a default --bind only if the caller didn't pass one.
case " $* " in *" --bind "*) : ;; *) set -- --bind "$BIND_DEFAULT" "$@" ;; esac

echo "ecodex-translator: $BIN --upstreams-config $UPSTREAMS $*" >&2
exec "$BIN" --upstreams-config "$UPSTREAMS" "$@"
