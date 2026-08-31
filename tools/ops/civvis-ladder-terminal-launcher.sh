#!/bin/zsh
# What Terminal opens when the ladder keeper starts the loop.
#
# The keeper cannot run `civvis-game-supervisor.sh` from launchd — installing
# the control mod writes inside Civ6.app and macOS grants that to Terminal, not
# to launchd (see tools/ops/ladder_watchdog.py). So Terminal hosts one managed
# interactive owner, which in turn owns the loop and its GUI helpers.
#
# ⚠⚠ THE POINT OF THIS FILE IS THAT A TERMINAL WINDOW IS NOT A LOG. Opened
# directly, the supervisor's shell output — every `set -u` failure, every
# unhandled error, the exit status itself — exists only in a GUI window that
# closes when the process ends, and is then unrecoverable. That is exactly what
# happened on 2026-08-17T21:13Z: the loop exited cleanly through its EXIT trap
# after one failed attempt and left no evidence anywhere on disk about why,
# because the only copy of its stderr had been painted into a window that was
# gone by the time anyone looked.
#
# So everything is teed to a file that outlives the window, and the exit status
# is recorded as a line rather than as the absence of one.

set -u
SELF_DIR=${0:A:h}
LOG=${CIVVIS_LADDER_LOG:-$HOME/Library/Logs/civvis-ladder.log}
# CIVVIS_LADDER_SUPERVISOR remains a compatibility override for an operator's
# direct launcher; normal recovery must use the host so an audit cannot create
# a second independent supervisor beside it.
HOST=${CIVVIS_LADDER_HOST:-${CIVVIS_LADDER_SUPERVISOR:-${SELF_DIR}/civvis-interactive-host.sh}}
mkdir -p "${LOG:h}"

say() {
  print -r -- "[launcher] $(date -u +%FT%TZ) $*" >> "$LOG"
}

# Terminal replaces a document shell with its normal post-command shell before
# an EXIT trap can inspect it. The old direct-TTY cleanup therefore logged
# "own tty not found" and left the completed document behind. Spawn a tiny
# Terminal-descended reaper before exit instead: after the shell is actually
# idle it can close only a window whose title still proves it was ours. An
# operator's normal Terminal remains busy and never matches this predicate.
WINDOW_REAPER_SCHEDULED=0
schedule_idle_window_reap() {
  (( WINDOW_REAPER_SCHEDULED )) && return 0
  WINDOW_REAPER_SCHEDULED=1
  [[ -z ${CIVVIS_LADDER_KEEP_WINDOW:-} ]] || return 0
  local own_tty=${TTY:-}
  [[ "$own_tty" == /dev/tty* ]] || return 0
  (
    /usr/bin/nohup /usr/bin/osascript >>"$LOG" 2>&1 <<'APPLESCRIPT'
delay 1
set reaped to 0
tell application "Terminal"
  repeat with i from (count of windows) to 1 by -1
    try
      set w to item i of windows
      if (busy of w) is false and (((name of w) contains "civvis-ladder-terminal-launcher") or ((name of w) contains "civvis-verified-head-launcher")) then
        close w
        set reaped to reaped + 1
      end if
    end try
  end repeat
end tell
return "window cleanup: reaped " & reaped & " idle managed window(s)"
APPLESCRIPT
  ) &
  say "window cleanup: scheduled idle managed-window reaper"
}
trap 'schedule_idle_window_reap || true' EXIT

# ⚠ NOT `{ ... } | tee`. A pipeline hands the exit line to a `tee` that dies
# with the rest of the pipeline, and the last thing written is exactly the thing
# worth keeping — measured 2026-08-17T21:20Z, where the "starting" line landed
# and the status line did not. Append directly, and redirect the supervisor's
# own output with `>>` so nothing depends on a second process staying alive.
# ★★★ THE WINDOW GETS OUT OF THE WAY, BECAUSE THE WINDOW IS NOT THE LOG.
#
# The header above is emphatic that everything worth keeping is teed to
# ${LOG}, which is precisely why nothing is lost by hiding this window — and
# something IS lost by showing it. `open -a Terminal` opens a NEW window every
# time the keeper recovers the loop, each one lands in front of whatever the
# operator is looking at, and the operator is looking at a Civilization VI game
# being recorded. Four dead launcher windows had stacked up over one afternoon
# of restarts on 2026-08-18 and had to be minimised by hand.
#
# Two lines fix both halves: this window minimises itself, and the dead ones
# from previous recoveries are closed. A dead launcher window holds no
# information — its shell has exited and its output is in ${LOG} — so closing it
# discards nothing. A BUSY one is a live lane and is never touched.
#
# Set CIVVIS_LADDER_KEEP_WINDOW=1 to watch it live instead; `tail -f ${LOG}`
# does the same job without a window in front of the game.
if [[ -z ${CIVVIS_LADDER_KEEP_WINDOW:-} ]]; then
  # ⚠ The result is LOGGED, not discarded. A silent `|| true` here would make
  # "the window still covers the game" and "the script never ran" look the same
  # from the log, which is the whole failure mode this file exists to prevent.
  window_report=$(/usr/bin/osascript - "$TTY" 2>&1 <<'APPLESCRIPT'
on run argv
  set myTty to item 1 of argv
  set mineSeen to 0
  set reaped to 0
  set liveSeen to 0
  tell application "Terminal"
    repeat with i from (count of windows) to 1 by -1
      try
        set w to item i of windows
        if (count of tabs of w) > 0 then
          set mine to false
          -- ⚠ `tty` ALONE IS NOT AN IDENTITY. macOS reassigns a tty device
          -- number as soon as its shell exits, so a dead launcher window keeps
          -- reporting the tty this live one was just given: matching on tty
          -- alone claimed three windows as "mine" and reaped none of them.
          -- A window whose shell has exited is not this shell; require both.
          repeat with t in tabs of w
            try
              if (tty of t) is myTty and (busy of w) is true then set mine to true
            end try
          end repeat
          if mine then
            set miniaturized of w to true
            set mineSeen to mineSeen + 1
          -- Terminal titles the document it opened, not necessarily the script
          -- that it `exec`s.  Normal recovery opens the operator wrapper, whose
          -- title is `civvis-verified-head-launcher.sh`; matching only this
          -- hand-off file left every completed recovery window behind.
          else if ((name of w) contains "civvis-ladder-terminal-launcher") or ((name of w) contains "civvis-verified-head-launcher") then
            if (busy of w) is false then
              close w
              set reaped to reaped + 1
            else
              set liveSeen to liveSeen + 1
            end if
          end if
        end if
      end try
    end repeat
  end tell
  return "minimised " & mineSeen & ", reaped " & reaped & ", live siblings " & liveSeen & ", tty " & myTty
end run
APPLESCRIPT
) || window_report="osascript failed"
  say "window: ${window_report}; follow the run with: tail -f ${LOG}"
fi

say "starting managed interactive host ${HOST} (pid $$)"
/bin/zsh "$HOST" >> "$LOG" 2>&1
# ⚠ NOT `status`: in zsh that name is read-only, an alias for $?, and
# assigning it aborts the script one line before the thing worth logging.
rc=$?
say "supervisor exited with status ${rc}"
exit $rc
