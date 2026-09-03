#!/bin/zsh
# Keep specifically identified CIVVIS helper documents from covering a live
# Civilization VI game.
#
# The persistent ladder goes through `open -g -j -a Terminal` and labels its
# own document.  Some short-lived recovery helpers still need Terminal's App
# Management/Accessibility responsibility, though, and older/manual callers
# used `tell application "Terminal" to do script ...`.  Terminal brings that
# document to the front.  This guard is a safety net for those exact helper
# names: it labels and miniaturizes the document, then returns focus to Civ VI
# only if that document had been frontmost.  It does not inspect or move normal
# Terminal shells, generic Python commands, or arbitrary CIVVIS development
# work.
#
# It runs from the verified Terminal-descended chain.  That ancestry is needed
# to automate Terminal and System Events on this host; a LaunchAgent cannot
# inherit the required Automation grant.  One instance lives while a game lane
# is present and exits after the grace period:
#
#   civvis-terminal-window-guard.sh          loop (one instance)
#   civvis-terminal-window-guard.sh --once   one pass and print its result
#   touch ~/.civvis-terminal-window-guard-off stand it down
set -u
unsetopt BG_NICE

LOG=${CIVVIS_TERMINAL_WINDOW_GUARD_LOG:-$HOME/Library/Logs/civvis-terminal-window-guard.log}
LOCK=${CIVVIS_TERMINAL_WINDOW_GUARD_LOCK:-$HOME/.civvis-terminal-window-guard.lock}
OFF=${CIVVIS_TERMINAL_WINDOW_GUARD_OFF:-$HOME/.civvis-terminal-window-guard-off}
INTERVAL=${CIVVIS_TERMINAL_WINDOW_GUARD_INTERVAL:-1}
GRACE=${CIVVIS_TERMINAL_WINDOW_GUARD_GRACE:-180}
PASS_TIMEOUT=${CIVVIS_TERMINAL_WINDOW_GUARD_PASS_TIMEOUT:-5}
OSASCRIPT=${CIVVIS_TERMINAL_WINDOW_GUARD_OSASCRIPT:-/usr/bin/osascript}
LANE_OVERRIDE=${CIVVIS_TERMINAL_WINDOW_GUARD_LANE:-}
MARKER='CIVVIS managed helper'
ONCE=0
[[ "${1:-}" == --once ]] && ONCE=1
mkdir -p "${LOG:h}"

say() { print -r -- "[terminal-window-guard] $(date -u +%FT%TZ) $*" >> "$LOG" }

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

# A pass is deliberately bounded: a busy Terminal Apple Event must not leave a
# growing procession of stuck helpers behind it.
pass() {
  local out waited=0 child
  out=$(mktemp "${TMPDIR:-/tmp}/civvis-terminal-window-guard.XXXXXX") || return 1
  applescript > "$out" 2>&1 &
  child=$!
  while kill -0 "$child" 2>/dev/null; do
    if (( waited >= PASS_TIMEOUT )); then
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
  exec "$OSASCRIPT" - "$MARKER" <<'APPLESCRIPT' 2>&1
on run argv
  set windowMarker to item 1 of argv
  set hiddenCount to 0
  set reaped to 0
  set focused to 0
  set restoreCiv6 to false
  if application "Terminal" is not running then
    return "hidden=0 reaped=0 focused=0"
  end if
  tell application "Terminal"
    repeat with w in windows
      try
        set wasFrontmost to frontmost of w
        set tabCount to count of tabs of w
        repeat with t in tabs of w
          try
            set marked to ((custom title of t) is windowMarker)
            if not marked then
              -- Terminal exposes a document title on its window, not its tab.
              -- Limit that window-level title to the selected tab so another
              -- tab in an ordinary multi-tab window can never be swept in.
              set n to name of w
              -- These are the narrow, one-shot helpers that outside recovery
              -- callers have historically started with Terminal `do script`.
              if (selected of t) is true and (n contains "civvis-rehost-bootstrap.py" or n contains "civvis-capture-free-setup.py" or n contains "civvis-attach-cont2-" or n contains "civvis-resume-cont2-") then
                set custom title of t to windowMarker
                set title displays custom title of t to true
                set marked to true
              end if
            end if
            -- A window-level miniaturize would otherwise hide unrelated tabs.
            -- These automation callers create standalone documents, so refuse
            -- a multi-tab window rather than disturbing an operator's shell.
            if marked then
              if tabCount is 1 then
                if busy of t then
                  if (miniaturized of w) is false then
                    set miniaturized of w to true
                    set hiddenCount to hiddenCount + 1
                    if wasFrontmost then set restoreCiv6 to true
                  end if
                else
                  -- Terminal exposes a reliable close operation for a window,
                  -- not an individual tab. This is our one marked document.
                  close w
                  set reaped to reaped + 1
                end if
              end if
            end if
          end try
        end repeat
      end try
    end repeat
  end tell
  if restoreCiv6 then
    try
      tell application "System Events"
        if exists process "Civ6_Exe_Child" then
          set frontmost of process "Civ6_Exe_Child" to true
          set focused to 1
        end if
      end tell
    end try
  end if
  return "hidden=" & hiddenCount & " reaped=" & reaped & " focused=" & focused
end run
APPLESCRIPT
}

if (( ONCE )); then
  result=$(pass)
  say "once: $result"
  print -r -- "$result"
  exit 0
fi

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
  if [[ ! -d "$LOCK" || "$(cat "$LOCK/pid" 2>/dev/null || true)" != "$$" ]]; then
    say "lost the lock $LOCK; exiting"
    trap - HUP INT TERM EXIT
    break
  fi
  if lane_active; then
    gone_for=0
    result=$(pass)
  else
    (( gone_for += INTERVAL ))
    if (( gone_for >= GRACE )); then
      say "no game lane for ${gone_for}s; exiting"
      break
    fi
    result=$(pass)
  fi
  if [[ "$result" == timeout ]]; then
    say "pass timed out after ${PASS_TIMEOUT}s; next pass in ${INTERVAL}s"
  elif [[ "$result" != 'hidden=0 reaped=0 focused=0' ]]; then
    say "$result"
  fi
  sleep "$INTERVAL"
done
