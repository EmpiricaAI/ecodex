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
  version "0.153.4"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-apple-darwin.tar.gz"
      sha256 "c3d64a069bdc9e92021877e3045bff2cedd943496734c5d97267ffb7d39bc6c9"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-apple-darwin.tar.gz"
      sha256 "51d2b676dc38f7201c4696cf5585ff90fafe5df64ac6af28c30e9675d94dc9ec"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "cc5842994cbef7ec3c9532945c7146b9d11e7db12053ddacf73fa077dc188dff"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "6321cfd824fec3ec6253edc645567acb6bf3ce056ad0490175f1babf3714dcaf"
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
