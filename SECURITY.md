# Security Policy

ecodex is an agent runtime that executes code, runs shell commands, and connects to LLM providers. The security boundary matters. We take vulnerability reports seriously.

## Reporting a vulnerability

**Preferred:** [Open a private security advisory](https://github.com/EmpiricaAI/ecodex/security/advisories/new) on this repository. GitHub's private vulnerability reporting routes directly to maintainers without making the report public.

**Alternative:** Email `security@empirica-ai.com` if you can't use GitHub's flow.

Please **do not** open public issues, discuss on social media, or share PoCs publicly before we've had a chance to investigate. We aim to acknowledge new reports within 72 hours and to publish a fix or mitigation timeline within 14 days for confirmed issues.

When reporting, please include:

- A description of the vulnerability and how to reproduce it
- The version (`ecodex --version`) and install channel (install script / Homebrew / direct binary / cargo / source)
- Whether the issue affects ecodex-specific code (the empirica plugin, the translator, the installers, the default config) or upstream codex code we redistribute
- Any suggested mitigations or fixes

## What counts as a security issue

ecodex inherits codex's threat model and adds its own surfaces. The following are in scope:

| Surface | Examples |
|---|---|
| **Sandbox escape** | A tool call escapes the configured `landlock` / `seatbelt` / network restrictions; a plugin gains write access outside its declared `writableRoots`. |
| **Auth / secrets exposure** | API keys or session tokens leak into rollout files or logs, or are sent to the wrong provider; the keyring helper or the translator's key store exposes credentials. |
| **Plugin trust bypass** | An untrusted hook script executes without the plugin trust check; the `ECODEX_AUTO_TRUSTED_PLUGIN_IDS` allowlist is bypassed; a hook output forges a Sentinel decision. |
| **Sentinel firewall bypass** | A praxic tool call (edit, write, mutating shell) executes without a valid CHECK, or with a forged CHECK; the investigation-proportionality budget is bypassed in ways that hide work from calibration. |
| **Wire-protocol translator** | The translator passes through malformed provider responses that could cause memory unsafety in codex; SSE injection from a compromised provider triggers unintended tool calls. |
| **Install and provisioning integrity** | An installer, or ecodex's first-start provisioning of the plugin and config, writes outside its declared paths; checksum verification of downloaded binaries is bypassed. |
| **Supply chain** | A dependency in `Cargo.toml` is a known-malicious crate; an upstream codex commit we sync introduces a backdoor we should reject. |

The following are **out of scope**:

- Issues in the upstream OpenAI codex code that we redistribute unchanged — please report those to [openai/codex security](https://github.com/openai/codex/security)
- LLM "jailbreaks" or prompt injection that don't translate into a privilege boundary breach (those are model-vendor concerns)
- Issues in third-party providers (DeepSeek, Anthropic, OpenAI, etc.) that we connect to
- DoS via legitimate but expensive operations (large cargo builds, long-running sessions); resource-exhaustion bugs that are clearly accidental and not exploitable for privilege escalation

## Supported versions

ecodex is pre-1.0. Security fixes go into the **latest release**; older releases are best-effort. ecodex versions keep the upstream codex base's major.minor and count ecodex releases in the patch number, so "latest" is the highest patch on the newest base.

| Version | Supported |
|---|---|
| The latest release | ✅ |
| `main` between releases | ✅ (rolling) |
| Anything older | ❌ |

A fix lands on `main`, ships as a patch release through `scripts/release.sh`, and the advisory or CVE id is noted in the GitHub Security tab. `ecodex update` moves an install to the latest release.

## Coordinated disclosure

For high-severity issues we'll coordinate the public disclosure with you:

1. You file the private advisory
2. We acknowledge, scope the impact, and propose a fix timeline
3. We develop the fix on a private branch and write the advisory
4. We coordinate the publish window with you (typically aligned with the patch release)
5. After release, we credit you in the advisory unless you prefer anonymity

## Out-of-band

If you find an issue actively being exploited or with critical impact and can't wait for the standard flow, email `security@empirica-ai.com` with `URGENT` in the subject line.

---

Thank you for helping us keep ecodex safe.
