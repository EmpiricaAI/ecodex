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
  version "0.147.1"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-apple-darwin.tar.gz"
      sha256 "67259187cddc352e5a35196c06513c9bc9419b7904a36a9e08868bb4ebf3c22e"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-apple-darwin.tar.gz"
      sha256 "fdc4bbc741d7c236269a402854cae4a7a96988cccaf18d779dca5a980905296e"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "4b53b7d89439bc679df906fa7d73beb9032a4c8f1a1d8f75c4656b3258045f9a"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "6cc95a73ba667854192525041125f39f0c513b5d2b4b163f03edf3f7531fe8fa"
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
    assert_match "codex-cli", shell_output("#{bin}/ecodex --version")
  end
end
