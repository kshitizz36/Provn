#!/usr/bin/env bash
set -euo pipefail

CHANGED=$(git diff HEAD~1 HEAD --name-only 2>/dev/null || git diff --name-only HEAD)

if [ -z "$CHANGED" ]; then
  echo "No changed files to scan."
  exit 0
fi

FOUND_BLOCKING=0

while IFS= read -r file; do
  [ -n "$file" ] || continue

  if [ ! -f "$file" ]; then
    continue
  fi

  result=$(./provn-cli/target/release/provn check --json "$file" 2>&1 || true)
  echo "$result" | python3 -m json.tool || echo "$result"

  RESULT_JSON="$result" python3 - "$file" <<'PY'
import json
import os
import sys

path = sys.argv[1]

try:
    payload = json.loads(os.environ["RESULT_JSON"])
except Exception:
    print(f"::warning file={path},line=1,title=Provn output::Could not parse provn output for {path}")
    raise SystemExit(0)

for finding in payload.get("findings", []):
    tier = str(finding.get("tier", "unknown"))
    level = "error" if tier in {"T0", "T1"} else "warning"
    line = finding.get("line") or 1
    description = str(finding.get("description", "Provn finding")).replace("\n", " ")
    print(f"::{level} file={path},line={line},title=Provn {tier}::{path}:{line} {description}")
PY

  has_blocking=$(RESULT_JSON="$result" python3 -c 'import json, os; findings = json.loads(os.environ["RESULT_JSON"]).get("findings", []); print("true" if any(str(f.get("tier", "")) in {"T0", "T1"} for f in findings) else "false")' 2>/dev/null || echo false)

  if [ "$has_blocking" = "true" ]; then
    FOUND_BLOCKING=1
  fi
done <<< "$CHANGED"

if [ "$FOUND_BLOCKING" -ne 0 ]; then
  echo "::error::Provn detected a blocking secret or IP leak. See scan output above."
  exit 1
fi
