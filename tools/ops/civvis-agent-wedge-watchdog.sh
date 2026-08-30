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
# Seconds of stack sampling kept from a wedged Civilization VI before it is
# restarted. Two is enough to name the stuck thread and cheap next to the five
# minutes of no progress that preceded it; `0` is still a valid sample.
WEDGE_SAMPLE_SECONDS=${CIVVIS_WEDGE_SAMPLE_SECONDS:-2}
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
# ⭐⭐⭐ A DEEP WEDGED GAME IS HANDED TO THE CLIMB, NOT DESTROYED. Civ 6 writes
# an autosave every turn and `civ6_civvis_climb.py` can reload one into a FRESH
# Civ 6 (`resume_from_autosave` -> `civ6_play --load-save`), which is the only
# thing that recovers a parked core: the deadlocked process is replaced and the
# match is kept. Killing the climb here threw that away — 7 `-contN` runs exist,
# all from 08-17..19, none since this watchdog began signalling. So past this
# turn, signal ONLY the player and leave the climb alive to do the reload.
RESUME_FLOOR=${CIVVIS_WEDGE_RESUME_MIN_TURN:-20}
# Polls with no owned player after a handoff before the climb is presumed hung
# and terminated after all. This watchdog exists for a climb that is itself
# blocked, so the handoff must never become a way for one to sit forever.
HANDOFF_GRACE=${CIVVIS_WEDGE_HANDOFF_GRACE:-12}
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

# The watchdog belongs to the unattended climb lane, not to every Python
# process whose argv happens to mention civ6_play.py.  A protected manual
# continuation deliberately has no climb parent: it may be loading a saved,
# viable game while the normal supervisor is held.  The old global pgrep then
# sent SIGINT after five quiet mirror polls, cutting off an actively computing
# King continuation at t172.  Prove both Python harnesses and their ancestry
# before this helper may signal anything.  On uncertainty, leave the game alone.
is_python_harness() {
  local pid="$1" marker="$2" command="" executable=""
  [[ "$pid" == <-> ]] || return 1
  command=$(ps -p "$pid" -o command= 2>/dev/null)
  [[ "$command" == *"$marker"* ]] || return 1
  executable=$(lsof -a -p "$pid" -d txt -Fn 2>/dev/null \
    | sed -n 's/^n//p' | head -n 1)
  case "$executable" in
    */Python|*/python|*/python[0-9]*) return 0 ;;
  esac
  return 1
}

parent_pid() {
  local pid="$1" parent=""
  [[ "$pid" == <-> ]] || return 1
  parent=$(ps -p "$pid" -o ppid= 2>/dev/null | tr -d '[:space:]')
  [[ "$parent" == <-> ]] || return 1
  print -r -- "$parent"
}

descends_from() {
  local child="$1" ancestor="$2" parent="" hops=0
  while (( hops < 64 )); do
    [[ "$child" == "$ancestor" ]] && return 0
    parent=$(parent_pid "$child" || true)
    [[ "$parent" == <-> && "$parent" != "$child" ]] || return 1
    child="$parent"
    (( hops += 1 ))
  done
  return 1
}

# Print "<climb-pid> <play-pid>" only for the one player the live climb owns.
# A successful PID lookup alone is deliberately insufficient: the game install
# is shared, and a manual save continuation must never become this watchdog's
# recovery target.
owned_climb_and_player() {
  local climb="" play=""
  for climb in ${(f)"$(pgrep -f '[c]iv6_civvis_climb\.py' 2>/dev/null)"}; do
    is_python_harness "$climb" "civ6_civvis_climb.py" || continue
    for play in ${(f)"$(pgrep -f '[c]iv6_play\.py' 2>/dev/null)"}; do
      is_python_harness "$play" "civ6_play.py" || continue
      descends_from "$play" "$climb" || continue
      print -r -- "$climb $play"
      return 0
    done
  done
  return 1
}

player_uses_tag() {
  local pid="$1" tag="$2" command=""
  is_python_harness "$pid" "civ6_play.py" || return 1
  command=$(ps -p "$pid" -o command= 2>/dev/null)
  [[ "$command" == *"--tag $tag"* ]]
}

