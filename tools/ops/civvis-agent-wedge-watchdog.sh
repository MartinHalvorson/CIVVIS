#!/bin/zsh
# Restart a game whose agent has died, or whose current game makes no progress.
#
# ⚠⚠⚠ THE FAILURE THIS CATCHES IS SILENT. On 2026-08-19 a DiplomacyActionView
# dialogue defeated both the mod's autoclose and the harness's rescue; the agent
# stopped issuing orders at turn 49 and Civ 6 ran on to turn 93 with a dead seat
# — 44 turns played by nobody. The play log looks QUIET, not broken, and
# `popup_clear` disqualifies itself in this exact state ("no turn recorded yet;
# this is setup"), so nothing recovers it.
#
# One reliable signal is the AGENT'S OWN turn falling behind the GAME'S turn:
#   max(turn) in orders.sqlite   vs   /status turn from the mirror.
#
# A second, intentionally conservative signal catches the opposite failure:
# agent and game agree on a turn forever. It is armed only after the mirror
# agrees with this run's JSONL turn, then requires several unchanged mirror
# process/turn/frame and local-turn samples. That excludes a mirror left over
# from a previous game and gives a genuinely slow late-game turn ample time.
#
# Remedy is the tested restart order: TERM the climb (owns the gamelock and
# releases it), then SIGINT civ6_play (SIGTERM skips its atexit teardown and
# leaves a stale RunTag). The supervisor then starts the next game itself.
set -u

RUNS=${CIVVIS_RUNS:-$HOME/civvis-civ6-runs/control}
PORT=${CIVVIS_MIRROR_PORT:-8610}
LOG=${CIVVIS_WEDGE_LOG:-$HOME/civvis-civ6-runs/agent_wedge_watchdog.log}
POLL_S=${CIVVIS_WEDGE_POLL_S:-60}
# A gap this large, seen on consecutive polls, is a dead agent rather than a
# slow turn. Two confirmations so a mid-turn sample never triggers a restart.
GAP=${CIVVIS_WEDGE_GAP:-12}
CONFIRM=${CIVVIS_WEDGE_CONFIRM:-2}
# Unlike a slow late-game turn, a unit blocker repeated on the *same* turn is
# positive evidence that Civ VI cannot advance. The controller has already
# tried its bounded forfeit ladder; this separate guard keeps the unattended
# lane from spending the rest of the timeout on that one board.
BLOCKER_STREAK=${CIVVIS_WEDGE_BLOCKER_STREAK:-6}
# Consecutive one-minute samples with no synchronized game progress before a
# clean recovery. Five means roughly five minutes after the first trustworthy
# sample — shorter than the harness's eight-minute frozen-turn backstop, but
# still deliberately patient about legitimate late-game animations.
PROGRESS_CONFIRM=${CIVVIS_WEDGE_PROGRESS_CONFIRM:-5}
PROGRESS_TURN_SKEW=${CIVVIS_WEDGE_PROGRESS_TURN_SKEW:-1}
SELF_DIR=${0:A:h}
STATE_READER=${CIVVIS_WEDGE_STATE_READER:-${SELF_DIR}/civvis_watchdog_state.py}
LOCK=${CIVVIS_WEDGE_LOCK:-$HOME/.civvis-agent-wedge-watchdog.lock}

say() { print -r -- "[wedge] $(date -u +%FT%TZ) $*" >> "$LOG" }

# The climb owns the game lock. Preserve the documented recovery order so its
# cleanup runs before the player sees SIGINT; SIGTERM on civ6_play skips atexit
# and can strand the run tag for the successor game.
restart_attempt() {
  local reason="$1"
  say "$reason; restarting (TERM climb, then INT civ6_play)"
  local climb=$(pgrep -f "[c]iv6_civvis_climb\.py" 2>/dev/null | head -1)
  [[ -n "$climb" ]] && kill -TERM "$climb" 2>/dev/null && say "  TERM climb $climb"
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    [[ -n "$climb" ]] && kill -0 "$climb" 2>/dev/null || break
    sleep 3
  done
  local play=$(pgrep -f "[c]iv6_play\.py" 2>/dev/null | head -1)
  [[ -n "$play" ]] && kill -INT "$play" 2>/dev/null && say "  INT civ6_play $play"
}

reset_progress() {
  progress_strikes=0
  last_progress=""
}

if ! mkdir "$LOCK" 2>/dev/null; then
  holder=$(cat "$LOCK/pid" 2>/dev/null || print -r -- "")
  if [[ -n "$holder" ]] && kill -0 "$holder" 2>/dev/null; then
    say "another watchdog is alive (pid $holder); exiting"; exit 0
  fi
  rm -rf -- "$LOCK"; mkdir "$LOCK" 2>/dev/null || exit 70
fi
print -r -- "$$" > "$LOCK/pid"
trap 'rm -rf -- "$LOCK"' EXIT
trap 'exit 0' HUP INT TERM

