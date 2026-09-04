#!/bin/zsh
# Long-lived GUI-capable owner for the unattended Civ VI/CIVVIS session.
#
# This must be run in a Terminal shell, not launchd: this Mac's launchd context
# cannot write the Civ VI bundle or query its Accessibility window. Keep this
# shell open (the Terminal window may be hidden) so its children retain the
# necessary App Management and Accessibility responsibility.
set -u
# zsh sets BG_NICE by default: every `&` job starts at nice +5 and the whole
# subtree inherits it. On 2026-08-11 that put Civilization VI -- the one process
# the live ladder depends on -- underneath every nice-0 cargo build on the box
# (9-11 s/turn quiet, ~18 s/turn under fleet load), and macOS refuses to lower a
# nice once set, so a demoted game stays demoted for its whole run.
# civvis-keeper.sh had already found and fixed this for the exhibition lane; the
# live lane kept paying it. tools/test_ops_background_priority.py holds the line.
unsetopt BG_NICE

# Keep every helper beside this source-owned entry point. A previous copy in
# $HOME restarted an older supervisor after recovery, so the live loop and its
# repair path could silently run different revisions.
SELF_DIR=${0:A:h}
SUPERVISOR=${CIVVIS_SUPERVISOR:-${SELF_DIR}/civvis-game-supervisor.sh}
POPUP_KEEPER=${CIVVIS_POPUP_KEEPER:-${SELF_DIR}/civvis-popup-keeper.sh}
POPUP_KEEPER_LOCK=${CIVVIS_POPUP_KEEPER_LOCK:-$HOME/.civvis-popup-keeper.lock}
POPUP_KEEPER_PID_FILE=$POPUP_KEEPER_LOCK/pid
MIRROR_KEEPER=${CIVVIS_MIRROR_KEEPER:-${SELF_DIR}/civvis-mirror-keeper.sh}
WEDGE_WATCHDOG=${CIVVIS_WEDGE_WATCHDOG:-${SELF_DIR}/civvis-agent-wedge-watchdog.sh}
GAMELOCK=${CIVVIS_GAMELOCK:-${SELF_DIR:h}/civ6_control/gamelock.py}
GAMELOCK_PYTHON=${CIVVIS_GAMELOCK_PYTHON:-python3}
LOG=${CIVVIS_INTERACTIVE_HOST_LOG:-$HOME/civvis-civ6-runs/interactive_host.log}
LOCK=${CIVVIS_INTERACTIVE_HOST_LOCK:-$HOME/.civvis-interactive-host.lock}
PID_FILE=$LOCK/pid
SUPERVISOR_LOCK=${CIVVIS_SUPERVISOR_LOCK:-$HOME/.civvis-game-supervisor.lock}
SUPERVISOR_PID_FILE=$SUPERVISOR_LOCK/pid
POLL_S=${CIVVIS_INTERACTIVE_HOST_POLL_S:-5}
# The supervisor's fixed capture-free profile advances and dismisses native
# dialogs through the control mod.  Its popup keeper is deliberately visual
# automation, so inheriting it here would reintroduce screen capture into a
# recording-safe session before the supervisor gets a chance to select its
# capture-free owner.
CAPTURE_FREE=${CIVVIS_CAPTURE_FREE:-0}
INTENTFILE=${CIVVIS_OPERATOR_INTENT_FILE:-${CIVVIS_INTENTFILE:-$HOME/.civvis-operator-intent}}
supervisor_pid=""
supervisor_owned=0
popup_keeper_pid=""
popup_keeper_owned=0
mirror_keeper_pid=""
mirror_keeper_owned=0
WEDGE_WATCHDOG_LOCK=${CIVVIS_WEDGE_LOCK:-$HOME/.civvis-agent-wedge-watchdog.lock}
WEDGE_WATCHDOG_PID_FILE=$WEDGE_WATCHDOG_LOCK/pid
wedge_watchdog_pid=""
wedge_watchdog_owned=0

say() { print -r -- "[interactive-host] $(date -u +%FT%TZ) $*" >> "$LOG" }

verification_intent_running() {
  [[ -r "$INTENTFILE" ]] && [[ "$(<"$INTENTFILE")" == running ]]
}

intent_reason() {
  if [[ -r "$INTENTFILE" ]]; then
    print -r -- "intent=$(<"$INTENTFILE")"
  else
    print -r -- "intent=missing"
  fi
}

