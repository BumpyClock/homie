#!/bin/bash
set -e

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BINARY_NAME="homie-gateway"
SOURCE_PATH="$PROJECT_ROOT/target/release/$BINARY_NAME"
DEST_PATH="$HOME/.local/bin/$BINARY_NAME"

# Build release
echo "Building release version..."
cd "$PROJECT_ROOT"
cargo build --release

# Check if binary was built
if [[ ! -f "$SOURCE_PATH" ]]; then
    echo "Error: Binary not found at $SOURCE_PATH"
    exit 1
fi

# Create ~/.local/bin if it doesn't exist
mkdir -p "$HOME/.local/bin"

# Check if file exists and prompt for overwrite
if [[ -f "$DEST_PATH" ]]; then
    read -p "File already exists at $DEST_PATH. Overwrite? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Aborted."
        exit 0
    fi
fi

# Copy binary
cp "$SOURCE_PATH" "$DEST_PATH"
chmod +x "$DEST_PATH"

echo "Installed $BINARY_NAME to $DEST_PATH"
