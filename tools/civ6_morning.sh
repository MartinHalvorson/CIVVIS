#!/bin/sh
# One command for "what happened overnight". Everything below is generated from the
# run data, not from anything written by hand.
#
# Usage: tools/civ6_morning.sh [SINCE]
#   SINCE is an ISO-8601 UTC instant; it defaults to 04:00Z today.
#
# The two things that had to change to make this runnable by anyone: it used to
# `cd` to one agent's scratchpad worktree, which is deleted the moment that task
# closes out, and it pinned `--after` to a literal date, so every run after
# 2026-07-31 silently widened the ledger window instead of reporting one night.
W=$(cd "$(dirname "$0")/.." && pwd)
cd "$W" || exit 1
# The most recent 04:00Z that has actually HAPPENED. Naively using today's
# 04:00Z reports an empty night whenever this is run between midnight and 04:00Z
# -- the window starts in the future and filters out every row, which reads
# exactly like "nothing ran overnight". (`date -v` is BSD/macOS, as is the rest
# of this rig.)
if [ "$(date -u +%H)" -lt 04 ]; then
	SINCE=${1:-$(date -u -v-1d +%Y-%m-%dT04:00:00Z)}
else
	SINCE=${1:-$(date -u +%Y-%m-%dT04:00:00Z)}
fi
echo "================ LEDGER (compare rows WITHIN a code_rev, never across) ========"
python3 tools/civ6_night_report.py --after "$SINCE"
echo
echo "================ LIVE RUN ====================================================="
python3 tools/civ6_civvis_status.py
echo
echo "================ DETECTORS ON THE LIVE RUN ===================================="
python3 tools/civ6_watchdogs.py
echo
echo "================ SETTLERS, WHICH IS WHERE THE CITY COUNT COMES FROM ==========="
python3 tools/civ6_settler_trace.py
echo
echo "================ IS THE LOOP STILL UP? ========================================"
pgrep -f civ6_civvis_climb >/dev/null && echo "climb UP" || echo "climb DOWN"
pgrep -f civ6_watchdog_daemon >/dev/null && echo "watchdog daemon UP" || echo "watchdog daemon DOWN"
pgrep -f Civ6_Exe >/dev/null && echo "Civilization VI UP" || echo "Civilization VI DOWN"
echo
echo "Notes: newest under ~/civvis-civ6-runs/climb-logs/NOTES-*.md"