hold_status() {
  # A missing or unreadable lock helper must not turn an explicit halt into a
  # fresh game.  Treat an inspection failure as a hold until an operator can
  # resolve it, while the normal no-hold answer remains exit 1.
  #
  # ⚠⚠ THIS ASKS FOR THE EXPLICIT, DURABLE HALT AND NOTHING ELSE. It used to
  # ask `--hold-status`, whose answer also covers a lock whose live holder
  # drives no run — a state the batch loop passes through for a few seconds
  # between one attempt's exit and the next attempt's launch. Polled every five
  # seconds, that read as "operator halt active", this host stopped its own
  # supervisor, and the batch loop and the game under it died with it: four
  # games on 2026-08-18/19 ended at t18/t44/t72/t83 as `game exited` within
  # seconds of such a line in this log. Stopping the machine is the operator's
  # marker's job; a standing hold is a report for the keeper, not an order.
  if [[ ! -f "$GAMELOCK" ]]; then
    print -r -- "cannot inspect operator halt: missing $GAMELOCK"
    return 0
  fi
  local output="" rc=0
  output=$("$GAMELOCK_PYTHON" "$GAMELOCK" --halt-status 2>&1)
  rc=$?
  case "$rc" in
    0) print -r -- "$output"; return 0 ;;
    1) return 1 ;;
    *) print -r -- "cannot inspect operator halt: ${output:-exit $rc}"; return 0 ;;
  esac
}

release_lock() {
  local holder=""
  [[ -f "$PID_FILE" ]] && holder=$(<"$PID_FILE")
  [[ "$holder" == "$$" ]] && rm -rf -- "$LOCK"
}

stop_owned_process_tree() {
  local signal=$1 pid=$2 child
  # The background command can have a zsh job wrapper, caffeinate, and then the
  # actual supervisor below the PID returned by `$!`. Signal descendants first:
  # killing only the wrapper lets a deeper child become launchd-owned and keep
  # running after this host exits. A process-group kill is not safe here: a
  # Terminal-launched shell can put the background job in the host's group.
  for child in ${(f)"$(pgrep -P "$pid" 2>/dev/null)"}; do
    stop_owned_process_tree "$signal" "$child"
  done
  kill -"$signal" "$pid" 2>/dev/null || true
}

stop_children() {
  # An audit can reopen this host after the ladder launcher has already started
  # a valid supervisor. Adopt that supervisor, but never signal it on host exit:
  # this host did not create it and may otherwise end a healthy game.
  (( mirror_keeper_owned )) && [[ -n "$mirror_keeper_pid" ]] \
      && stop_owned_process_tree TERM "$mirror_keeper_pid"
  (( wedge_watchdog_owned )) && [[ -n "$wedge_watchdog_pid" ]] \
      && stop_owned_process_tree TERM "$wedge_watchdog_pid"
  (( popup_keeper_owned )) && [[ -n "$popup_keeper_pid" ]] \
      && stop_owned_process_tree TERM "$popup_keeper_pid"
  (( supervisor_owned )) && [[ -n "$supervisor_pid" ]] \
      && stop_owned_process_tree TERM "$supervisor_pid"
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

pid_is_live() {
  [[ -n "$1" ]] && kill -0 "$1" 2>/dev/null
}

live_supervisor_pid() {
  local holder="" command=""
  [[ -r "$SUPERVISOR_PID_FILE" ]] || return 1
  holder=$(<"$SUPERVISOR_PID_FILE")
  case "$holder" in
    ''|*[!0-9]*) return 1 ;;
  esac
  pid_is_live "$holder" || return 1
  command=$(ps -p "$holder" -o command= 2>/dev/null)
  [[ "$command" == *"${SUPERVISOR:t}"* ]] || return 1
  print -r -- "$holder"
}

start_supervisor() {
  local existing=""
  if ! verification_intent_running; then
    say "verification intent is not running; refusing to start game supervisor ($(intent_reason))"
    return 1
  fi
  existing=$(live_supervisor_pid || true)
  if [[ -n "$existing" ]]; then
    supervisor_pid=$existing
    supervisor_owned=0
    say "adopted already-live game supervisor pid $supervisor_pid"
    return 0
  fi
  if [[ ! -f "$SUPERVISOR" ]]; then
    say "cannot start game supervisor: missing $SUPERVISOR"
    supervisor_pid=""
    supervisor_owned=0
    return 1
  fi
  /usr/bin/caffeinate -dims /bin/zsh "$SUPERVISOR" \
      >>"$HOME/civvis-game-supervisor.interactive.log" 2>&1 &
  supervisor_pid=$!
  supervisor_owned=1
  say "started game supervisor pid $supervisor_pid"
}

live_popup_keeper_pid() {
  local holder="" command=""
  [[ -r "$POPUP_KEEPER_PID_FILE" ]] || return 1
  holder=$(<"$POPUP_KEEPER_PID_FILE")
  case "$holder" in
    ''|*[!0-9]*) return 1 ;;
  esac
  pid_is_live "$holder" || return 1
  command=$(ps -p "$holder" -o command= 2>/dev/null)
  [[ "$command" == *"${POPUP_KEEPER:t}"* ]] || return 1
  print -r -- "$holder"
}

