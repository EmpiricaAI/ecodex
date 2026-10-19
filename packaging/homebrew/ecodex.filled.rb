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
  version "0.160.4"
  license "Apache-2.0"

  # The plugin's hooks shell out to the empirica CLI.
  depends_on "empiricaai/tap/empirica"

  on_macos do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-apple-darwin.tar.gz"
      sha256 "583807f9cdbad5ecb77c71b6f84936809c4ef70cc331cc367f3f33a2250466c3"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-apple-darwin.tar.gz"
      sha256 "6896ee7697515fc98652dd428a54897098b8089eef08260a4a86a93881ff77e3"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "4c32f8a40851c55c754270fbc13f2d768b1ec3599d5b2d17be615e5caa3a037b"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "de6568f34d9b712c9b18de747ce67787d266758ed395adde0ea3a2b5bb036fa9"
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
