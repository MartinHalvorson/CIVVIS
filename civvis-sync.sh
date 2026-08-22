#!/bin/zsh
# Shim. The real script is tracked in the repo at tools/civvis_sync.sh.
#
# ⚠⚠ THIS FILE USED TO BE THE WHOLE ~200-LINE SCRIPT, AND IT EXISTED NOWHERE ELSE.
# Its own `git hash-object | git cat-file -e` test — the test it runs against
# other people's files — said this disk was the only copy of it. The guard
# against stranded code was itself stranded, and it also had the hole that let
# 146 unstaged lines of `civvis_orders --without` sit undetected for a day.
#
# Both are fixed by tracking it: `com.civvis.sync` (launchd, every 900s) still
# runs THIS path, and this path now runs whatever is on origin/main, so the
# scheduled sweep updates itself with the repo.
#
# Do not re-add logic here. Anything this needs to do belongs in
# tools/civvis_sync.sh or tools/civvis_worktree_audit.py, where CI can see it.

set -u
REAL=/Users/martin/CIVVIS/tools/civvis_sync.sh

if [[ ! -x $REAL ]]; then
  print -r -- "[sync] $(date -u +%FT%TZ) MISSING $REAL -- the tracked sync script is gone; not falling back to a stale local copy" \
    | tee -a /Users/martin/civvis-sync.log
  exit 1
fi
exec $REAL "$@"
