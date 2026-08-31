#!/bin/zsh
# Activate an updated CIVVIS supervisor only in a verified no-game gap.
#
# This one-shot helper freezes the exact supervisor before its final check, so
# it cannot launch a game between a passive check and TERM. It never signals
# Civ VI, civ6_play.py, or the climb runner; if any is present, it waits.
set -u

SUPERVISOR=${CIVVIS_SUPERVISOR:-$HOME/civvis-game-supervisor.sh}
SUPERVISOR_LOCK=${CIVVIS_SUPERVISOR_LOCK:-$HOME/.civvis-game-supervisor.lock}
SUPERVISOR_PID_FILE=$SUPERVISOR_LOCK/pid
GAME_HOLDER=${CIVVIS_GAME_HOLDER:-$HOME/.civvis-civ6-game.lock/holder.json}
LOCK=${CIVVIS_SUPERVISOR_RELOAD_LOCK:-$HOME/.civvis-supervisor-safe-reload.lock}
PID_FILE=$LOCK/pid
LOG=${CIVVIS_SUPERVISOR_RELOAD_LOG:-$HOME/civvis-climb-logs/supervisor-safe-reload.log}
POLL_S=${CIVVIS_SUPERVISOR_RELOAD_POLL_S:-5}
INTENTFILE=${CIVVIS_OPERATOR_INTENT_FILE:-${CIVVIS_INTENTFILE:-$HOME/.civvis-operator-intent}}

say() { print -r -- "[supervisor-safe-reload] $(date -u +%FT%TZ) $*" >> "$LOG"; }

verification_intent_running() {
  [[ -r "$INTENTFILE" ]] && [[ "$(<"$INTENTFILE")" == running ]]
}

release_lock() {
  local holder=""
  [[ -f "$PID_FILE" ]] && holder=$(<"$PID_FILE")
  [[ "$holder" == "$$" ]] && rm -rf -- "$LOCK"
}

acquire_lock() {
  if mkdir "$LOCK" 2>/dev/null; then
    print -r -- "$$" > "$PID_FILE"
    return 0
  fi
  local holder=""
  [[ -f "$PID_FILE" ]] && holder=$(<"$PID_FILE")
  if [[ -n "$holder" ]] && kill -0 "$holder" 2>/dev/null; then
    return 1
  fi
  rm -rf -- "$LOCK"
  mkdir "$LOCK" 2>/dev/null || return 2
  print -r -- "$$" > "$PID_FILE"
}

supervisor_pid() {
  local pid="" command=""
  [[ -r "$SUPERVISOR_PID_FILE" ]] || return 1
  pid=$(<"$SUPERVISOR_PID_FILE")
  case "$pid" in
    ''|*[!0-9]*) return 1 ;;
  esac
  kill -0 "$pid" 2>/dev/null || return 1
  command=$(ps -p "$pid" -o command= 2>/dev/null)
  [[ "$command" == *"$SUPERVISOR"* ]] || return 1
  print -r -- "$pid"
}

game_harness_alive() {
  local pid="" command=""
  [[ -r "$GAME_HOLDER" ]] || return 1
  pid=$(sed -nE 's/^[[:space:]]*"pid"[[:space:]]*:[[:space:]]*([0-9]+).*/\1/p' "$GAME_HOLDER" | head -n 1)
  case "$pid" in
    ''|*[!0-9]*) return 1 ;;
  esac
  kill -0 "$pid" 2>/dev/null || return 1
  command=$(ps -p "$pid" -o command= 2>/dev/null)
  [[ "$command" == *"civ6_play.py"* ]]
}

# A launch/build child means a game can be started imminently, so leave the
# supervisor alone. A plain `sleep` child is the safe inter-batch state.
supervisor_is_launching() {
  local supervisor=$1 child="" command=""
  for child in ${(f)"$(pgrep -P "$supervisor" 2>/dev/null)"}; do
    command=$(ps -p "$child" -o command= 2>/dev/null)
    if [[ "$command" == *"civ6_civvis_climb.py"* || "$command" == *"cargo "* || "$command" == *"rustc "* || "$command" == *" git "* || "$command" == *"/git "* ]]; then
      return 0
    fi
  done
  return 1
}

# macOS caffeinate forks its assertion helper under the command shell. If the
# shell exits during this deliberate reload, clean up only that exact orphan;
# the host starts a fresh assertion together with the replacement supervisor.
supervisor_caffeinate_pid() {
  local supervisor=$1 child="" command=""
  for child in ${(f)"$(pgrep -P "$supervisor" 2>/dev/null)"}; do
    command=$(ps -p "$child" -o command= 2>/dev/null)
    if [[ "$command" == *"/usr/bin/caffeinate -dims /bin/zsh $SUPERVISOR"* ]]; then
      print -r -- "$child"
      return 0
    fi
  done
  return 1
}

mkdir -p "${LOG:h}"
if ! verification_intent_running; then
  say "verification intent is not running; no supervisor reload requested"
  exit 0
fi
acquire_lock
case $? in
  0) ;;
  1) exit 0 ;;
  *) exit 70 ;;
esac
trap release_lock EXIT
trap 'exit 0' HUP INT TERM

say "watching for a no-game gap"
while true; do
  if ! verification_intent_running; then
    say "verification intent is not running; abandoning reload without touching the supervisor"
    exit 0
  fi
  supervisor=$(supervisor_pid) || { say "supervisor is already absent; no activation needed"; exit 0; }
  if game_harness_alive || supervisor_is_launching "$supervisor"; then
    sleep "$POLL_S"
    continue
  fi

  # Stop first, then repeat every predicate with the parent unable to launch.
  old_caffeinate=$(supervisor_caffeinate_pid "$supervisor" || true)
  if ! verification_intent_running; then
    say "verification intent changed before reload; leaving supervisor untouched"
    exit 0
  fi
  kill -STOP "$supervisor" 2>/dev/null || { sleep "$POLL_S"; continue; }
  sleep 1
  current=$(supervisor_pid || true)
  if [[ "$current" != "$supervisor" ]] || game_harness_alive || supervisor_is_launching "$supervisor"; then
    [[ "$current" == "$supervisor" ]] && kill -CONT "$supervisor" 2>/dev/null || true
    sleep "$POLL_S"
    continue
  fi

  say "verified idle gap; requesting supervisor reload (pid $supervisor)"
  kill -TERM "$supervisor" 2>/dev/null || true
  kill -CONT "$supervisor" 2>/dev/null || true
  for _ in {1..30}; do
    replacement=$(supervisor_pid || true)
    if [[ -n "$replacement" && "$replacement" != "$supervisor" ]]; then
      if [[ -n "$old_caffeinate" ]] && kill -0 "$old_caffeinate" 2>/dev/null; then
        command=$(ps -p "$old_caffeinate" -o command= 2>/dev/null)
        if [[ "$command" == *"/usr/bin/caffeinate -dims /bin/zsh $SUPERVISOR"* ]]; then
          kill -TERM "$old_caffeinate" 2>/dev/null || true
        fi
      fi
      say "replacement supervisor $replacement confirmed"
      exit 0
    fi
    sleep 1
  done

  # Do not escalate a failed graceful stop. Resume it and try another verified
  # gap later rather than risking a healthy game or a wedged Civ VI core.
  kill -CONT "$supervisor" 2>/dev/null || true
  say "supervisor did not reload cleanly; leaving it running and retrying later"
  sleep "$POLL_S"
done
