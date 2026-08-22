#!/bin/zsh
# Wait for the holy-lane-parity direct screen (by PID, never pgrep) and analyse it.
cd /Users/martin || exit 1
PID=34053
while kill -0 "$PID" 2>/dev/null; do sleep 60; done
W=/Users/martin/civvis-holy-lane-parity-direct-screen-bb8d
"$W/target/ci/gene_screen" --analyze /Users/martin/civvis-gene-screens/holy-lane-parity-direct.jsonl \
  --json /Users/martin/civvis-gene-screens/holy-lane-parity-direct-analysis.json
echo "SCREEN COMPLETE"
