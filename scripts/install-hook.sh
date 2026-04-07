#!/bin/bash
# Install the Provn pre-commit hook into the current git repo
# Usage: ./scripts/install-hook.sh [provn_binary_path]

set -e

PROVN_BIN="${1:-$(which provn 2>/dev/null || which aegis 2>/dev/null || echo "$(pwd)/provn-cli/target/release/provn")}"
HOOK_PATH=".git/hooks/pre-commit"

if [ ! -d ".git" ]; then
    echo "Error: not in a git repository root"
    exit 1
fi

if [ ! -f "$PROVN_BIN" ]; then
    echo "Provn binary not found at: $PROVN_BIN"
    echo "Build with: cd provn-cli && cargo build --release"
    exit 1
fi

mkdir -p .git/hooks

cat > "$HOOK_PATH" <<EOF
#!/bin/sh
"$PROVN_BIN" scan
EOF

chmod +x "$HOOK_PATH"
echo "Provn pre-commit hook installed at $HOOK_PATH"
echo "  Binary: $PROVN_BIN"
echo ""
echo "Test it: echo 'AKIAIOSFODNN7EXAMPLE' >> test.py && git add test.py && git commit -m test"
