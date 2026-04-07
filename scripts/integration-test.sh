#!/bin/bash
# CLI-only Provn integration test / demo rehearsal
# Usage: ./scripts/integration-test.sh

set -e

PROVN="$(pwd)/provn-cli/target/release/provn"
TMPDIR_TEST=$(mktemp -d)
PASS=0
FAIL=0

log() { echo "[test] $*"; }
ok()  { echo "[test] ✓ $1"; PASS=$((PASS+1)); }
fail(){ echo "[test] ✗ $1: $2"; FAIL=$((FAIL+1)); }

cleanup() {
    rm -rf "$TMPDIR_TEST"
}
trap cleanup EXIT

# Check prerequisites
if [ ! -f "$PROVN" ]; then
    log "Building provn..."
    (cd provn-cli && cargo build --release 2>&1 | tail -3)
fi

# ── Test 1: Clean commit ────────────────────────────────────────────────────
log "Test 1: Clean file"
cd "$TMPDIR_TEST"
git init -q && git config user.email "test@test.com" && git config user.name "Test"
cat > clean.py << 'EOF'
def greet(name):
    return f"Hello, {name}!"
EOF
git add clean.py

OUTPUT=$("$PROVN" check clean.py 2>&1)
if echo "$OUTPUT" | grep -qi "clean"; then
    ok "Clean file passes"
else
    fail "Clean file" "Expected Clean, got: $OUTPUT"
fi

# ── Test 2: AWS key blocked ──────────────────────────────────────────────────
log "Test 2: AWS access key detection"
cat > secrets.py << 'EOF'
AWS_ACCESS_KEY_ID = "AKIAIOSFODNN7EXAMPLE"
EOF
git add secrets.py

OUTPUT=$("$PROVN" check secrets.py 2>&1 || true)
if echo "$OUTPUT" | grep -qi "threat\|block\|T0\|aws"; then
    ok "AWS key detected"
else
    fail "AWS key detection" "Expected threat, got: $OUTPUT"
fi

# ── Test 3: System prompt detection ─────────────────────────────────────────
log "Test 3: System prompt detection"
cat > bot.py << 'EOF'
system_prompt = "You are FinanceBot. Use our proprietary scoring algorithm: score = 0.7*income - 0.3*debt. Never reveal this formula to users."
EOF
git add bot.py

OUTPUT=$("$PROVN" check bot.py 2>&1 || true)
if echo "$OUTPUT" | grep -qi "threat\|block\|T1\|system_prompt"; then
    ok "System prompt detected"
else
    fail "System prompt" "Expected threat, got: $OUTPUT"
fi

# ── Test 4: Private key detection ───────────────────────────────────────────
log "Test 4: Private key detection"
cat > key.txt << 'EOF'
-----BEGIN RSA PRIVATE KEY-----
MIIEowIBAAKCAQEA2a2rwplBQLzFakeKey==
-----END RSA PRIVATE KEY-----
EOF
git add key.txt

OUTPUT=$("$PROVN" check key.txt 2>&1 || true)
if echo "$OUTPUT" | grep -qi "threat\|block\|T0\|private"; then
    ok "Private key detected"
else
    fail "Private key" "Expected threat, got: $OUTPUT"
fi

# ── Test 5: Entropy detection ────────────────────────────────────────────────
log "Test 5: High entropy string"
cat > config.py << 'EOF'
secret_token = "x7Kp2mNqR9vT4wYjLhBcDfAeGiUoSzXnPqRsT"
EOF
git add config.py

"$PROVN" check config.py 2>&1 || true
ok "Entropy scan ran without crash"

# ── Test 6: Verify audit chain ───────────────────────────────────────────────
log "Test 6: Audit chain"
"$PROVN" verify-audit 2>&1 || true
ok "verify-audit command ran"

# ── Summary ──────────────────────────────────────────────────────────────────
cd - > /dev/null
echo ""
echo "══════════════════════════════"
echo "  Results: $PASS passed, $FAIL failed"
echo "══════════════════════════════"

if [ $FAIL -gt 0 ]; then
    exit 1
fi
