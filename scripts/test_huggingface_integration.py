import importlib.util
import shutil
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("check_huggingface_integration.py")
SPEC = importlib.util.spec_from_file_location("check_huggingface_integration", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
check = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(check)


def _fixture(parent: Path) -> Path:
    repo = parent / "repo"
    ecodex = repo / "ecodex"
    ecodex.mkdir(parents=True)
    shutil.copy2(check.DEFAULT_CONFIG, ecodex / "config.toml.default")
    shutil.copy2(check.PROFILE_CONFIG, ecodex / "huggingface.config.toml")
    return repo


class HuggingFaceIntegrationContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.scratch = tempfile.TemporaryDirectory(prefix="ecodex-hf-test-")
        self.repo = _fixture(Path(self.scratch.name))

    def tearDown(self) -> None:
        self.scratch.cleanup()

    def test_repo_contract_accepts_shipped_configs(self) -> None:
        self.assertEqual(check.check_repo_contract(self.repo), [])

    def test_chat_wire_api_cannot_manufacture_a_valid_provider(self) -> None:
        config = self.repo / "ecodex" / "config.toml.default"
        text = config.read_text(encoding="utf-8")
        anchor = '[model_providers.huggingface]\nname = "Hugging Face Inference Providers"'
        start = text.index(anchor)
        end = text.find("\n[", start + len(anchor))
        if end == -1:
            end = len(text)
        provider = text[start:end].replace('wire_api = "responses"', 'wire_api = "chat"')
        config.write_text(f"{text[:start]}{provider}{text[end:]}", encoding="utf-8")

        failures = check.check_repo_contract(self.repo)

        self.assertTrue(
            any("exact token-safe Hugging Face Responses provider" in item for item in failures)
        )

    def test_literal_token_cannot_replace_env_reference(self) -> None:
        profile = self.repo / "ecodex" / "huggingface.config.toml"
        text = profile.read_text(encoding="utf-8")
        profile.write_text(
            text.replace('env_key = "HF_TOKEN"', 'experimental_bearer_token = "secret"'),
            encoding="utf-8",
        )

        failures = check.check_repo_contract(self.repo)

        self.assertTrue(any("provider must match" in item for item in failures))

    def test_reserved_openai_override_cannot_pass(self) -> None:
        config = self.repo / "ecodex" / "config.toml.default"
        with config.open("a", encoding="utf-8") as handle:
            handle.write('\n[model_providers.openai]\nname = "OpenAI"\n')

        failures = check.check_repo_contract(self.repo)

        self.assertTrue(any("reserved built-in provider ID 'openai'" in item for item in failures))


if __name__ == "__main__":
    unittest.main()
