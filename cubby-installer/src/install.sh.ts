export const installScript = `#!/bin/sh
set -e

# cubby installer script - downloads and installs the cubby binary
# Colors for output
RED='\\033[0;31m'
GREEN='\\033[0;32m'
YELLOW='\\033[1;33m'
NC='\\033[0m' # No Color

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux*)
    OS_TYPE="linux"
    ;;
  Darwin*)
    OS_TYPE="darwin"
    ;;
  *)
    echo "\${RED}Unsupported operating system: $OS\${NC}"
    echo "Cubby currently supports macOS and Linux only."
    exit 1
    ;;
esac

case "$ARCH" in
  x86_64|amd64)
    ARCH_TYPE="x86_64"
    ;;
  arm64|aarch64)
    ARCH_TYPE="aarch64"
    ;;
  *)
    echo "\${RED}Unsupported architecture: $ARCH\${NC}"
    exit 1
    ;;
esac

# Construct binary name
BINARY_NAME="cubby-\${OS_TYPE}-\${ARCH_TYPE}"
FINAL_NAME="cubby"

# Download binary
DOWNLOAD_URL="https://get.cubby.sh/binaries/$BINARY_NAME"
TMP_FILE="/tmp/cubby_install_$$"

# Spinner function
spin() {
  local spin=("-" "\\\\" "|" "/")
  local i=0
  while kill -0 $1 2>/dev/null; do
    printf "\b\${spin[i]}"
    i=$(( (i+1) % 4 ))
    sleep 0.1
  done
  printf "\b"
}

# Start download in background
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$DOWNLOAD_URL" -o "$TMP_FILE" &
elif command -v wget >/dev/null 2>&1; then
  wget -q "$DOWNLOAD_URL" -O "$TMP_FILE" &
else
  echo "\${RED}Error: curl or wget is required\${NC}"
  exit 1
fi

# Get the PID of the download process
pid=$!

# Show spinner while downloading
echo "\${GREEN} downloading cubby cli... "
spin $pid

# Make binary executable
chmod +x "$TMP_FILE"

# Install to user's local bin (no sudo required)
INSTALL_DIR="$HOME/.local/bin"
mkdir -p "$INSTALL_DIR"
mv "$TMP_FILE" "$INSTALL_DIR/$FINAL_NAME"

# Check if ~/.local/bin is in PATH
if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
  echo "\${YELLOW} $INSTALL_DIR is not in your PATH\${NC}"
  echo "Add this to your ~/.bashrc or ~/.zshrc:"
  echo "  export PATH=\\"\\\$HOME/.local/bin:\\\$PATH\\""
  echo ""
fi


# Run cubby setup
"$INSTALL_DIR/$FINAL_NAME" setup
`;
