#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONFIG_FILE="${PROVN_CONFIG:-${AEGIS_CONFIG:-$ROOT_DIR/provn.yml}}"
HOST="${PROVN_HOST:-${AEGIS_HOST:-127.0.0.1}}"
PORT="${PROVN_PORT:-${AEGIS_PORT:-8080}}"

read_config_model() {
  if [[ -f "$CONFIG_FILE" ]]; then
    awk '
      /^[[:space:]]*model:[[:space:]]*/ {
        value = $2
        gsub(/"/, "", value)
        print value
        exit
      }
    ' "$CONFIG_FILE"
  fi
}

find_model_path() {
  local configured="${PROVN_MODEL_PATH:-${AEGIS_MODEL_PATH:-$(read_config_model)}}"
  local candidates=()

  if [[ -n "$configured" ]]; then
    candidates+=("$configured")
    candidates+=("$HOME/.provn/models/$configured")
    candidates+=("$HOME/.aegis/models/$configured")
    candidates+=("$ROOT_DIR/$configured")
  fi

  candidates+=(
    "$HOME/.provn/models/provn-gemma4-e2b-q4km.gguf"
    "$HOME/.provn/models/Gemma-4-E2B-it.Q4_K_M.gguf"
    "$HOME/.provn/models/provn-gemma4-e2b.gguf"
    "$HOME/.provn/models/provn-gemma4-e2b-gguf"
    "$HOME/.aegis/models/aegis-gemma4-e2b-q4km.gguf"
    "$HOME/.aegis/models/aegis-gemma4-e2b.gguf"
    "$HOME/.aegis/models/aegis-gemma4-e2b-gguf"
  )

  local candidate
  for candidate in "${candidates[@]}"; do
    if [[ -f "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  candidate="$(
    find "$HOME/.provn/models" "$HOME/.aegis/models" -maxdepth 1 -type f \
      \( -name 'provn-gemma4-e2b*' -o -name 'aegis-gemma4-e2b*' -o -name 'Gemma-4-E2B-it.Q4_K_M.gguf' -o -name '*.gguf' \) \
      2>/dev/null | head -n 1 || true
  )"
  if [[ -n "$candidate" ]]; then
    printf '%s\n' "$candidate"
    return 0
  fi

  return 1
}

find_llama_server() {
  if [[ -n "${PROVN_LLAMA_SERVER:-${AEGIS_LLAMA_SERVER:-}}" && -x "${PROVN_LLAMA_SERVER:-${AEGIS_LLAMA_SERVER:-}}" ]]; then
    printf '%s\n' "${PROVN_LLAMA_SERVER:-${AEGIS_LLAMA_SERVER:-}}"
    return 0
  fi

  if command -v llama-server >/dev/null 2>&1; then
    command -v llama-server
    return 0
  fi

  local candidates=(
    "$HOME/llama.cpp/build/bin/llama-server"
    "$HOME/Programming/llama.cpp/build/bin/llama-server"
    "$HOME/Desktop/llama.cpp/build/bin/llama-server"
    "/opt/homebrew/bin/llama-server"
    "/usr/local/bin/llama-server"
  )

  local candidate
  for candidate in "${candidates[@]}"; do
    if [[ -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  return 1
}

MODEL_PATH="$(find_model_path || true)"
if [[ -z "$MODEL_PATH" ]]; then
  echo "Provn could not find a GGUF model."
  echo "Expected one of:"
  echo "  - \$PROVN_MODEL_PATH"
  echo "  - ~/.provn/models/provn-gemma4-e2b-q4km.gguf"
  echo "  - ~/.provn/models/Gemma-4-E2B-it.Q4_K_M.gguf"
  echo "  - ~/.provn/models/provn-gemma4-e2b.gguf"
  echo "  - ~/.provn/models/provn-gemma4-e2b-gguf"
  echo "  - ~/.aegis/models/aegis-gemma4-e2b.gguf"
  echo "  - ~/.aegis/models/aegis-gemma4-e2b-gguf"
  exit 1
fi

LLAMA_SERVER="$(find_llama_server || true)"
if [[ -z "$LLAMA_SERVER" ]]; then
  echo "Provn could not find llama-server."
  echo "Set \$PROVN_LLAMA_SERVER or install/build llama.cpp so llama-server is on PATH."
  exit 1
fi

echo "Starting Provn semantic server"
echo "Model:  $MODEL_PATH"
echo "Server: $LLAMA_SERVER"
echo "URL:    http://$HOST:$PORT/completion"

exec "$LLAMA_SERVER" \
  -m "$MODEL_PATH" \
  --host "$HOST" \
  --port "$PORT" \
  ${PROVN_LLAMA_ARGS:-${AEGIS_LLAMA_ARGS:-}}
