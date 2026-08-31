#!/bin/zsh
# Bounded, non-invasive health audit for the unattended Civ VI/CIVVIS session.
#
# This runs from the recurring monitor. It never restarts a healthy app,
# controller, mirror, or browser. The interactive host owns its children; the
# only recoveries here reopen proven-absent helpers through Terminal's
# GUI-capable context.

set -u
# zsh sets BG_NICE by default: every `&` job starts at nice +5 and the whole
# subtree inherits it. On 2026-08-11 that put Civilization VI -- the one process
# the live ladder depends on -- underneath every nice-0 cargo build on the box
# (9-11 s/turn quiet, ~18 s/turn under fleet load), and macOS refuses to lower a
# nice once set, so a demoted game stays demoted for its whole run.
# civvis-keeper.sh had already found and fixed this for the exhibition lane; the
# live lane kept paying it. tools/test_ops_background_priority.py holds the line.
unsetopt BG_NICE

BASE=$HOME
SELF_DIR=${0:A:h}
RUNS=$BASE/civvis-civ6-runs/control
PLAY_LOGS=$BASE/civvis-climb-logs
AUDIT_LOG=$BASE/civvis-civ6-runs/overnight_audit.log
HOST_LAUNCHER=${CIVVIS_HOST_LAUNCHER:-$SELF_DIR/civvis-ladder-terminal-launcher.sh}
MIRROR_KEEPER=${CIVVIS_MIRROR_KEEPER:-$SELF_DIR/civvis-mirror-keeper.sh}
# The keeper is tracked beside this script now; the old home-directory copy
# was an untracked deployment that outlived every teardown.
DISPLAY_KEEPER=${CIVVIS_DISPLAY_KEEPER:-$SELF_DIR/civvis-display-keeper.mjs}
SUPERVISOR_LOCK=${CIVVIS_SUPERVISOR_LOCK:-$BASE/.civvis-game-supervisor.lock}
SUPERVISOR_PID_FILE=$SUPERVISOR_LOCK/pid
JQ=/opt/homebrew/bin/jq
[[ -x "$JQ" ]] || JQ=$(command -v jq 2>/dev/null || true)
EVENT_FRESH_S=${CIVVIS_OVERNIGHT_EVENT_FRESH_S:-180}
# The mirror keeper itself allows a planned follower replacement to settle.
# Match that policy here so an audit during a just-seated game's first export
# reports a real fault, not a harmless few-second server handoff.
MIRROR_SETTLE_S=${CIVVIS_OVERNIGHT_MIRROR_SETTLE_S:-20}
# A completed configured game spends several minutes rebuilding and seating the
# next one.  Treat that bounded interval as a handoff, but surface a real
# warning if it lasts longer than the normal bootstrap allowance.
TURN_CAP=${CIVVIS_OVERNIGHT_TURN_CAP:-250}
HANDOFF_GRACE_S=${CIVVIS_OVERNIGHT_HANDOFF_GRACE_S:-900}

say() { print -r -- "[overnight-audit] $(date -u +%FT%TZ) $*" >> "$AUDIT_LOG"; }

first_pid() {
  /usr/bin/pgrep -f "$1" 2>/dev/null | /usr/bin/head -n 1 || true
}

