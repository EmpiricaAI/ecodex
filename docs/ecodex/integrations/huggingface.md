# Hugging Face integration

ecodex can use Hugging Face in two independent ways:

- `hf skills add --global` installs the generated `hf-cli` skill under
  `~/.agents/skills`, which ecodex discovers as a user skill.
- The `huggingface` model provider sends native Responses API requests to Hugging
  Face Inference Providers at `https://router.huggingface.co/v1`.

The model provider needs a Hugging Face token. Installing and discovering the
locally generated `hf-cli` skill does not.

## Install the skill

Install or update the Hugging Face CLI, then install its generated skill:

```bash
hf --version
hf skills add --global
```

`hf skills add --global` writes
`$HOME/.agents/skills/hf-cli/SKILL.md`. The skill is generated from the installed
CLI version and does not require a login or network fetch. ecodex reads that
user-level root after its user config layer is active. Restart ecodex after
installing or updating a skill so a new host-skill snapshot is built.

To verify discovery without making an inference request:

```bash
ecodex -p huggingface debug prompt-input \
  "Use the Hugging Face CLI skill." | grep hf-cli
```

The repository check performs a stronger version of this test in an isolated
scratch `HOME`, and requires both the skill name and its scratch
`.agents/skills` root to be in the model-visible prompt:

```bash
python3 scripts/check_huggingface_integration.py --live \
  --hf "$(command -v hf)" \
  --ecodex "$(command -v ecodex)"
```

The check never writes to the real home directory or live ecodex config.

## Configure inference

The source installer copies `ecodex/huggingface.config.toml` to
`~/.codex/huggingface.config.toml` (the default `CODEX_HOME`) if the destination
does not already exist. Profile-v2 selection uses this separate file; a legacy
`[profiles.huggingface]` table in the main config is not selected by
`-p huggingface` on current ecodex.

For a source checkout without running the installer:

```bash
mkdir -p "${CODEX_HOME:-$HOME/.codex}"
cp ecodex/huggingface.config.toml \
  "${CODEX_HOME:-$HOME/.codex}/huggingface.config.toml"
```

Create a fine-grained token with the **Make calls to Inference Providers**
permission, then expose it only through the environment:

```bash
read -rsp "Hugging Face token: " HF_TOKEN
export HF_TOKEN
printf '\n'
ecodex exec -p huggingface "Reply with one sentence."
```

Do not put a token literal in TOML. Both bundled provider definitions use
`env_key = "HF_TOKEN"`; ecodex reads the value at request time and sends it as
a bearer token. With `HF_TOKEN` absent or empty, config and skill discovery
still load, while inference fails closed with an error naming the missing
environment variable.

The profile currently selects `openai/gpt-oss-120b:groq`, a model/provider pair
used in Hugging Face's Responses API guide. Override it for one command with a
Hub repository ID and optional routing suffix:

```bash
ecodex exec -p huggingface \
  -m "moonshotai/Kimi-K2-Instruct-0905:groq" \
  "Summarize this repository."
```

## Authentication boundaries

No token is needed for:

- `hf --version`, `hf --help`, and the locally generated `hf-cli` skill;
- `hf skills add --global` for the default `hf-cli` skill;
- ecodex config parsing and `debug prompt-input` skill discovery.

A token is needed for:

- inference through `router.huggingface.co`;
- private or gated Hub content;
- uploads, repository mutations, Jobs, Endpoints, and other account-scoped
  `hf` commands.

`hf auth login` stores a CLI credential for Hub commands, but ecodex inference
intentionally does not read that credential store. Export `HF_TOKEN` from a
secret manager for the ecodex process.

## Limitations

- Hugging Face's Responses API is currently documented as beta. Model support,
  tool behavior, routing availability, quotas, and credits remain provider- and
  account-dependent.
- `hf skills add <marketplace-name>` downloads a marketplace skill and can
  require network access. The no-name `hf skills add --global` path used here
  generates `hf-cli` locally.
- A shell may resolve an older or broken `hf` shim before the intended binary.
  Use `command -v hf` and `hf --version`; pass explicit binary paths to the
  repo-side live check when needed.
- The prebuilt binary installer does not seed ecodex configuration. Copy the
  profile as shown above, or add the `huggingface` provider and profile overlay
  to the active `CODEX_HOME` yourself.
- Skill catalogs are snapshotted for a running session. Restart ecodex after
  changing installed skills.

Primary references: [Hugging Face Responses API](https://huggingface.co/docs/inference-providers/en/guides/responses-api)
and [Hugging Face CLI for AI Agents](https://huggingface.co/docs/hub/agents-cli).
