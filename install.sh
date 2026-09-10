#!/bin/sh
# Installs the latest mlc release binary into ~/.local/bin. No Rust toolchain needed.
#   curl -sSL https://raw.githubusercontent.com/oneortwo/mlc-cli/main/install.sh | sh
set -e

REPO="oneortwo/mlc-cli"
BINARY="mlc"
INSTALL_DIR="${MLC_INSTALL_DIR:-${HOME}/.local/bin}"

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
    darwin) OS="apple-darwin" ;;
    linux) OS="unknown-linux-gnu" ;;
    *) echo "error: unsupported OS: $OS (download a binary from https://github.com/${REPO}/releases)" >&2; exit 1 ;;
esac

case "$ARCH" in
    x86_64|amd64) ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *) echo "error: unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

TARGET="${ARCH}-${OS}"
LATEST=$(curl -sSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
if [ -z "$LATEST" ]; then
    echo "error: could not determine the latest release" >&2
    exit 1
fi

BASE_URL="https://github.com/${REPO}/releases/download/${LATEST}"
ARCHIVE="${BINARY}-${TARGET}.tar.gz"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

echo "Downloading ${BINARY} ${LATEST} for ${TARGET}..."
curl -sSL -o "${TMP_DIR}/${ARCHIVE}" "${BASE_URL}/${ARCHIVE}"
curl -sSL -o "${TMP_DIR}/SHA256SUMS" "${BASE_URL}/SHA256SUMS"

# Verify the checksum when a SHA-256 tool is available.
EXPECTED=$(grep " ${ARCHIVE}\$" "${TMP_DIR}/SHA256SUMS" | cut -d ' ' -f 1)
if command -v sha256sum >/dev/null 2>&1; then
    ACTUAL=$(sha256sum "${TMP_DIR}/${ARCHIVE}" | cut -d ' ' -f 1)
elif command -v shasum >/dev/null 2>&1; then
    ACTUAL=$(shasum -a 256 "${TMP_DIR}/${ARCHIVE}" | cut -d ' ' -f 1)
else
    ACTUAL=""
fi
if [ -n "$ACTUAL" ] && [ -n "$EXPECTED" ] && [ "$ACTUAL" != "$EXPECTED" ]; then
    echo "error: checksum mismatch for ${ARCHIVE}" >&2
    exit 1
fi

mkdir -p "$INSTALL_DIR"
tar xzf "${TMP_DIR}/${ARCHIVE}" -C "$INSTALL_DIR"
chmod +x "${INSTALL_DIR}/${BINARY}"
echo "Installed ${BINARY} to ${INSTALL_DIR}/${BINARY}"

case ":$PATH:" in
    *":${INSTALL_DIR}:"*) ;;
    *) echo "Add ${INSTALL_DIR} to your PATH to use ${BINARY} from anywhere." ;;
esac

MLC="${INSTALL_DIR}/${BINARY}"
if command -v fish >/dev/null 2>&1; then
    mkdir -p "${HOME}/.config/fish/completions"
    "$MLC" completions fish > "${HOME}/.config/fish/completions/${BINARY}.fish"
    echo "Fish completions installed."
fi
if command -v bash >/dev/null 2>&1; then
    mkdir -p "${HOME}/.local/share/bash-completion/completions"
    "$MLC" completions bash > "${HOME}/.local/share/bash-completion/completions/${BINARY}"
    echo "Bash completions installed."
fi
if command -v zsh >/dev/null 2>&1; then
    mkdir -p "${HOME}/.zfunc"
    "$MLC" completions zsh > "${HOME}/.zfunc/_${BINARY}"
    echo "Zsh completions installed (add ~/.zfunc to fpath if it is not already)."
fi

echo "Next: run '${BINARY} auth setup' to save your MLC credentials, then '${BINARY} doctor'."
