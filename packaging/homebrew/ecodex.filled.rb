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
  version "0.164.1"
  license "Apache-2.0"

  # The plugin's hooks shell out to the empirica CLI.
  depends_on "empiricaai/tap/empirica"

  on_macos do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-apple-darwin.tar.gz"
      sha256 "be18671e37e5e59a9b96fa1dabfdbb9ed1f725740dd7e7e74922bf72465b162e"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-apple-darwin.tar.gz"
      sha256 "c17c81808b1fb94776473bd00eb5d95a9d08f14b98874593ef2e28ceb46905fd"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "c01b58071c04d53462d36ded58c9f9f10dc8bab4e80415166519e6a4ee99ffbc"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "f60804f0628c76ccde5f5ad65b896725541b6ae79ca3dd5d46d0924cc232ce46"
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
