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
  version "0.164.0"
  license "Apache-2.0"

  # The plugin's hooks shell out to the empirica CLI.
  depends_on "empiricaai/tap/empirica"

  on_macos do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-apple-darwin.tar.gz"
      sha256 "f9dfc4f10b5c68b1ed056c2c9dee650f95ea9c0514d9eaf79e9720a6573a4771"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-apple-darwin.tar.gz"
      sha256 "8beaf3c772b959b5cd24a8ea5a21e32e0317010c4891deff95c80274a38088be"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "9713914932a5bfe2c4cbc5b82ce35cf1a18e46aa12bbc661066a229bb74fc5f5"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "18cc021d3bc111d2373e6171187eb4e2e4c5fe9a17294da9908e3d1ba79d9336"
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
