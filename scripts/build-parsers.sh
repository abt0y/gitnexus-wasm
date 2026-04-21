#!/bin/bash
set -e

# Build tree-sitter parsers to WASM for GitNexus
# Usage: ./scripts/build-parsers.sh [output_dir]

OUTPUT_DIR="${1:-web/public/parsers}"
mkdir -p "$OUTPUT_DIR"

# Ensure tree-sitter CLI is installed
if ! command -v tree-sitter &> /dev/null; then
    echo "Installing tree-sitter CLI..."
    npm install -g tree-sitter-cli
fi

# Language configurations
declare -A LANGUAGES=(
    ["typescript"]="tree-sitter/tree-sitter-typescript:tsx"
    ["javascript"]="tree-sitter/tree-sitter-javascript:."
    ["python"]="tree-sitter/tree-sitter-python:."
    ["go"]="tree-sitter/tree-sitter-go:."
    ["rust"]="tree-sitter/tree-sitter-rust:."
    ["java"]="tree-sitter/tree-sitter-java:."
    ["c"]="tree-sitter/tree-sitter-c:."
    ["cpp"]="tree-sitter/tree-sitter-cpp:."
    ["csharp"]="tree-sitter/tree-sitter-c-sharp:."
    ["php"]="tree-sitter/tree-sitter-php:php"
    ["swift"]="alex-pinkus/tree-sitter-swift:."
    ["ruby"]="tree-sitter/tree-sitter-ruby:."
)

for lang in "${!LANGUAGES[@]}"; do
    IFS=':' read -r repo dir <<< "${LANGUAGES[$lang]}"

    echo "Building $lang parser..."

    TMP_DIR="/tmp/tree-sitter-$lang"

    if [ ! -d "$TMP_DIR" ]; then
        git clone --depth 1 "https://github.com/$repo.git" "$TMP_DIR"
    fi

    cd "$TMP_DIR/$dir"

    # Build WASM parser
    tree-sitter build --wasm

    # Copy to output
    if [ -f "parser.wasm" ]; then
        cp "parser.wasm" "$OUTPUT_DIR/$lang.wasm"
        echo "  ✓ Built $lang.wasm ($(du -h "$OUTPUT_DIR/$lang.wasm" | cut -f1))"
    else
        echo "  ✗ Failed to build $lang parser"
    fi
done

echo ""
echo "All parsers built to $OUTPUT_DIR:"
ls -lh "$OUTPUT_DIR"
