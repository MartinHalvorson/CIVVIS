#!/bin/zsh
# Start the CIVVIS verification loop WITHOUT opening a Terminal window.
#
# ⚠ Why this exists: `open -g -j -a Terminal <script>` is the documented way to
# get a Terminal-descended tree (tools/ops/ladder_watchdog.py explains why that
# ancestry is mandatory -- installing the control mod writes inside Civ6.app and
# macOS attributes that grant to the RESPONSIBLE process, which is Terminal).
# But every such call creates a new Terminal window, it lands in front when
# Terminal is already frontmost, and it covers the game being recorded. Worse,
# popup_clear.py refuses to click while anything but Civ 6 is frontmost
# ("leader on screen but 'Terminal' is frontmost; not clicking"), so a Terminal
# window stealing focus actively stops popups being cleared.
#
# The insight: a shell that is ALREADY a descendant of Terminal.app inherits the
# same grants, so it can start the tree directly with nohup and no window is
# created at all. Verified 2026-08-18 from this context:
#   App Management  -> could touch a file inside Civ6.app/Contents/Assets/DLC
#   Accessibility   -> System Events returned the real Civ6 window position
#   Screen Recording-> screencapture returned 236 distinct luma values
#
# ⚠⚠ RUN THIS ONLY FROM A TERMINAL-DESCENDED SHELL. From launchd it will come up
# looking healthy and every attempt will die at "NO GAME" with a modal-free menu,
# because a dead synthetic click is silent. The guard below refuses instead.
set -u

WRAPPER=${1:-$HOME/civvis-verification-launch.command}
LOG=${CIVVIS_LADDER_LOG:-$HOME/Library/Logs/civvis-ladder.log}

# Walk up the process tree; Terminal.app must be an ancestor.
terminal_descended() {
  local pid=$$ ppid cmd
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    ppid=$(ps -o ppid= -p "$pid" 2>/dev/null | tr -d ' ')
    [[ -z "$ppid" || "$ppid" == "0" ]] && return 1
    cmd=$(ps -o comm= -p "$ppid" 2>/dev/null)
    [[ "$cmd" == *"Terminal.app"* ]] && return 0
    [[ "$ppid" == "1" ]] && return 1
    pid=$ppid
  done
  return 1
}

if ! terminal_descended; then
  print -r -- "REFUSING: not Terminal-descended. Starting the loop from here would" >&2
  print -r -- "  come up looking healthy and then fail every attempt at NO GAME." >&2
  print -r -- "  Run this from a Terminal window (or a Claude session hosted in one)." >&2
  exit 78
fi

if [[ ! -f "$WRAPPER" ]]; then
  print -r -- "REFUSING: no wrapper at $WRAPPER" >&2
  exit 66
fi

mkdir -p "${LOG:h}"
print -r -- "[relaunch] $(date -u +%FT%TZ) starting windowless (parent shell $$)" >> "$LOG"
nohup /bin/zsh "$WRAPPER" >> "$LOG" 2>&1 &
disown
print -r -- "started windowless; pid $!  (log: $LOG)"
