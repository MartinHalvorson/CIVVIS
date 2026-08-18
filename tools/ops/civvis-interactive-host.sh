#!/bin/zsh
# Long-lived GUI-capable owner for the unattended Civ VI/CIVVIS session.
#
# This must be run in a Terminal shell, not launchd: this Mac's launchd context
# cannot write the Civ VI bundle or query its Accessibility window. Keep this
# shell open (the Terminal window may be hidden) so its children retain the
# necessary App Management and Accessibility responsibility.
set -u

SUPERVISOR=$HOME/civvis-game-supervisor.sh
POPUP_KEEPER=$HOME/civvis-popup-keeper.sh
MIRROR_KEEPER=$HOME/civvis-mirror-keeper.sh
LOG=$HOME/civvis-civ6-runs/interactive_host.log
LOCK=${CIVVIS_INTERACTIVE_HOST_LOCK:-$HOME/.civvis-interactive-host.lock}
PID_FILE=$LOCK/pid
supervisor_pid=""
popup_keeper_pid=""
mirror_keeper_pid=""

say() { print -r -- "[interactive-host] $(date -u +%FT%TZ) $*" >> "$LOG" }

release_lock() {
  local holder=""
  [[ -f "$PID_FILE" ]] && holder=$(<"$PID_FILE")
  [[ "$holder" == "$$" ]] && rm -rf -- "$LOCK"
}

stop_children() {
  [[ -n "$mirror_keeper_pid" ]] && kill -TERM "$mirror_keeper_pid" 2>/dev/null || true
  [[ -n "$popup_keeper_pid" ]] && kill -TERM "$popup_keeper_pid" 2>/dev/null || true
  [[ -n "$supervisor_pid" ]] && kill -TERM "$supervisor_pid" 2>/dev/null || true
}

if ! mkdir "$LOCK" 2>/dev/null; then
  holder=""
  [[ -f "$PID_FILE" ]] && holder=$(<"$PID_FILE")
  if [[ -n "$holder" ]] && kill -0 "$holder" 2>/dev/null; then
    say "another interactive host is already alive (pid $holder); exiting"
    exit 0
  fi
  rm -rf -- "$LOCK"
  if ! mkdir "$LOCK" 2>/dev/null; then
    say "could not acquire interactive host lock"
    exit 70
  fi
fi
print -r -- "$$" > "$PID_FILE"
trap 'stop_children; release_lock' EXIT
trap 'exit 0' HUP INT TERM

start_supervisor() {
  /usr/bin/caffeinate -dims /bin/zsh "$SUPERVISOR" >>$HOME/civvis-game-supervisor.interactive.log 2>&1 &
  supervisor_pid=$!
  say "started game supervisor pid $supervisor_pid"
}

start_popup_keeper() {
  /bin/zsh "$POPUP_KEEPER" >>$HOME/civvis-civ6-runs/popup_clear.keeper.launch.log 2>&1 &
  popup_keeper_pid=$!
  say "started popup keeper pid $popup_keeper_pid"
}

start_mirror_keeper() {
  local holder=""
  [[ -f "$HOME/.civvis-mirror-keeper.lock/pid" ]] && holder=$(<"$HOME/.civvis-mirror-keeper.lock/pid")
  if [[ -n "$holder" ]] && kill -0 "$holder" 2>/dev/null; then
    mirror_keeper_pid=$holder
    say "adopted mirror keeper pid $mirror_keeper_pid"
    return
  fi
  /bin/zsh "$MIRROR_KEEPER" >>$HOME/civvis-civ6-mirror/mirror-keeper.launch.log 2>&1 &
  mirror_keeper_pid=$!
  say "started mirror keeper pid $mirror_keeper_pid"
}

say "host up (pid $$)"
start_supervisor
start_popup_keeper
start_mirror_keeper

while true; do
  if [[ -z "$supervisor_pid" ]] || ! kill -0 "$supervisor_pid" 2>/dev/null; then
    say "game supervisor exited; restarting"
    start_supervisor
  fi
  if [[ -z "$popup_keeper_pid" ]] || ! kill -0 "$popup_keeper_pid" 2>/dev/null; then
    say "popup keeper exited; restarting"
    start_popup_keeper
  fi
  if [[ -z "$mirror_keeper_pid" ]] || ! kill -0 "$mirror_keeper_pid" 2>/dev/null; then
    say "mirror keeper exited; restarting"
    start_mirror_keeper
  fi
  sleep 5
done
