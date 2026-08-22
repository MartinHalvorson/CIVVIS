#!/bin/zsh
# Regenerate ~/civvis-host-tolerant.sh from the TRACKED interactive host.
#
# ⚠⚠⚠ WHY: the tracked host exits at EVERY game boundary. When a game ends,
# civ6_play exits while the climb still briefly holds the gamelock; a held lock
# with no harness behind it is a STANDING hold, and `gamelock.py --hold-status`
# exits 0 for a standing hold and an explicit operator halt ALIKE. The host
# treats any exit 0 as "operator halt active", stops its children and quits.
# Measured twice on 2026-08-18/19 (22:56:46Z, 00:33:13Z) with NO halt marker on
# disk. `com.civvis.ladder-watchdog` then relaunches a STOCK tree through
# Terminal, which both opens a window and discards the operator's settings.
#
# The fix keeps the halt contract intact -- an EXPLICIT halt still stops the
# host, and an inspection failure still fails safe -- while a standing hold is
# logged and ignored. gamelock's own standing_hold() docstring exists to let a
# caller tell "halted" from "wedged", "which are the same symptom and opposite
# remedies"; this makes the host act on that distinction.
set -u
SRC=${1:-/Users/martin/CIVVIS/tools/ops/civvis-interactive-host.sh}
DST=${2:-$HOME/civvis-host-tolerant.sh}

python3 - "$SRC" "$DST" <<'PY'
import sys, pathlib
src, dst = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
text = src.read_text()
old = '''    0) print -r -- "$output"; return 0 ;;'''
new = '''    0)
       # --- generated: a STANDING hold is not an OPERATOR halt ---------------
       # `--hold-status` exits 0 for both. Only an explicit halt (the marker
       # written by `gamelock.py --halt`) may stop this host; a standing hold is
       # the ordinary lock-release race at a game boundary and must not end the
       # ladder. Inspection failures below still fail safe as holds.
       if [[ "$output" == *"explicitly halted"* ]]; then
         print -r -- "$output"
         return 0
       fi
       say "ignoring a STANDING hold (not an operator halt): $output"
       return 1
       ;;
       # --- end generated ----------------------------------------------------'''
if old not in text:
    sys.exit("REFUSING: the hold_status exit-0 branch moved; regenerate by hand")
text = text.replace(old, new, 1)
dst.write_text(text)
PY
chmod +x "$DST"
print -r -- "generated $DST from $SRC"
