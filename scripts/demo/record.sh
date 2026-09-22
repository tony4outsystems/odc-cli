#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CMDS_FILE="$SCRIPT_DIR/cmds.txt"
CAST_FILE="$SCRIPT_DIR/demo.cast"

# Source environment
if [ -f "$PROJECT_ROOT/.env" ]; then
  set -a
  source "$PROJECT_ROOT/.env"
  set +a
fi

if ! command -v odc &> /dev/null; then
  echo "Error: odc binary not found in PATH"
  exit 1
fi

echo "Recording demo commands..."

# Create temp script with pv typing effect
TEMP_SCRIPT=$(mktemp)
trap "rm -f $TEMP_SCRIPT" EXIT

{
  echo "set +e"
  while IFS= read -r cmd || [ -n "$cmd" ]; do
    [ -z "$cmd" ] || [ "${cmd:0:1}" = "#" ] && continue
    echo "pv -qL 50 <<<'$cmd' && sleep 0.5"
  done < "$CMDS_FILE"
} > "$TEMP_SCRIPT"

asciinema rec --overwrite --title "ODC CLI Demo" "$CAST_FILE" -c "bash $TEMP_SCRIPT"

echo "Recording saved to $CAST_FILE"