newest_events() {
  local -a candidates
  candidates=("$RUNS"/*/events.jsonl(N))
  (( ${#candidates} > 0 )) || return 0
  /bin/ls -t "${candidates[@]}" 2>/dev/null | /usr/bin/head -n 1 || true
}

start_host() {
  # Only the host needs Terminal's App Management grant. `-g -j` avoids
  # stealing Civ VI's foreground or leaving a recovery window in the way.
  /usr/bin/open -g -j -a Terminal "$HOST_LAUNCHER" >/dev/null 2>&1
}

start_mirror_keeper() {
  # The keeper may need to restore Chrome through AppleScript, so it needs
  # Terminal's GUI grant. Keep that recovery hidden and out of Civ VI's
  # foreground rather than opening an ordinary Terminal window.
  /usr/bin/open -g -j -a Terminal "$MIRROR_KEEPER" >/dev/null 2>&1
}

start_display_keeper() {
  # This Node keeper needs no Accessibility grant; detach it directly instead
  # of creating another visible shell solely to hold it.
  /usr/bin/nohup /opt/homebrew/bin/node "$DISPLAY_KEEPER" \
      >>"$BASE/civvis-civ6-mirror/display-keeper.launch.log" 2>&1 &
}

live_supervisor_pid() {
  local holder="" command=""
  [[ -r "$SUPERVISOR_PID_FILE" ]] || return 1
  holder=$(<"$SUPERVISOR_PID_FILE")
  case "$holder" in
    ''|*[!0-9]*) return 1 ;;
  esac
  kill -0 "$holder" 2>/dev/null || return 1
  command=$(ps -p "$holder" -o command= 2>/dev/null)
  [[ "$command" == *"civvis-game-supervisor.sh"* ]] || return 1
  print -r -- "$holder"
}

frontmost_app() {
  /usr/bin/osascript -e 'tell application "System Events" to get name of first process whose frontmost is true' 2>/dev/null || true
}

focus_civ6() {
  /usr/bin/osascript -e 'tell application "System Events" to set frontmost of (first process whose name contains "Civ6") to true' >/dev/null 2>&1
}

mirror_status=''
mirror_turn='?'
mirror_painted='?'
mirror_viewers='?'
mirror_reachable=0
mirror_healthy=0

collect_mirror() {
  mirror_status=$(/usr/bin/curl -fsS --connect-timeout 2 --max-time 4 http://127.0.0.1:8610/status 2>/dev/null || true)
  mirror_turn='?'
  mirror_painted='?'
  mirror_viewers='?'
  mirror_reachable=0
  mirror_healthy=0
  [[ -n "$mirror_status" ]] && mirror_reachable=1
  [[ -n "$mirror_status" && -n "$JQ" ]] || return
  mirror_turn=$(print -r -- "$mirror_status" | "$JQ" -r '.turn // "?"' 2>/dev/null || print -r -- '?')
  mirror_painted=$(print -r -- "$mirror_status" | "$JQ" -r '.frames_painted // "?"' 2>/dev/null || print -r -- '?')
  mirror_viewers=$(print -r -- "$mirror_status" | "$JQ" -r '.viewers // "?"' 2>/dev/null || print -r -- '?')
  if [[ "$mirror_turn" == <-> && "$mirror_painted" == <-> && "$mirror_viewers" == <-> ]] \
      && (( mirror_viewers >= 1 && mirror_painted >= mirror_turn - 1 )); then
    mirror_healthy=1
  fi
}

mirror_needs_settle() {
  # The backup keeper is deliberately absent while the primary follower is
  # serving a healthy, painted mirror.  Wait only for a missing follower or an
  # unhealthy mirror that may still be settling.
  [[ -z "$follower_pid" ]] || (( ! mirror_healthy ))
}

display_page=0
collect_display_page() {
  local pages
  display_page=0
  [[ -n "$JQ" ]] || return
  pages=$(/usr/bin/curl -fsS --connect-timeout 2 --max-time 4 http://127.0.0.1:9230/json/list 2>/dev/null || true)
  [[ -n "$pages" ]] || return
  if print -r -- "$pages" | "$JQ" -e 'any(.[]; .type == "page" and (.url | contains("127.0.0.1:8610")))' >/dev/null 2>&1; then
    display_page=1
  fi
}

typeset -a warnings actions
warnings=()
actions=()

# The durable operator halt (`gamelock.py --halt`). This audit's whole job is
# restarting pieces of the game stack that have gone missing — host, mirror
# keeper, display keeper — which is exactly what must NOT happen while the
# operator has the machine's games stopped. Every keeper it would revive is a
# window or an input driver over whatever the operator is doing instead.
operator_halt=${CIVVIS_OPERATOR_HALT_FILE:-$HOME/.civvis-operator-halt.json}
if [[ -e "$operator_halt" ]]; then
  say "operator halt in force; auditing nothing and starting nothing"
  exit 0
fi

host_pid=$(first_pid '[c]ivvis-interactive-host\.sh')
supervisor_pid=$(live_supervisor_pid || true)
host_state=up
if [[ -z "$host_pid" ]]; then
  if [[ -n "$supervisor_pid" ]]; then
    # A legacy launcher can own a healthy batch without an interactive host.
    # Starting another host here used to create a competing supervisor every
    # five seconds; the next true outage will reopen the single managed host.
    host_state=supervisor-only
  elif start_host; then
    actions+=(host_reopened)
    sleep 5
    host_pid=$(first_pid '[c]ivvis-interactive-host\.sh')
  fi
  if [[ -z "$host_pid" && "$host_state" != supervisor-only ]]; then
    host_state=absent
    warnings+=(host_absent)
  elif [[ -n "$host_pid" ]]; then
    host_state=reopened
  fi
fi

events=$(newest_events)
event_age='?'
event_turn='?'
live_events=0
tiles_exported=0
if [[ -n "$events" && -f "$events" ]]; then
  now=$(date +%s)
  event_age=$(( now - $(stat -f %m "$events") ))
  event_turn=$(/usr/bin/grep '"kind": "turn"' "$events" 2>/dev/null | /usr/bin/tail -n 1 | /usr/bin/sed -E 's/.*"turn": ([0-9]+).*/\1/' || true)
  [[ "$event_turn" == <-> ]] || event_turn='?'
  (( event_age <= EVENT_FRESH_S )) && live_events=1
  /usr/bin/grep -q '"kind": "tiles"' "$events" 2>/dev/null && tiles_exported=1
fi

civ_pid=$(first_pid '[C]iv6_Exe_Child')
popup_pid=$(first_pid '[p]opup_clear\.py')
handoff_expected=0
if [[ -z "$civ_pid" && "$event_turn" == <-> && "$event_age" == <-> ]] \
    && (( event_turn >= TURN_CAP )); then
  if (( event_age <= HANDOFF_GRACE_S )); then
    handoff_expected=1
  else
    warnings+=(handoff_stalled)
  fi
fi
if (( handoff_expected )); then
  game_state="T${event_turn}:handoff-age${event_age}s"
else
  game_state="T${event_turn}:age${event_age}s"
  [[ -n "$civ_pid" ]] || game_state+='/civ-absent'
fi
foreground_name=$(frontmost_app)
foreground_state=${foreground_name:-unknown}
if (( live_events )) && [[ -n "$civ_pid" && "$foreground_name" != *Civ6* ]]; then
  if focus_civ6; then
    sleep 1
    foreground_name=$(frontmost_app)
  fi
  if [[ "$foreground_name" == *Civ6* ]]; then
    foreground_state=Civ6
    actions+=(civ6_refocused)
  else
    warnings+=(civ6_not_foreground)
  fi
fi
if (( live_events && ! handoff_expected )) && [[ -z "$civ_pid" ]]; then
  warnings+=(fresh_events_without_civ)
fi
if (( live_events )) && [[ -z "$popup_pid" ]]; then
  warnings+=(fresh_events_without_popup_clear)
fi

collect_mirror
mirror_keeper_pid=$(first_pid '[c]ivvis-mirror-keeper\.sh')
follower_pid=$(first_pid '[t]ools/follow\.py')
if (( live_events && tiles_exported && ! mirror_healthy )) \
    && [[ -n "$host_pid" && -z "$mirror_keeper_pid" ]]; then
  # A healthy mirror proves the live follower is already serving frames; a
  # separate keeper is redundant. Only an actual mirror outage warrants the
  # hidden GUI-capable recovery window below.
  sleep 8
  mirror_keeper_pid=$(first_pid '[c]ivvis-mirror-keeper\.sh')
  collect_mirror
  if [[ -z "$mirror_keeper_pid" ]] && (( ! mirror_healthy )) \
      && start_mirror_keeper; then
    actions+=(mirror_keeper_reopened)
    sleep 5
    mirror_keeper_pid=$(first_pid '[c]ivvis-mirror-keeper\.sh')
  fi
fi

collect_mirror
if (( live_events && tiles_exported )) \
    && mirror_needs_settle; then
  sleep "$MIRROR_SETTLE_S"
  mirror_keeper_pid=$(first_pid '[c]ivvis-mirror-keeper\.sh')
  follower_pid=$(first_pid '[t]ools/follow\.py')
  collect_mirror
fi
if (( live_events && tiles_exported )); then
  if (( ! mirror_healthy )); then
    [[ -n "$mirror_keeper_pid" ]] || warnings+=(mirror_keeper_absent)
    [[ -n "$follower_pid" ]] || warnings+=(follower_absent)
    warnings+=(mirror_not_ready)
  fi
fi

display_pid=$(first_pid '[c]ivvis-display-keeper\.mjs')
collect_display_page
if (( live_events && tiles_exported && mirror_reachable )) && [[ -z "$display_pid" ]]; then
  sleep 5
  display_pid=$(first_pid '[c]ivvis-display-keeper\.mjs')
  if [[ -z "$display_pid" ]] && start_display_keeper; then
    actions+=(display_keeper_reopened)
    sleep 8
    display_pid=$(first_pid '[c]ivvis-display-keeper\.mjs')
    collect_display_page
  fi
fi

# The display keeper performs a guarded CDP reload after 25 seconds. Let that
# recovery complete once before reporting a true paint/page warning.
if (( live_events && tiles_exported && mirror_reachable )) && (( ! display_page || ! mirror_healthy )); then
  sleep 30
  collect_mirror
  collect_display_page
fi
if (( live_events && tiles_exported && mirror_reachable )); then
  [[ -n "$display_pid" ]] || warnings+=(display_keeper_absent)
  (( display_page )) || warnings+=(display_page_absent)
fi

if (( mirror_healthy )); then
  mirror_state="T${mirror_turn}/paint${mirror_painted}/viewers${mirror_viewers}"
elif [[ -n "$mirror_status" ]]; then
  mirror_state="T${mirror_turn}/paint${mirror_painted}/viewers${mirror_viewers}/unhealthy"
else
  mirror_state=unavailable
fi
if [[ -n "$display_pid" && $display_page -eq 1 ]]; then
  display_state=keeper+page
elif [[ -n "$display_pid" ]]; then
  display_state=keeper/page-missing
else
  display_state=keeper-absent
fi

if /usr/bin/pmset -g batt 2>/dev/null | /usr/bin/grep -q 'AC Power'; then
  power_state=AC
else
  power_state=battery
fi
sleep_assertion=$(/usr/bin/pmset -g assertions 2>/dev/null | /usr/bin/awk '/PreventSystemSleep/ {print $2; exit}')
[[ "$sleep_assertion" == 1 ]] || warnings+=(sleep_assertion_missing)
disk_free=$(/bin/df -h "$BASE" | /usr/bin/awk 'NR == 2 {print $4}')

warning_text=none
(( ${#warnings} )) && warning_text="${(j:,:)warnings}"
action_text=none
(( ${#actions} )) && action_text="${(j:,:)actions}"
line="host=${host_state}; game=${game_state}; mirror=${mirror_state}; display=${display_state}; foreground=${foreground_state}; power=${power_state}+sleep${sleep_assertion:-?}; disk=${disk_free:-?}; action=${action_text}; warning=${warning_text}"
say "$line"
print -r -- "$line"
