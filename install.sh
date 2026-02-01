#!/bin/bash
set -e

# SyncForge TUI Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/nanablast/syncforge-tui/master/install.sh | bash

REPO="nanablast/syncforge-tui"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Detect OS and architecture
detect_platform() {
    OS=$(uname -s | tr '[:upper:]' '[:lower:]')
    ARCH=$(uname -m)

    case "$OS" in
        linux)
            case "$ARCH" in
                x86_64) PLATFORM="linux-x86_64" ;;
                *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
            esac
            ;;
        darwin)
            case "$ARCH" in
                x86_64) PLATFORM="macos-x86_64" ;;
                arm64) PLATFORM="macos-aarch64" ;;
                *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
            esac
            ;;
        *)
            echo "Unsupported OS: $OS"
            exit 1
            ;;
    esac
}

# Get latest release version
get_latest_version() {
    curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/'
}

# Download and install
install() {
    detect_platform
    VERSION=$(get_latest_version)

    if [ -z "$VERSION" ]; then
        echo "Error: Could not determine latest version"
        exit 1
    fi

    echo "Installing SyncForge TUI $VERSION for $PLATFORM..."

    DOWNLOAD_URL="https://github.com/$REPO/releases/download/$VERSION/syncforge-tui-$PLATFORM"
    TMP_FILE=$(mktemp)

    echo "Downloading from $DOWNLOAD_URL..."
    curl -fsSL "$DOWNLOAD_URL" -o "$TMP_FILE"
    chmod +x "$TMP_FILE"

    # Check if we need sudo
    if [ -w "$INSTALL_DIR" ]; then
        mv "$TMP_FILE" "$INSTALL_DIR/syncforge-tui"
    else
        echo "Need sudo to install to $INSTALL_DIR"
        sudo mv "$TMP_FILE" "$INSTALL_DIR/syncforge-tui"
    fi

    echo ""
    echo "SyncForge TUI $VERSION installed successfully!"
    echo "Run 'syncforge-tui' to start."
}

install
