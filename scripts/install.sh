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

for tool in curl tar uname; do
  command -v "$tool" >/dev/null 2>&1 || err "missing required tool: $tool"
done
# Checksum verification needs either shasum (macOS) or sha256sum (Linux).
command -v shasum >/dev/null 2>&1 || command -v sha256sum >/dev/null 2>&1 \
  || err "missing checksum tool: need shasum or sha256sum"

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
  # Capture curl's full output into a variable FIRST, then grep it. Piping
  # curl straight into `grep -m1` breaks under `set -o pipefail`: grep exits
  # (and closes its end of the pipe) as soon as it finds the first match,
  # while curl is often still writing -- curl sees that as a write failure
  # ("curl: (23) Failure writing output to destination") and the whole
  # pipeline's exit status (curl's, under pipefail) kills the script via -e,
  # even though grep+sed already got what they needed.
  latest_json="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest")"
  VERSION="$(printf '%s' "$latest_json" \
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
curl -fsSL -o "${TMP}/${ARCHIVE}.sha256" "${BASE}/${ARCHIVE}.sha256" \
  || err "checksum download failed — refusing to install an unverified artifact"
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

# --- extract + install ------------------------------------------------------
tar -xzf "${TMP}/${ARCHIVE}" -C "$TMP"
mkdir -p "$INSTALL_DIR"
for bin in ecodex codex-empirica-plugin codex-empirica-translator codex-code-mode-host; do
  [ -f "${TMP}/${bin}" ] || err "archive missing expected binary: ${bin}"
  install -m 0755 "${TMP}/${bin}" "${INSTALL_DIR}/${bin}"
  info "installed ${INSTALL_DIR}/${bin}"
done

info "done — ecodex ${VERSION} installed to ${INSTALL_DIR}"
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) : ;;
  *) info "NOTE: ${INSTALL_DIR} is not on your PATH. Add:  export PATH=\"${INSTALL_DIR}:\$PATH\"" ;;
esac
# --- empirica CLI (the plugin's hooks shell out to it) ------------------------
if command -v empirica >/dev/null 2>&1; then
  :
elif command -v pipx >/dev/null 2>&1; then
  info "installing the empirica CLI (pipx install empirica)…"
  pipx install empirica >/dev/null || info "WARNING: pipx install empirica failed — run it yourself"
elif command -v uv >/dev/null 2>&1; then
  info "installing the empirica CLI (uv tool install empirica)…"
  uv tool install empirica >/dev/null || info "WARNING: uv tool install empirica failed — run it yourself"
else
  info "NOTE: the empirica CLI is missing and neither pipx nor uv is available."
  info "      Install it with:  pipx install empirica   (the plugin's hooks need it)"
fi

cat <<'EOF'

Next steps:
  • Run  ecodex  — the first session installs the empirica plugin and a curated
      ~/.codex/config.toml. Later upgrades:  ecodex update
  • Mistral / Devstral: store the key under mistral.api_key in
      ~/.empirica/credentials.yaml, then run  codex-empirica-translator
EOF
