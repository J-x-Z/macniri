#!/bin/sh
set -e

# macniri Launcher Script

# 1. Set Critical Environment Variables
export RUSTFLAGS="-L/opt/homebrew/lib -C link-arg=-fuse-ld=/opt/homebrew/bin/ld64.lld"
export XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-/tmp}

# 2. Check for Linker
if [ ! -f "/opt/homebrew/bin/ld64.lld" ]; then
    echo "❌ Error: ld64.lld linker not found!"
    echo "👉 Please run: brew install llvm"
    exit 1
fi

# 3. Build & Run
echo "🚀 Building and Launching macniri..."
cargo run --release -c ./debug_config.kdl
