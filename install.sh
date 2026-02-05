#!/bin/bash
# fdmon installer script
# Usage: curl -fsSL https://raw.githubusercontent.com/ll931217/fdmon/master/install.sh | bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
REPO="ll931217/fdmon"
BINARY_NAME="fdmon"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Detect OS and architecture
detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)

    case "$os" in
        linux*)
            OS="linux"
            ;;
        darwin*)
            OS="darwin"
            ;;
        *)
            echo -e "${RED}Unsupported operating system: $os${NC}"
            exit 1
            ;;
    esac

    case "$arch" in
        x86_64|amd64)
            ARCH="x86_64"
            ;;
        aarch64|arm64)
            ARCH="aarch64"
            ;;
        *)
            echo -e "${RED}Unsupported architecture: $arch${NC}"
            exit 1
            ;;
    esac

    PLATFORM="${OS}-${ARCH}"
}

# Get latest release version
get_latest_version() {
    echo -e "${YELLOW}Fetching latest release...${NC}"
    VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')

    if [ -z "$VERSION" ]; then
        echo -e "${RED}Failed to fetch latest version${NC}"
        exit 1
    fi

    echo -e "${GREEN}Latest version: $VERSION${NC}"
}

# Download and install binary
install_binary() {
    local download_url="https://github.com/$REPO/releases/download/$VERSION/fdmon-$PLATFORM"
    local tmp_file="/tmp/fdmon-$$.tmp"

    echo -e "${YELLOW}Downloading fdmon for $PLATFORM...${NC}"

    if ! curl -fsSL "$download_url" -o "$tmp_file"; then
        echo -e "${RED}Failed to download binary${NC}"
        echo -e "${YELLOW}URL: $download_url${NC}"
        exit 1
    fi

    # Create install directory if it doesn't exist
    mkdir -p "$INSTALL_DIR"

    # Move binary to install directory
    mv "$tmp_file" "$INSTALL_DIR/$BINARY_NAME"
    chmod +x "$INSTALL_DIR/$BINARY_NAME"

    echo -e "${GREEN}✓ Installed to: $INSTALL_DIR/$BINARY_NAME${NC}"
}

# Check if install directory is in PATH
check_path() {
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        echo ""
        echo -e "${YELLOW}Warning: $INSTALL_DIR is not in your PATH${NC}"
        echo "Add the following to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
        echo ""
        echo "    export PATH=\"\$PATH:$INSTALL_DIR\""
        echo ""
    fi
}

# Verify installation
verify_installation() {
    if command -v "$BINARY_NAME" &> /dev/null; then
        local installed_version=$("$BINARY_NAME" --version 2>&1 || echo "unknown")
        echo -e "${GREEN}✓ Installation successful!${NC}"
        echo "Run 'fdmon --help' to get started"
    else
        echo -e "${GREEN}✓ Binary installed at $INSTALL_DIR/$BINARY_NAME${NC}"
        echo "Run '$INSTALL_DIR/$BINARY_NAME --help' to get started"
    fi
}

# Main installation flow
main() {
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "  fdmon - File Descriptor Monitor"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo ""

    detect_platform
    get_latest_version
    install_binary
    check_path
    verify_installation

    echo ""
    echo -e "${GREEN}Installation complete!${NC}"
}

main "$@"
