#!/bin/zsh
# civvis-foreground-guard.sh — keep macOS's background-items alert, and the
# Settings pane it points at, off the game.
#
# Every time a NEW LaunchAgent label is registered on a Mac, Background Task
# Management posts a persistent Notification Center alert — on macOS 26 titled
# "App Background Activity": “'zsh' can run in the background. You can manage
# background activity in Login Items & Extensions.” It sits in the top-right
# corner over whatever is there, which on a verification host is the Civ VI
# game being played and recorded, and a click on it opens System Settings on
# the Login Items & Extensions pane, in front of everything. Measured
# 2026-08-28: `launchctl bootstrap` of a throwaway agent → alert ~10 s later;
# Settings did not open by itself. Operator, the same day: "make sure login
# extensions windows does not appear in foreground covering our work here".
#
# The alert cannot be pre-approved without MDM, and this host registers new
# labels from time to time (each new clone that runs `civvis_collab.py
# bootstrap` installs its own com.civvis.freshness.<hash> agent). So this guard
# dismisses it instead, and closes a System Settings window on that pane:
#   * ONLY alerts whose text names Login Items & Extensions / App Background
#     Activity / Background Items Added — every other notification is left
#     alone (the recording guard's warnings are wanted);
#   * ONLY Settings windows on the Login Items pane, and only while a game lane
#     is up (a host or a player process exists) — an operator on that pane
#     between games is not interrupted.
#
# ⚠ It must be Terminal-descended. Driving Notification Center and System
# Settings is Apple Events to System Events, and macOS grants Automation to the
# RESPONSIBLE process — Terminal — while a launchd job inherits nothing (see
# ladder_watchdog.py). That is why civvis-verified-head-launcher.sh starts this
# from the Terminal-opened chain rather than a plist doing so. Run it by hand
# from a Terminal window and it works; from launchd it silently does nothing.
#
#   civvis-foreground-guard.sh            loop every 10 s while a game lane is up;
#                                         one instance (lock dir), exits when the
#                                         lane has been gone for the grace period
#   civvis-foreground-guard.sh --once     one pass, print what it did, exit
#   touch ~/.civvis-foreground-guard-off  stand the guard down (checked every pass)
set -u
unsetopt BG_NICE

OPS=${0:A:h}
LOG=${CIVVIS_FOREGROUND_GUARD_LOG:-$HOME/Library/Logs/civvis-foreground-guard.log}
LOCK=${CIVVIS_FOREGROUND_GUARD_LOCK:-$HOME/.civvis-foreground-guard.lock}
OFF=${CIVVIS_FOREGROUND_GUARD_OFF:-$HOME/.civvis-foreground-guard-off}
INTERVAL=${CIVVIS_FOREGROUND_GUARD_INTERVAL:-10}
GRACE=${CIVVIS_FOREGROUND_GUARD_GRACE:-180}
# ⚠ Every pass is bounded. System Events answers Apple Events one at a time,
# and on 2026-08-28 twenty-eight stray guards (started by a test suite that
# never set CIVVIS_FOREGROUND_GUARD=0) each parked an osascript on it until
# every pass on the host — this guard's included — hung for minutes. A pass
# that has not answered in this many seconds is killed and logged, and the
# next one starts on schedule.
PASS_TIMEOUT=${CIVVIS_FOREGROUND_GUARD_PASS_TIMEOUT:-20}
# Tests stub these; a host never needs to.
OSASCRIPT=${CIVVIS_FOREGROUND_GUARD_OSASCRIPT:-/usr/bin/osascript}
LANE_OVERRIDE=${CIVVIS_FOREGROUND_GUARD_LANE:-}
ONCE=0
[[ "${1:-}" == --once ]] && ONCE=1
mkdir -p "${LOG:h}"

say() { print -r -- "[foreground-guard] $(date -u +%FT%TZ) $*" >> "$LOG" }

# A game lane is up while the Terminal-hosted chain or a player exists. Every
# pattern is bracket-escaped so this process never matches itself, and never
# shows up in civ6_civvis_climb.busy()'s own grep.
lane_active() {
  if [[ -n "$LANE_OVERRIDE" ]]; then
    [[ "$LANE_OVERRIDE" == 1 ]]
    return
  fi
  pgrep -f 'civvis-interactive-hos[t]\.sh' >/dev/null 2>&1 && return 0
  pgrep -f 'civ6_pla[y]\.py' >/dev/null 2>&1 && return 0
  pgrep -f 'MacOS/Civ6_Ex[e]' >/dev/null 2>&1 && return 0
  return 1
}

