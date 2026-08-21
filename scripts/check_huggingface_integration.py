#!/usr/bin/env python3
"""Check ecodex's repo and live Hugging Face integration contracts."""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_CONFIG = REPO / "ecodex" / "config.toml.default"
PROFILE_CONFIG = REPO / "ecodex" / "huggingface.config.toml"
EXPECTED_PROVIDER = {
    "name": "Hugging Face Inference Providers",
    "base_url": "https://router.huggingface.co/v1",
    "env_key": "HF_TOKEN",
    "env_key_instructions": (
        "Create a fine-grained token with Inference Providers permission at "
        "https://huggingface.co/settings/tokens, then export HF_TOKEN in your shell."
    ),
    "wire_api": "responses",
}


def _load_toml(path: Path) -> dict:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def check_repo_contract(repo: Path = REPO) -> list[str]:
    failures = []
    default_path = repo / "ecodex" / "config.toml.default"
    profile_path = repo / "ecodex" / "huggingface.config.toml"
    for path in (default_path, profile_path):
        if not path.is_file():
            failures.append(f"missing integration config: {path}")
    if failures:
        return failures

    try:
        default = _load_toml(default_path)
        profile = _load_toml(profile_path)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        return [f"cannot parse Hugging Face integration config: {exc}"]

    default_providers = default.get("model_providers", {})
    default_provider = default_providers.get("huggingface")
    profile_provider = profile.get("model_providers", {}).get("huggingface")
    if "openai" in default_providers:
        failures.append(
            "config.toml.default must not redefine reserved built-in provider ID 'openai'"
        )
    if default_provider != EXPECTED_PROVIDER:
        failures.append(
            "config.toml.default must contain the exact token-safe Hugging Face "
            f"Responses provider contract; got {default_provider!r}"
        )
    if profile_provider != EXPECTED_PROVIDER:
        failures.append(
            "huggingface.config.toml provider must match config.toml.default; "
            f"got {profile_provider!r}"
        )
    if profile.get("model_provider") != "huggingface":
        failures.append("huggingface.config.toml must select model_provider = 'huggingface'")
    model = profile.get("model")
    if not isinstance(model, str) or "/" not in model:
        failures.append("huggingface.config.toml must select a Hugging Face repository model ID")
    return failures


def _run(command: list[str], *, cwd: Path, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=cwd,
        env=env,
        text=True,
        capture_output=True,
        timeout=45,
        check=False,
    )


def check_live_contract(ecodex: str, hf: str) -> list[str]:
    failures = []
    with tempfile.TemporaryDirectory(prefix="ecodex-hf-") as scratch_text:
        scratch = Path(scratch_text)
        home = scratch / "home"
        codex_home = home / ".codex"
        workspace = scratch / "workspace"
        codex_home.mkdir(parents=True)
        workspace.mkdir()
        shutil.copy2(DEFAULT_CONFIG, codex_home / "config.toml")
        shutil.copy2(PROFILE_CONFIG, codex_home / "huggingface.config.toml")

        env = os.environ.copy()
        env.update(
            {
                "HOME": str(home),
                "CODEX_HOME": str(codex_home),
                "CODEX_SQLITE_HOME": str(codex_home),
                "HF_HOME": str(home / ".cache" / "huggingface"),
            }
        )
        env.pop("HF_TOKEN", None)
        env.pop("HUGGING_FACE_HUB_TOKEN", None)

        install = _run(
            [hf, "skills", "add", "--global", "--force", "--format", "json"],
            cwd=workspace,
            env=env,
        )
        skill_path = home / ".agents" / "skills" / "hf-cli" / "SKILL.md"
        if install.returncode != 0:
            failures.append(
                "hf skills add --global failed in scratch HOME: "
                f"exit {install.returncode}: {install.stderr.strip()}"
            )
            return failures
        if not skill_path.is_file():
            failures.append(f"hf skills add did not create {skill_path}")
            return failures
        skill_text = skill_path.read_text(encoding="utf-8")
        if not skill_text.startswith("---\nname: hf-cli\n"):
            failures.append("generated hf-cli SKILL.md has incompatible YAML frontmatter")

        prompt = _run(
            [
                ecodex,
                "-p",
                "huggingface",
                "debug",
                "prompt-input",
                "Use the Hugging Face CLI skill.",
            ],
            cwd=workspace,
            env=env,
        )
        if prompt.returncode != 0:
            failures.append(
                "ecodex debug prompt-input failed in scratch HOME: "
                f"exit {prompt.returncode}: {prompt.stderr.strip()}"
            )
        else:
            try:
                prompt_json = json.loads(prompt.stdout)
            except json.JSONDecodeError as exc:
                failures.append(f"ecodex debug prompt-input returned invalid JSON: {exc}")
            else:
                rendered = json.dumps(prompt_json)
                skill_root = skill_path.parent.parent
                if "hf-cli" not in rendered or str(skill_root) not in rendered:
                    failures.append(
                        "ecodex model-visible prompt omitted the scratch HOME hf-cli skill"
                    )

        auth = _run(
            [
                ecodex,
                "exec",
                "-p",
                "huggingface",
                "--ephemeral",
                "--skip-git-repo-check",
                "--ignore-rules",
                "Return the word ready.",
            ],
            cwd=workspace,
            env=env,
        )
        auth_output = f"{auth.stdout}\n{auth.stderr}"
        if auth.returncode == 0:
            failures.append("tokenless ecodex exec unexpectedly succeeded")
        if "HF_TOKEN" not in auth_output:
            failures.append(
                "tokenless ecodex exec did not reach the expected HF_TOKEN auth rejection"
            )
        wrong_failures = (
            "profile config file not found",
            "model provider `huggingface` not found",
            "wire_api",
        )
        for marker in wrong_failures:
            if marker.lower() in auth_output.lower():
                failures.append(f"tokenless ecodex exec failed for the wrong reason: {marker}")
    return failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--live", action="store_true", help="run scratch-HOME CLI checks")
    parser.add_argument("--ecodex", default="ecodex", help="ecodex executable")
    parser.add_argument("--hf", default="hf", help="hf executable")
    args = parser.parse_args()

    failures = check_repo_contract()
    if args.live and not failures:
        failures.extend(check_live_contract(args.ecodex, args.hf))
    if failures:
        print("✗ Hugging Face integration check FAILED:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1
    scope = "repo + scratch-HOME live" if args.live else "repo"
    print(f"✓ Hugging Face integration contract holds ({scope}).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
