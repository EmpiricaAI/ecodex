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
  version "__VERSION__"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-apple-darwin.tar.gz"
      sha256 "__SHA256_AARCH64_APPLE_DARWIN__"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-apple-darwin.tar.gz"
      sha256 "__SHA256_X86_64_APPLE_DARWIN__"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-aarch64-unknown-linux-musl.tar.gz"
      sha256 "__SHA256_AARCH64_UNKNOWN_LINUX_MUSL__"
    end
    on_intel do
      url "https://github.com/EmpiricaAI/ecodex/releases/download/v#{version}/ecodex-x86_64-unknown-linux-musl.tar.gz"
      sha256 "__SHA256_X86_64_UNKNOWN_LINUX_MUSL__"
    end
  end

  def install
    # Tarball contains the three binaries at its root.
    bin.install "ecodex", "codex-empirica-plugin", "codex-empirica-translator"
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
