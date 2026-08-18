#!/bin/zsh
# Bounded, non-invasive health audit for the unattended Civ VI/CIVVIS session.
#
# This runs from the recurring monitor. It never restarts a healthy app,
# controller, mirror, or browser. The interactive host owns its children; the
# only recoveries here reopen proven-absent helpers through Terminal's
# GUI-capable context.

set -u

BASE=$HOME
RUNS=$BASE/civvis-civ6-runs/control
PLAY_LOGS=$BASE/civvis-climb-logs
AUDIT_LOG=$BASE/civvis-civ6-runs/overnight_audit.log
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
  /usr/bin/osascript -e 'tell application "Terminal" to do script "exec /bin/zsh '"$HOME"'/civvis-interactive-host.sh"' >/dev/null 2>&1
}

start_mirror_keeper() {
  /usr/bin/osascript -e 'tell application "Terminal" to do script "exec /bin/zsh '"$HOME"'/civvis-mirror-keeper.sh"' >/dev/null 2>&1
}

start_display_keeper() {
  /usr/bin/osascript -e 'tell application "Terminal" to do script "exec /opt/homebrew/bin/node '"$HOME"'/civvis-display-keeper.mjs"' >/dev/null 2>&1
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

host_pid=$(first_pid '[c]ivvis-interactive-host\.sh')
host_state=up
if [[ -z "$host_pid" ]]; then
  if start_host; then
    actions+=(host_reopened)
    sleep 5
    host_pid=$(first_pid '[c]ivvis-interactive-host\.sh')
  fi
  if [[ -z "$host_pid" ]]; then
    host_state=absent
    warnings+=(host_absent)
  else
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

mirror_keeper_pid=$(first_pid '[c]ivvis-mirror-keeper\.sh')
follower_pid=$(first_pid '[t]ools/follow\.py')
if (( live_events && tiles_exported )) && [[ -n "$host_pid" && -z "$mirror_keeper_pid" ]]; then
  # The interactive host restarts children every five seconds. Give it one
  # bounded chance before opening the exact missing helper.
  sleep 8
  mirror_keeper_pid=$(first_pid '[c]ivvis-mirror-keeper\.sh')
  if [[ -z "$mirror_keeper_pid" ]] && start_mirror_keeper; then
    actions+=(mirror_keeper_reopened)
    sleep 5
    mirror_keeper_pid=$(first_pid '[c]ivvis-mirror-keeper\.sh')
  fi
fi

collect_mirror
if (( live_events && tiles_exported )) \
    && { [[ -z "$mirror_keeper_pid" || -z "$follower_pid" ]] || (( ! mirror_healthy )); }; then
  sleep "$MIRROR_SETTLE_S"
  mirror_keeper_pid=$(first_pid '[c]ivvis-mirror-keeper\.sh')
  follower_pid=$(first_pid '[t]ools/follow\.py')
  collect_mirror
fi
if (( live_events && tiles_exported )); then
  [[ -n "$mirror_keeper_pid" ]] || warnings+=(mirror_keeper_absent)
  [[ -n "$follower_pid" ]] || warnings+=(follower_absent)
  (( mirror_healthy )) || warnings+=(mirror_not_ready)
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
