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
  version "0.170.0"
  license "Apache-2.0"

  # The plugin's hooks shell out to the empirica CLI.
  depends_on "empiricaai/tap/empirica"

  on_macos do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-apple-darwin.tar.gz"
      sha256 "ac6838234689b436894b7ff676605aee9afc0713fc05cc1dfcb48e2a10bcb899"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-apple-darwin.tar.gz"
      sha256 "46e43abca0532ebd3cb5c9019f17c679768170c0aa9c818c20c542f1d06ce66e"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "8842ad82e43452c4d963ea943b99626cc77145cb6fa57a506bff6c602df18fa8"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "9e0beb136ee7de2c1f4406520cae063d74268e54d32a99c4d439f27cf9d3842e"
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
