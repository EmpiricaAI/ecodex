# Homebrew formula for ecodex — PREBUILT BINARY (no compile, no Rust toolchain).
#
# This is the authoritative source; `scripts/sync-homebrew.sh <version>` fills
# the per-platform SHA-256 values from the GitHub Release artifacts and copies
# this file into the EmpiricaAI/homebrew-tap repo (Formula/ecodex.rb).
#
# The __SHA256_*__ tokens are placeholders replaced by the sync script. Do not
# hand-edit them — re-run the sync script after a release.
class Ecodex < Formula
  desc "Empirica-native fork of OpenAI Codex — calibrated agentic coding CLI"
  homepage "https://github.com/EmpiricaAI/ecodex"
  version "0.160.3"
  license "Apache-2.0"

  # The plugin's hooks shell out to the empirica CLI.
  depends_on "empiricaai/tap/empirica"

  on_macos do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-apple-darwin.tar.gz"
      sha256 "b2dbe01a5e24748eda8dde27c5782fe45ecef2c8e2440f925895672d1e70ec87"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-apple-darwin.tar.gz"
      sha256 "df83dd5fb248d803535ea2c0fced9cabee9690e3e4772fc9d99ff876c36d859e"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "8592c0a00156b471de54b7bdc0f8b18c1aa2ead833e733d3f1071b5879c7bba0"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "5e804a157d50438e2ee2f5cdc41310966e83e2e35cbe0743120d269dbbc7f298"
    end
  end

  def install
    # Tarball contains the four binaries at its root.
    bin.install "ecodex", "codex-empirica-plugin", "codex-empirica-translator", "codex-code-mode-host"
  end

  def caveats
    <<~EOS
      The first `ecodex` session installs the empirica plugin and a curated
      ~/.codex/config.toml.
      Mistral/Devstral route through the translator: store the key under
      mistral.api_key in ~/.empirica/credentials.yaml, then run
      `codex-empirica-translator`.
    EOS
  end

  test do
    assert_match "ecodex", shell_output("#{bin}/ecodex --version")
  end
end
