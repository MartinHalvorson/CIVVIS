#!/bin/zsh
# Keep the CIVVIS strategy league running on every machine that is up.
#
# Prefers the canonical tool from the fleet's own origin/main worktree, which
# `civvis_fleet.py deploy` keeps current, and falls back to the staged copy
# until that lands on main. So this script self-updates without being edited.
export PATH="$HOME/.cargo/bin:$PATH"
ROOT=/Users/martin/civvis-fleet
CANON="$ROOT/src/tools/civvis_fleet.py"
STAGED="$ROOT/civvis_fleet.staged.py"
TOOL="$STAGED"
[[ -f "$CANON" ]] && TOOL="$CANON"
echo "$(date -u +%FT%TZ) fleet keeper starting with $TOOL"
exec /usr/bin/python3 "$TOOL" run --interval 300
