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
  version "0.157.0"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-apple-darwin.tar.gz"
      sha256 "5038a878d31dbc9edab622a40b68997ff8baab65409d44735db70efb1104bde0"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-apple-darwin.tar.gz"
      sha256 "1dccd4bf8c6cc14ed49689d33d45b909ec72802df29b28d6ce4833076d639ec8"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "872f8ed76e77c6d8220522d60e3f6537c48794485cec3807d27a89738c99e630"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "1e379892a1b011ac807d15161748ee1f7e4bd6974683ef0b8525a1a8e001123b"
    end
  end

  def install
    # Tarball contains the four binaries at its root.
    bin.install "ecodex", "codex-empirica-plugin", "codex-empirica-translator", "codex-code-mode-host"
  end

  def caveats
    <<~EOS
      ecodex's epistemic plugin needs the empirica CLI on PATH:
        https://github.com/EmpiricaAI/empirica
      Chat providers (Mistral/Devstral, etc.) route through the translator:
        run `codex-empirica-translator` before launching ecodex.
    EOS
  end

  test do
    assert_match "ecodex", shell_output("#{bin}/ecodex --version")
  end
end
