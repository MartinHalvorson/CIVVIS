#!/bin/zsh
# Keep the GUI-capable Civ VI popup clearer alive from an interactive context.
#
# macOS TCC blocks launchd from controlling Civ VI on this machine.  A LaunchAgent
# can make the process table look healthy while the helper sees no game window, so
# this intentionally runs as a detached child of the interactive runner instead.
set -u
# zsh sets BG_NICE by default: every `&` job starts at nice +5 and the whole
# subtree inherits it. On 2026-08-11 that put Civilization VI -- the one process
# the live ladder depends on -- underneath every nice-0 cargo build on the box
# (9-11 s/turn quiet, ~18 s/turn under fleet load), and macOS refuses to lower a
# nice once set, so a demoted game stays demoted for its whole run.
# civvis-keeper.sh had already found and fixed this for the exhibition lane; the
# live lane kept paying it. tools/test_ops_background_priority.py holds the line.
unsetopt BG_NICE

PYTHON=/opt/homebrew/opt/python@3.14/bin/python3.14
CLEARER=$HOME/CIVVIS/tools/civ6_control/popup_clear.py
RUNS=$HOME/civvis-civ6-runs/control
ACTIVITY_LOG=$HOME/civvis-civ6-runs/popup_clear.log
KEEPER_LOG=$HOME/civvis-civ6-runs/popup_clear.keeper.log
LOCK=${CIVVIS_POPUP_KEEPER_LOCK:-$HOME/.civvis-popup-keeper.lock}
PID_FILE=$LOCK/pid

say() { print -r -- "[popup-keeper] $(date -u +%FT%TZ) $*" >> "$KEEPER_LOG" }

release_lock() {
  local holder=""
  [[ -f "$PID_FILE" ]] && holder=$(<"$PID_FILE")
  [[ "$holder" == "$$" ]] && rm -rf -- "$LOCK"
}

if ! mkdir "$LOCK" 2>/dev/null; then
  holder=""
  [[ -f "$PID_FILE" ]] && holder=$(<"$PID_FILE")
  if [[ -n "$holder" ]] && kill -0 "$holder" 2>/dev/null; then
    say "another popup keeper is already alive (pid $holder); exiting"
    exit 0
  fi
  rm -rf -- "$LOCK"
  if ! mkdir "$LOCK" 2>/dev/null; then
    say "could not acquire popup keeper lock"
    exit 70
  fi
fi
print -r -- "$$" > "$PID_FILE"
trap release_lock EXIT
trap 'exit 0' HUP INT TERM

say "keeper up (pid $$)"
while true; do
  # Keep diagnostic stdout separate from the activity ledger. popup_clear.py writes
  # each activity line to --log itself; combining both streams doubled every line.
  PYTHONWARNINGS='ignore::DeprecationWarning' "$PYTHON" -u "$CLEARER" \
      --interval 2.5 --runs "$RUNS" --log "$ACTIVITY_LOG" >> "$KEEPER_LOG" 2>&1 &
  clearer_pid=$!
  say "started popup clearer pid $clearer_pid"
  wait "$clearer_pid"
  exit_status=$?
  say "popup clearer pid $clearer_pid exited status $exit_status; restarting in 3s"
  sleep 3
done
