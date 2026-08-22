#!/bin/zsh
# Regenerate ~/civvis-supervisor-rotating.sh from the TRACKED supervisor.
#
# The tracked supervisor reads its victory lane ONCE, at script start
# (`VICTORY=${CIVVIS_VICTORY:-}`, line ~63) and uses it inside the cycle loop.
# The operator asked for a different lane PER GAME, so the lane has to be
# chosen inside the loop instead. That is the only behavioural change.
#
# ⚠ This is a generated copy, never a hand-maintained one. The play tree is
# `git checkout --detach origin/main`ed every cycle, so a patch applied in
# place would be wiped; and a hand-edited copy in $HOME is exactly the drift
# that broke the ladder on 2026-08-17. Regenerate (and diff) whenever the
# tracked file moves.
#
# ⚠⚠ A copy outside tools/ops/ derives HEAD_REPO=${0:A:h:h:h} as $HOME, not the
# repo. The caller MUST set CIVVIS_HEAD_REPO=/Users/martin/CIVVIS — the
# supervisor's own guard message names this override.
set -u
SRC=${1:-/Users/martin/CIVVIS/tools/ops/civvis-game-supervisor.sh}
DST=${2:-$HOME/civvis-supervisor-rotating.sh}
ANCHOR='  DIFFICULTY=$EXPLICIT_DIFFICULTY'

if ! grep -qF -- "$ANCHOR" "$SRC"; then
  print -r -- "REFUSING: anchor not found in $SRC; the tracked supervisor moved" >&2
  exit 1
fi

python3 - "$SRC" "$DST" <<'PY'
import sys, pathlib
src, dst = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
text = src.read_text()
anchor = "  DIFFICULTY=$EXPLICIT_DIFFICULTY"
block = '''  # --- generated: rotate the victory lane once per cycle -------------------
  # Operator request 2026-08-18: play a different victory condition each game.
  # Targeting is a NEGATIVE gate in advanced.rs -- an agent aimed at one lane
  # prices the others' machinery at -10_000 and abstains from non-emergency
  # World Congress ballots -- so a lane must be chosen per game, not per batch.
  # The index lives on disk so a supervisor restart resumes the rotation
  # instead of replaying lane 0 forever.
  LANE_FILE=${CIVVIS_LANE_FILE:-$HOME/.civvis-victory-rotation}
  # Lanes come from a FILE first so the operator can change the target without
  # restarting the tree -- env is read once at process start, and the lane was
  # already changed twice in one session. Space-separated, one line.
  LANE_LIST_FILE=${CIVVIS_LANE_LIST_FILE:-$HOME/.civvis-victory-lanes}
  if [[ -r "$LANE_LIST_FILE" ]]; then
    LANES=(${=$(<"$LANE_LIST_FILE")})
  else
    LANES=(${=CIVVIS_VICTORY_LANES:-domination science religious diplomatic})
  fi
  (( ${#LANES[@]} )) || LANES=(diplomatic)
  LANE_I=$(cat "$LANE_FILE" 2>/dev/null || print -r -- 0)
  [[ "$LANE_I" =~ '^[0-9]+$' ]] || LANE_I=0
  VICTORY=${LANES[$(( LANE_I % ${#LANES[@]} + 1 ))]}
  print -r -- $(( (LANE_I + 1) % ${#LANES[@]} )) > "$LANE_FILE"
  # ⚠⚠ ZSH DOES NOT WORD-SPLIT AN UNQUOTED EXPANSION. The tracked line
  # `${VICTORY:+--victory "$VICTORY"}` expands to ONE argv token containing a
  # space ("--victory domination"), and argparse answers `unrecognized
  # arguments`. It never showed because VICTORY was always empty upstream, so
  # the expansion vanished -- CIVVIS_VICTORY has in fact never worked. Measured
  # 2026-08-18: four cycles died in 22s, each burning a rotation slot without
  # playing. Build the flag as an ARRAY instead.
  VICTORY_ARGS=()
  [[ -n "$VICTORY" ]] && VICTORY_ARGS=(--victory "$VICTORY")
  say "victory lane for this cycle: $VICTORY (rotation slot $(( LANE_I % ${#LANES[@]} )) of ${#LANES[@]})"
  # --- end generated -------------------------------------------------------

'''
if anchor not in text:
    sys.exit("anchor vanished")
text = text.replace(anchor, block + anchor, 1)

callsite = '      ${VICTORY:+--victory "$VICTORY"} \\\n'
if callsite not in text:
    sys.exit("REFUSING: the --victory call site moved; regenerate by hand")
text = text.replace(callsite, '      "${VICTORY_ARGS[@]}" \\\n', 1)

# Extra climb flags, file-driven so they can change without restarting the tree.
extra_anchor = '      --logs "$LOGS" >'
if extra_anchor not in text:
    sys.exit("REFUSING: the --logs call site moved; regenerate by hand")
text = text.replace(extra_anchor, '      "${EXTRA_CLIMB_ARGS[@]}" \\\n      --logs "$LOGS" >', 1)
lane_marker = '  VICTORY_ARGS=()'
extra_block = """  # Extra climb flags from a file (space-separated, one line), re-read every
  # cycle so the operator can tune without restarting the tree. Empty/missing =
  # nothing added, which is the stock invocation.
  EXTRA_ARGS_FILE=${CIVVIS_CLIMB_EXTRA_ARGS_FILE:-$HOME/.civvis-climb-extra-args}
  EXTRA_CLIMB_ARGS=()
  if [[ -r "$EXTRA_ARGS_FILE" ]]; then
    EXTRA_CLIMB_ARGS=(${=$(<"$EXTRA_ARGS_FILE")})
  fi
  (( ${#EXTRA_CLIMB_ARGS[@]} )) && say "extra climb args: ${EXTRA_CLIMB_ARGS[*]}"
"""
text = text.replace(lane_marker, extra_block + lane_marker, 1)
dst.write_text(text)
PY

chmod +x "$DST"
print -r -- "generated $DST from $SRC"
print -r -- "tracked sha: $(cd ${SRC:h:h:h} && git rev-parse --short HEAD)"
diff <(print -r -- "") /dev/null >/dev/null 2>&1
print -r -- "--- inserted block ---"
grep -n "generated: rotate" -A 14 "$DST"
