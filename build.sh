#!/bin/bash
set -euo pipefail

# Lumen Build Script
# Builds the WASM modules, TypeScript packages, and demo app.

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "==================================="
echo "  Lumen Build"
echo "  Trustless Ethereum Light Client"
echo "==================================="
echo ""

# --- Prerequisites Check ---

echo "Checking prerequisites..."

if ! command -v rustc &> /dev/null; then
    echo -e "${RED}Error: Rust is not installed. Install from https://rustup.rs${NC}"
    exit 1
fi

if ! command -v wasm-pack &> /dev/null; then
    echo -e "${YELLOW}wasm-pack not found. Installing...${NC}"
    cargo install wasm-pack
fi

if ! command -v pnpm &> /dev/null; then
    echo -e "${RED}Error: pnpm is not installed. Install with: npm install -g pnpm${NC}"
    exit 1
fi

# Check for LLVM (needed for blst WASM compilation)
LLVM_CLANG=""
if [ -f "/opt/homebrew/opt/llvm/bin/clang" ]; then
    LLVM_CLANG="/opt/homebrew/opt/llvm/bin/clang"
    LLVM_AR="/opt/homebrew/opt/llvm/bin/llvm-ar"
elif [ -f "/usr/local/opt/llvm/bin/clang" ]; then
    LLVM_CLANG="/usr/local/opt/llvm/bin/clang"
    LLVM_AR="/usr/local/opt/llvm/bin/llvm-ar"
elif command -v clang &> /dev/null && clang --target=wasm32-unknown-unknown -x c /dev/null -c -o /dev/null 2>/dev/null; then
    LLVM_CLANG="clang"
    LLVM_AR="llvm-ar"
fi

if [ -z "$LLVM_CLANG" ]; then
    echo -e "${RED}Error: LLVM with wasm32 target support is required for WASM builds.${NC}"
    echo "Install with: brew install llvm"
    exit 1
fi

echo -e "${GREEN}Prerequisites OK${NC}"
echo "  LLVM clang: $LLVM_CLANG"
echo ""

# --- Step 1: Run Rust tests ---

echo "Step 1/5: Running Rust tests..."
cargo test --workspace
echo -e "${GREEN}Rust tests passed${NC}"
echo ""

# --- Step 2: Build WASM modules ---

echo "Step 2/5: Building WASM modules..."

cd crates/lumen-wasm
CC_wasm32_unknown_unknown="$LLVM_CLANG" \
AR_wasm32_unknown_unknown="$LLVM_AR" \
    wasm-pack build --target web --release --out-dir ../../packages/lumen-js/wasm
cd ../..

# Check WASM size
WASM_FILE="packages/lumen-js/wasm/lumen_wasm_bg.wasm"
if [ -f "$WASM_FILE" ]; then
    WASM_SIZE=$(wc -c < "$WASM_FILE" | tr -d ' ')
    WASM_SIZE_KB=$((WASM_SIZE / 1024))
    echo "WASM binary size: ${WASM_SIZE_KB}KB"

    # Check gzipped size
    GZIP_SIZE=$(gzip -c "$WASM_FILE" | wc -c | tr -d ' ')
    GZIP_SIZE_KB=$((GZIP_SIZE / 1024))
    echo "WASM gzipped size: ${GZIP_SIZE_KB}KB"

    # 2MB limit check
    MAX_GZIP_SIZE=2097152  # 2MB in bytes
    if [ "$GZIP_SIZE" -gt "$MAX_GZIP_SIZE" ]; then
        echo -e "${RED}ERROR: Gzipped WASM exceeds 2MB limit (${GZIP_SIZE_KB}KB)${NC}"
        exit 1
    fi
    echo -e "${GREEN}WASM size OK (under 2MB gzipped)${NC}"
else
    echo -e "${YELLOW}Warning: WASM file not found at $WASM_FILE${NC}"
fi

echo ""

# --- Step 3: Install npm dependencies ---

echo "Step 3/5: Installing npm dependencies..."
pnpm install 2>&1 | tail -5
echo -e "${GREEN}Dependencies installed${NC}"
echo ""

# --- Step 4: Build TypeScript packages ---

echo "Step 4/5: Building TypeScript packages..."
cd packages/lumen-js && pnpm run build:ts 2>&1 && cd ../..
cd packages/lumen-react && pnpm run build 2>&1 && cd ../..
echo -e "${GREEN}TypeScript packages built${NC}"
echo ""

# --- Step 5: Build demo ---

echo "Step 5/5: Building demo..."
cd demo && pnpm run build 2>&1 && cd ..
echo -e "${GREEN}Demo built${NC}"
echo ""

# --- Done ---

echo "==================================="
echo -e "${GREEN}Build complete!${NC}"
echo ""
echo "WASM binary: ${WASM_SIZE_KB:-?}KB (${GZIP_SIZE_KB:-?}KB gzipped)"
echo ""
echo "To run the demo:"
echo "  cd demo && pnpm dev"
echo ""
echo "To run tests:"
echo "  cargo test --workspace"
echo ""
echo "Toolchain:"
echo "  Rust: $(rustc --version)"
echo "  wasm-pack: $(wasm-pack --version 2>/dev/null || echo 'not installed')"
echo "  Node: $(node --version)"
echo "  pnpm: $(pnpm --version)"
echo "==================================="