# The climb owns the game lock. Preserve the documented recovery order so its
# cleanup runs before the player sees SIGINT; SIGTERM on civ6_play skips atexit
# and can strand the run tag for the successor game.  Both PIDs were proven as
# one climb-owned pair by owned_climb_and_player before this is called.
restart_attempt() {
  local reason="$1" climb="$2" play="$3" tag="$4" turn="${5:-0}"
  if ! is_python_harness "$climb" "civ6_civvis_climb.py" \
      || ! player_uses_tag "$play" "$tag"; then
    say "$tag recovery target is no longer the proven owned pair; leaving it alone"
    return 0
  fi
  # ⭐⭐ LEAVE EVIDENCE. A wedge that is only restarted teaches nothing, and the
  # restart destroys the one state that could explain it.
  #
  # Two games wedged on 2026-08-28 — a Prince run at t34 and a King run at t44 —
  # and after both, all that survived was "no synchronized progress" and a dead
  # process. The events stop mid-turn, `stdout.log` ends on an unremarkable line
  # (its no-path sentinels are ordinary: a run that never wedged carried 31 of
  # them), and neither the last events nor the last stdout line were the same
  # between the two. So there was nothing left to compare, and every theory
  # about the cause was unfalsifiable.
  #
  # `sample` answers the only question that matters — WHERE is the game stuck —
  # and needs no privileges for a process this user owns. Measured on this host:
  # a 1-second sample of Civ 6 returns a 1789-line call graph naming the main
  # thread and its dispatch queue. It runs BEFORE the kill, because afterwards
  # there is nothing to sample, and its failure never blocks the restart.
  local game_pid sample_file
  game_pid=$(pgrep -x Civ6_Exe_Child 2>/dev/null | head -1)
  sample_file="$RUNS/$tag/wedge-sample.txt"
  if [[ -n "$game_pid" && -x /usr/bin/sample && -d "$RUNS/$tag" ]]; then
    if /usr/bin/sample "$game_pid" "${WEDGE_SAMPLE_SECONDS}" \
        -file "$sample_file" >/dev/null 2>&1; then
      say "  sampled wedged Civ 6 pid $game_pid to $sample_file"
    else
      say "  could not sample Civ 6 pid $game_pid; restarting without it"
    fi
  else
    say "  no Civ 6 process to sample; restarting without it"
  fi
  if [[ "$turn" =~ '^[0-9]+$' ]] && (( turn >= RESUME_FLOOR )) \
      && [[ "$tag" != "$handoff_tag" ]]; then
    say "$reason; t${turn} is worth reloading, handing to the climb (INT civ6_play only)"
    handoff_tag="$tag"
    handoff_climb="$climb"
    handoff_polls=0
    player_uses_tag "$play" "$tag" \
      && kill -INT "$play" 2>/dev/null && say "  INT civ6_play $play"
    return 0
  fi
  say "$reason; restarting (TERM climb, then INT civ6_play)"
  kill -TERM "$climb" 2>/dev/null && say "  TERM climb $climb"
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    kill -0 "$climb" 2>/dev/null || break
    sleep 3
  done
  player_uses_tag "$play" "$tag" \
    && kill -INT "$play" 2>/dev/null && say "  INT civ6_play $play"
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
handoff_tag=""
handoff_climb=""
handoff_polls=0
last_unowned_tag=""
while true; do
  sleep "$POLL_S"
  tag=$(ls -t "$RUNS" 2>/dev/null | grep '^civvis-' | head -1)
  [[ -z "$tag" ]] && { strikes=0; reset_progress; continue }
  ownership=$(owned_climb_and_player || true)
  if [[ -z "$ownership" ]]; then
    # A direct player is expected during a protected autosave continuation.
    # Logging once per run retains diagnosis without turning an unowned game
    # into a six-minute stream of harmless noise.
    if [[ "$tag" != "$last_unowned_tag" ]] \
        && pgrep -f '[c]iv6_play\.py' >/dev/null 2>&1; then
      say "$tag has an unowned direct civ6_play; watchdog will not signal it"
      last_unowned_tag="$tag"
    fi
    if [[ -n "$handoff_tag" ]]; then
      handoff_polls=$(( handoff_polls + 1 ))
      if (( handoff_polls >= HANDOFF_GRACE )); then
        say "no player ${handoff_polls} poll(s) after handing $handoff_tag to the climb; TERM climb $handoff_climb"
        kill -TERM "$handoff_climb" 2>/dev/null
        handoff_tag=""; handoff_climb=""; handoff_polls=0
      fi
    fi
    strikes=0
    reset_progress
    continue
  fi
  read -r climb_pid play_pid <<< "$ownership"
  if [[ -n "$handoff_tag" && "$tag" != "$handoff_tag" ]]; then
    say "$tag is playing after the handoff of $handoff_tag; the reload recovered the match"
    handoff_tag=""; handoff_climb=""; handoff_polls=0
  fi
  if ! player_uses_tag "$play_pid" "$tag"; then
    say "$tag does not match the proven climb-owned player; leaving it alone"
    strikes=0
    reset_progress
    continue
  fi
  # A new game resets the count; never carry strikes across runs.
  [[ "$tag" != "$last_tag" ]] && {
    strikes=0
    reset_progress
    last_tag=$tag
    last_unowned_tag=""
  }

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
    restart_attempt "$tag repeating unit blocker ${blocker_name} at t${blocker_turn} (${blocker_count} sightings)" \
      "$climb_pid" "$play_pid" "$tag" "$blocker_turn"
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
        restart_attempt "$tag NO GAME PROGRESS confirmed at t${mirror_turn}" \
          "$climb_pid" "$play_pid" "$tag" "$mirror_turn"
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
      restart_attempt "$tag DEAD AGENT confirmed" "$climb_pid" "$play_pid" "$tag" "$mirror_turn"
      strikes=0
      reset_progress
    fi
  else
    (( strikes )) && say "$tag recovered (gap ${gap}); clearing strikes"
    strikes=0
  fi
done