# One pass. Prints `alerts=<seen> closed=<dismissed> settings=<closed>`, or
# `timeout` when osascript did not answer within PASS_TIMEOUT.
# The argument says whether a Settings window on the pane may be closed.
pass() {
  local settings_mode=$1 out waited=0 child
  out=$(mktemp "${TMPDIR:-/tmp}/civvis-foreground-guard.XXXXXX") || return 1
  applescript "$settings_mode" > "$out" 2>&1 &
  child=$!
  while kill -0 "$child" 2>/dev/null; do
    if (( waited >= PASS_TIMEOUT )); then
      # osascript is the grandchild; kill the whole pass by process group is
      # not available here, so name both.
      pkill -TERM -P "$child" 2>/dev/null
      kill -TERM "$child" 2>/dev/null
      sleep 1
      pkill -KILL -P "$child" 2>/dev/null
      kill -KILL "$child" 2>/dev/null
      rm -f -- "$out"
      print -r -- "timeout"
      return 0
    fi
    sleep 1
    (( waited += 1 ))
  done
  wait "$child" 2>/dev/null
  cat -- "$out"
  rm -f -- "$out"
}

applescript() {
  local settings_mode=$1
  exec "$OSASCRIPT" - "$settings_mode" <<'APPLESCRIPT' 2>&1
on run argv
  set settingsMode to item 1 of argv
  set seen to 0
  set closed to 0
  set settingsClosed to 0
  tell application "System Events"
    if exists process "NotificationCenter" then
      repeat with w in windows of process "NotificationCenter"
        set alerts to {}
        -- The path measured on macOS 26; older layouts nest one level less.
        try
          set alerts to UI elements of scroll area 1 of group 1 of group 1 of w
        on error
          try
            set alerts to UI elements of scroll area 1 of group 1 of w
          on error
            set alerts to {}
          end try
        end try
        repeat with a in alerts
          set txt to ""
          try
            repeat with t in static texts of a
              set txt to txt & (value of t) & " "
            end repeat
          end try
          if txt is not "" then set seen to seen + 1
          if txt contains "Login Items & Extensions" or txt contains "Background Items Added" or txt contains "App Background Activity" then
            try
              repeat with act in actions of a
                if description of act is "Close" then
                  perform act
                  set closed to closed + 1
                end if
              end repeat
            end try
          end if
        end repeat
      end repeat
    end if
    if settingsMode is "close" and (exists process "System Settings") then
      repeat with w in windows of process "System Settings"
        set n to ""
        try
          set n to name of w
        end try
        if n contains "Login Items" then
          try
            perform action "AXPress" of (first button of w whose subrole is "AXCloseButton")
            set settingsClosed to settingsClosed + 1
          end try
        end if
      end repeat
    end if
  end tell
  return "alerts=" & seen & " closed=" & closed & " settings=" & settingsClosed
end run
APPLESCRIPT
}

if (( ONCE )); then
  result=$(pass close)
  say "once: $result"
  print -r -- "$result"
  exit 0
fi

# One instance. A stale lock from a crashed guard is taken over.
if ! mkdir "$LOCK" 2>/dev/null; then
  holder=$(cat "$LOCK/pid" 2>/dev/null || true)
  if [[ "$holder" == <-> ]] && kill -0 "$holder" 2>/dev/null; then
    say "already running as pid $holder; exiting"
    exit 0
  fi
  rm -rf -- "$LOCK"
  mkdir "$LOCK" || exit 1
fi
print -r -- $$ > "$LOCK/pid"
trap 'rm -rf -- "$LOCK"; exit 0' HUP INT TERM EXIT

say "up (pid $$, every ${INTERVAL}s, grace ${GRACE}s, off marker $OFF)"
gone_for=0
while true; do
  if [[ -e "$OFF" ]]; then
    say "off marker $OFF present; standing down"
    break
  fi
  # A guard whose lock is gone has lost its home (a test's temporary HOME,
  # a reaped directory) or been taken over; it has nothing to guard for.
  if [[ ! -d "$LOCK" || "$(cat "$LOCK/pid" 2>/dev/null || true)" != "$$" ]]; then
    say "lost the lock $LOCK; exiting"
    trap - HUP INT TERM EXIT
    break
  fi
  if lane_active; then
    gone_for=0
    result=$(pass close)
  else
    (( gone_for += INTERVAL ))
    if (( gone_for >= GRACE )); then
      say "no game lane for ${gone_for}s; exiting"
      break
    fi
    # Between games the alert still covers the recording; the pane does not
    # get closed under an operator who opened it.
    result=$(pass keep)
  fi
  if [[ "$result" == timeout ]]; then
    say "pass timed out after ${PASS_TIMEOUT}s (System Events busy?); next pass in ${INTERVAL}s"
  elif [[ "$result" != "alerts="*" closed=0 settings=0" ]]; then
    say "$result"
  fi
  sleep "$INTERVAL"
done