start_popup_keeper() {
  local existing=""
  existing=$(live_popup_keeper_pid || true)
  if [[ -n "$existing" ]]; then
    popup_keeper_pid=$existing
    popup_keeper_owned=0
    say "adopted popup keeper pid $popup_keeper_pid"
    return 0
  fi
  if [[ ! -f "$POPUP_KEEPER" ]]; then
    say "cannot start popup keeper: missing $POPUP_KEEPER"
    popup_keeper_pid=""
    popup_keeper_owned=0
    return 1
  fi
  /bin/zsh "$POPUP_KEEPER" \
      >>"$HOME/civvis-civ6-runs/popup_clear.keeper.launch.log" 2>&1 &
  popup_keeper_pid=$!
  popup_keeper_owned=1
  say "started popup keeper pid $popup_keeper_pid"
}

start_managed_helpers() {
  if [[ "$CAPTURE_FREE" == 1 ]]; then
    say "capture-free session: visual popup keeper disabled"
  else
    start_popup_keeper || true
  fi
  start_mirror_keeper || true
  start_wedge_watchdog || true
}

start_mirror_keeper() {
  local holder=""
  [[ -f "$HOME/.civvis-mirror-keeper.lock/pid" ]] && holder=$(<"$HOME/.civvis-mirror-keeper.lock/pid")
  if pid_is_live "$holder"; then
    mirror_keeper_pid=$holder
    mirror_keeper_owned=0
    say "adopted mirror keeper pid $mirror_keeper_pid"
    return 0
  fi
  if [[ ! -f "$MIRROR_KEEPER" ]]; then
    say "cannot start mirror keeper: missing $MIRROR_KEEPER"
    mirror_keeper_pid=""
    mirror_keeper_owned=0
    return 1
  fi
  /bin/zsh "$MIRROR_KEEPER" \
      >>"$HOME/civvis-civ6-mirror/mirror-keeper.launch.log" 2>&1 &
  mirror_keeper_pid=$!
  mirror_keeper_owned=1
  say "started mirror keeper pid $mirror_keeper_pid"
}

live_wedge_watchdog_pid() {
  local holder="" command=""
  [[ -r "$WEDGE_WATCHDOG_PID_FILE" ]] || return 1
  holder=$(<"$WEDGE_WATCHDOG_PID_FILE")
  case "$holder" in
    ''|*[!0-9]*) return 1 ;;
  esac
  pid_is_live "$holder" || return 1
  command=$(ps -p "$holder" -o command= 2>/dev/null)
  [[ "$command" == *"${WEDGE_WATCHDOG:t}"* ]] || return 1
  print -r -- "$holder"
}

start_wedge_watchdog() {
  local existing=""
  existing=$(live_wedge_watchdog_pid || true)
  if [[ -n "$existing" ]]; then
    wedge_watchdog_pid=$existing
    wedge_watchdog_owned=0
    say "adopted live agent wedge watchdog pid $wedge_watchdog_pid"
    return 0
  fi
  if [[ ! -f "$WEDGE_WATCHDOG" ]]; then
    say "cannot start agent wedge watchdog: missing $WEDGE_WATCHDOG"
    wedge_watchdog_pid=""
    wedge_watchdog_owned=0
    return 1
  fi
  /bin/zsh "$WEDGE_WATCHDOG" \
      >>"$HOME/civvis-civ6-runs/agent_wedge_watchdog.launch.log" 2>&1 &
  wedge_watchdog_pid=$!
  wedge_watchdog_owned=1
  say "started agent wedge watchdog pid $wedge_watchdog_pid"
}

say "host up (pid $$)"
if ! verification_intent_running; then
  say "verification intent is not running; exiting before startup: $(intent_reason)"
  exit 0
elif held=$(hold_status); then
  say "operator halt active; exiting before startup: $held"
  exit 0
elif start_supervisor; then
  if (( supervisor_owned )); then
    start_managed_helpers
  else
    say "external supervisor already owns the live batch; not starting duplicate helpers"
  fi
fi

while true; do
  if ! verification_intent_running; then
    say "verification intent is not running; stopping owned children and exiting: $(intent_reason)"
    exit 0
  elif held=$(hold_status); then
    say "operator halt active; stopping owned children and exiting: $held"
    exit 0
  fi
  if ! pid_is_live "$supervisor_pid"; then
    say "game supervisor exited; restarting"
    if start_supervisor && (( supervisor_owned )); then
      start_managed_helpers
    fi
  fi
  if (( supervisor_owned )); then
    if [[ "$CAPTURE_FREE" != 1 ]] && ! pid_is_live "$popup_keeper_pid"; then
      say "popup keeper exited; restarting"
      start_popup_keeper || true
    fi
    if ! pid_is_live "$mirror_keeper_pid"; then
      say "mirror keeper exited; restarting"
      start_mirror_keeper || true
    fi
    if ! pid_is_live "$wedge_watchdog_pid"; then
      say "agent wedge watchdog exited; restarting"
      start_wedge_watchdog || true
    fi
  fi
  sleep "$POLL_S"
done
