#!/usr/bin/env bash
# ecodex prebuilt-binary installer — NO Rust toolchain, NO compile.
#
#   curl -fsSL https://raw.githubusercontent.com/EmpiricaAI/ecodex/main/scripts/install.sh | bash
#
# Detects your OS + CPU, downloads the matching stripped release tarball from
# GitHub Releases, verifies its SHA-256, and installs the ecodex binaries into
# ~/.local/bin (override with ECODEX_INSTALL_DIR or --prefix DIR). Pin a
# version with ECODEX_VERSION=v0.2.6 (default: latest release).
#
# This is the fast path for non-developers. Developers who want a source build
# (and config seeding) can use ecodex/scripts/install.sh instead.
set -euo pipefail

REPO="EmpiricaAI/ecodex"
INSTALL_DIR="${ECODEX_INSTALL_DIR:-${HOME}/.local/bin}"
VERSION="${ECODEX_VERSION:-latest}"

while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) INSTALL_DIR="$2"; shift 2 ;;
    --version) VERSION="$2"; shift 2 ;;
    -h|--help)
      grep '^#' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

err()  { printf 'ecodex-install: %s\n' "$*" >&2; exit 1; }
info() { printf 'ecodex-install: %s\n' "$*"; }

for tool in curl tar shasum uname; do
  command -v "$tool" >/dev/null 2>&1 || command -v sha256sum >/dev/null 2>&1 || err "missing required tool: $tool"
done

# --- detect platform → release target triple -------------------------------
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux)  os_part="unknown-linux-gnu" ;;
  Darwin) os_part="apple-darwin" ;;
  *) err "unsupported OS: $os (Linux and macOS only; on Windows use WSL)" ;;
esac
case "$arch" in
  x86_64|amd64)  arch_part="x86_64" ;;
  arm64|aarch64) arch_part="aarch64" ;;
  *) err "unsupported CPU architecture: $arch" ;;
esac
TARGET="${arch_part}-${os_part}"

# --- resolve version -------------------------------------------------------
if [ "$VERSION" = "latest" ]; then
  info "resolving latest release…"
  VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep -m1 '"tag_name"' | sed -E 's/.*"tag_name" *: *"([^"]+)".*/\1/')"
  [ -n "$VERSION" ] || err "could not resolve latest release tag"
fi
info "installing ecodex ${VERSION} for ${TARGET}"

ARCHIVE="ecodex-${TARGET}.tar.gz"
BASE="https://github.com/${REPO}/releases/download/${VERSION}"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# --- download + verify checksum --------------------------------------------
info "downloading ${ARCHIVE}…"
curl -fSL --proto '=https' --tlsv1.2 -o "${TMP}/${ARCHIVE}" "${BASE}/${ARCHIVE}" \
  || err "download failed — no prebuilt binary for ${TARGET} at ${VERSION}? See ${BASE}"
if curl -fsSL -o "${TMP}/${ARCHIVE}.sha256" "${BASE}/${ARCHIVE}.sha256" 2>/dev/null; then
  info "verifying checksum…"
  ( cd "$TMP"
    expected="$(awk '{print $1}' "${ARCHIVE}.sha256")"
    if command -v sha256sum >/dev/null 2>&1; then
      actual="$(sha256sum "${ARCHIVE}" | awk '{print $1}')"
    else
      actual="$(shasum -a 256 "${ARCHIVE}" | awk '{print $1}')"
    fi
    [ "$expected" = "$actual" ] || err "checksum mismatch — refusing to install (expected $expected, got $actual)"
  )
else
  info "WARNING: no .sha256 published for this artifact; skipping checksum verification"
fi

# --- extract + install ------------------------------------------------------
tar -xzf "${TMP}/${ARCHIVE}" -C "$TMP"
mkdir -p "$INSTALL_DIR"
for bin in ecodex codex-empirica-plugin codex-empirica-translator; do
  [ -f "${TMP}/${bin}" ] || err "archive missing expected binary: ${bin}"
  install -m 0755 "${TMP}/${bin}" "${INSTALL_DIR}/${bin}"
  info "installed ${INSTALL_DIR}/${bin}"
done

info "done — ecodex ${VERSION} installed to ${INSTALL_DIR}"
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) : ;;
  *) info "NOTE: ${INSTALL_DIR} is not on your PATH. Add:  export PATH=\"${INSTALL_DIR}:\$PATH\"" ;;
esac
cat <<'EOF'

Next steps:
  • ecodex --version            # confirm the install
  • The empirica CLI is a separate dependency for the epistemic plugin:
      https://github.com/EmpiricaAI/empirica   (without it the plugin fails quiet)
  • Chat providers (Mistral/Devstral, etc.) route through the translator:
      run  ecodex-translator  (or scripts/ecodex-translator.sh) before launching.
EOF
