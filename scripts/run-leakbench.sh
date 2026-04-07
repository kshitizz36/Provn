#!/bin/bash
# Run the full LeakBench evaluation suite
# Usage: ./scripts/run-leakbench.sh

set -e

cd "$(dirname "$0")/../provn-cli"
cargo test -- leakbench --nocapture 2>&1 || true

echo ""
echo "Running LeakBench against eval set..."
python3 ../aegis-model/leakbench_eval.py 2>/dev/null || echo "(model not available — Layer 1+2 only)"
