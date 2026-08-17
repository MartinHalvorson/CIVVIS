#!/bin/zsh
# What Terminal opens when the ladder keeper starts the loop.
#
# The keeper cannot run `civvis-game-supervisor.sh` from launchd — installing
# the control mod writes inside Civ6.app and macOS grants that to Terminal, not
# to launchd (see tools/ops/ladder_watchdog.py). So Terminal hosts the loop.
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
SUPERVISOR=${CIVVIS_LADDER_SUPERVISOR:-${SELF_DIR}/civvis-game-supervisor.sh}
mkdir -p "${LOG:h}"

say() {
  print -r -- "[launcher] $(date -u +%FT%TZ) $*" >> "$LOG"
}

# ⚠ NOT `{ ... } | tee`. A pipeline hands the exit line to a `tee` that dies
# with the rest of the pipeline, and the last thing written is exactly the thing
# worth keeping — measured 2026-08-17T21:20Z, where the "starting" line landed
# and the status line did not. Append directly, and redirect the supervisor's
# own output with `>>` so nothing depends on a second process staying alive.
say "starting ${SUPERVISOR} (pid $$)"
/bin/zsh "$SUPERVISOR" >> "$LOG" 2>&1
# ⚠ NOT `status`: in zsh that name is read-only, an alias for $?, and
# assigning it aborts the script one line before the thing worth logging.
rc=$?
say "supervisor exited with status ${rc}"
exit $rc
