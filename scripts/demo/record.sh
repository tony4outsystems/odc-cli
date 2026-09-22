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

# Ensure odc is in PATH
if ! command -v odc &> /dev/null; then
  echo "Error: odc binary not found in PATH"
  exit 1
fi

# Create a temporary script with all commands
TEMP_SCRIPT=$(mktemp)
trap "rm -f $TEMP_SCRIPT" EXIT

cat > "$TEMP_SCRIPT" << 'EOF'
set +e
EOF

while IFS= read -r cmd || [ -n "$cmd" ]; do
  # Skip empty lines and comments
  [ -z "$cmd" ] || [ "${cmd:0:1}" = "#" ] && continue
  echo "$cmd" >> "$TEMP_SCRIPT"
done < "$CMDS_FILE"

# Record commands with asciinema
echo "Recording demo commands..."
asciinema rec --overwrite --title "ODC CLI Demo" "$CAST_FILE" -c "bash $TEMP_SCRIPT"

echo "Recording saved to $CAST_FILE"
