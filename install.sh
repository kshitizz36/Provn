#!/usr/bin/env bash
set -euo pipefail

REPO="kshitizz36/Provn"
BIN_NAME="provn"
INSTALL_DIR="/usr/local/bin"

# ── Detect OS and arch ────────────────────────────────────────────────────────

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin)
    case "$ARCH" in
      arm64)  ARTIFACT="provn-aarch64-apple-darwin" ;;
      x86_64) ARTIFACT="provn-x86_64-apple-darwin"  ;;
      *)      echo "Unsupported macOS arch: $ARCH"; exit 1 ;;
    esac
    EXT=".tar.gz"
    ;;
  Linux)
    case "$ARCH" in
      x86_64)          ARTIFACT="provn-x86_64-linux"  ;;
      aarch64|arm64)   ARTIFACT="provn-aarch64-linux"  ;;
      *)               echo "Unsupported Linux arch: $ARCH"; exit 1 ;;
    esac
    EXT=".tar.gz"
    ;;
  *)
    echo "Unsupported OS: $OS"
    echo "On Windows, download provn-x86_64-windows.zip from:"
    echo "  https://github.com/$REPO/releases/latest"
    exit 1
    ;;
esac

# ── Fetch latest release tag ──────────────────────────────────────────────────

echo "Fetching latest Provn release..."
TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
  | grep '"tag_name"' \
  | head -1 \
  | sed 's/.*"tag_name": "\(.*\)".*/\1/')

if [ -z "$TAG" ]; then
  echo "Could not fetch latest release. Check https://github.com/$REPO/releases"
  exit 1
fi

echo "Installing Provn $TAG..."

# ── Download and verify ───────────────────────────────────────────────────────

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

URL="https://github.com/$REPO/releases/download/$TAG/${ARTIFACT}${EXT}"
SHA_URL="${URL}.sha256"

curl -fsSL "$URL"     -o "$TMP/provn.tar.gz"
curl -fsSL "$SHA_URL" -o "$TMP/provn.tar.gz.sha256" 2>/dev/null || true

# Verify checksum if available
if [ -f "$TMP/provn.tar.gz.sha256" ]; then
  EXPECTED=$(awk '{print $1}' "$TMP/provn.tar.gz.sha256")
  if command -v sha256sum &>/dev/null; then
    ACTUAL=$(sha256sum "$TMP/provn.tar.gz" | awk '{print $1}')
  elif command -v shasum &>/dev/null; then
    ACTUAL=$(shasum -a 256 "$TMP/provn.tar.gz" | awk '{print $1}')
  fi
  if [ "${EXPECTED}" != "${ACTUAL:-skip}" ]; then
    echo "SHA-256 mismatch — aborting install"
    exit 1
  fi
fi

tar xzf "$TMP/provn.tar.gz" -C "$TMP"

# ── Install ───────────────────────────────────────────────────────────────────

if [ -w "$INSTALL_DIR" ]; then
  mv "$TMP/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
  chmod +x "$INSTALL_DIR/$BIN_NAME"
else
  sudo mv "$TMP/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
  sudo chmod +x "$INSTALL_DIR/$BIN_NAME"
fi

echo ""
echo "  ✓  Provn $TAG installed → $INSTALL_DIR/$BIN_NAME"
echo ""
echo "  Next steps:"
echo "    cd your-repo"
echo "    provn install          # add pre-commit hook"
echo "    provn                  # view status dashboard"
echo ""