say "watchdog up (pid $$, gap>${GAP} confirmed ${CONFIRM}x, no progress ${PROGRESS_CONFIRM}x, poll ${POLL_S}s)"
strikes=0
progress_strikes=0
last_progress=""
last_tag=""
while true; do
  sleep "$POLL_S"
  play_pid=$(pgrep -f "[c]iv6_play\.py" 2>/dev/null | head -1)
  [[ -z "$play_pid" ]] && { strikes=0; reset_progress; continue }

  tag=$(ls -t "$RUNS" 2>/dev/null | grep '^civvis-' | head -1)
  [[ -z "$tag" ]] && { strikes=0; reset_progress; continue }
  # A new game resets the count; never carry strikes across runs.
  [[ "$tag" != "$last_tag" ]] && { strikes=0; reset_progress; last_tag=$tag }

  # The agent and the game can agree on the same turn forever: a selected unit
  # remains ready, the mod repeatedly emits ENDTURN_BLOCKING_UNITS, and neither
  # side has a turn gap for the old watchdog to see. Count only the explicit,
  # latest same-turn unit blocker — the helper ignores normal production and
  # policy notifications and clears the signal as soon as a later turn lands.
  blocker_signal=""
  if [[ -f "$STATE_READER" ]]; then
    blocker_signal=$(python3 "$STATE_READER" "$RUNS/$tag/events.jsonl" 2>/dev/null || true)
  fi
  blocker_turn=""; blocker_name=""; blocker_count=""
  [[ -n "$blocker_signal" ]] && read -r blocker_turn blocker_name blocker_count <<< "$blocker_signal"
  if [[ "$blocker_turn" =~ '^[0-9]+$' ]] \
      && [[ "$blocker_count" =~ '^[0-9]+$' ]] \
      && (( blocker_count >= BLOCKER_STREAK )); then
    restart_attempt "$tag repeating unit blocker ${blocker_name} at t${blocker_turn} (${blocker_count} sightings)"
    strikes=0
    reset_progress
    continue
  fi

  mirror_status=$(curl -s --max-time 5 "http://127.0.0.1:${PORT}/status" 2>/dev/null)
  mirror_turn=$(print -r -- "$mirror_status" | python3 -c 'import sys,json;print(json.load(sys.stdin).get("turn") or 0)' 2>/dev/null)
  [[ "$mirror_turn" =~ '^[0-9]+$' ]] || { strikes=0; reset_progress; continue }
  db="$RUNS/$tag/orders.sqlite"
  [[ -r "$db" ]] || { strikes=0; reset_progress; continue }
  agent_turn=$(python3 - "$db" <<'PY' 2>/dev/null
import sqlite3, sys
try:
    c = sqlite3.connect(f"file:{sys.argv[1]}?mode=ro", uri=True)
    print(c.execute("select coalesce(max(turn),0) from orders").fetchone()[0])
except Exception:
    print(-1)
PY
)
  [[ "$agent_turn" =~ '^[0-9]+$' ]] || { strikes=0; reset_progress; continue }

  # Do not infer a stuck game from an old mirror. The helper emits nothing
  # until this run has a turn event that matches the current /status document.
  progress_signal=""
  if [[ -f "$STATE_READER" ]]; then
    progress_signal=$(print -r -- "$mirror_status" \
      | python3 "$STATE_READER" --progress "$RUNS/$tag/events.jsonl" \
          --max-turn-skew "$PROGRESS_TURN_SKEW" 2>/dev/null || true)
  fi
  if [[ "$progress_signal" =~ '^[0-9]+ [0-9]+ [0-9]+ [0-9]+$' ]]; then
    if [[ "$progress_signal" == "$last_progress" ]]; then
      progress_strikes=$(( progress_strikes + 1 ))
      say "$tag no synchronized progress (${progress_signal}) strike ${progress_strikes}/${PROGRESS_CONFIRM}"
      if (( progress_strikes >= PROGRESS_CONFIRM )); then
        restart_attempt "$tag NO GAME PROGRESS confirmed at t${mirror_turn}"
        strikes=0
        reset_progress
        continue
      fi
    else
      (( progress_strikes )) && say "$tag synchronized progress recovered; clearing ${progress_strikes} strike(s)"
      last_progress="$progress_signal"
      progress_strikes=0
    fi
  else
    reset_progress
  fi

  gap=$(( mirror_turn - agent_turn ))
  if (( gap >= GAP )); then
    strikes=$(( strikes + 1 ))
    say "$tag agent t${agent_turn} vs game t${mirror_turn} (gap ${gap}) strike ${strikes}/${CONFIRM}"
    if (( strikes >= CONFIRM )); then
      restart_attempt "$tag DEAD AGENT confirmed"
      strikes=0
      reset_progress
    fi
  else
    (( strikes )) && say "$tag recovered (gap ${gap}); clearing strikes"
    strikes=0
  fi
done
