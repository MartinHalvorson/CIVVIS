#!/bin/zsh
# Carry merged ops fixes into the tree the live ops layer actually runs from --
# but only the files whose running process can safely pick them up.
#
# ★★★★★ THE GAME TREE IS REFRESHED EVERY CYCLE. THE OPS LAYER IS NOT.
#
# `civvis-verified-head-launcher.sh` re-checks-out `origin/main` in the
# verification runtime at every launch, so `civ6_play.py`, the brain and the mod
# are always current. The ops layer is launched once and persists from whatever
# tree launched it. Measured 2026-09-02 that tree was 127 commits behind;
# 2026-09-03, 180. Merged reliability fixes -- #3083, #3073, #2978, #3090 and
# the `wedge-handoff.json` marker -- were on `main` and not in effect, and
# handoffs were being filed as `killed` with resumes unspent.
#
# ⚠⚠⚠ AND YOU CANNOT SIMPLY REFRESH THE WHOLE TREE. Rewriting a zsh script
# while it runs is not merely ineffective; it is unsafe. Measured directly on
# this host 2026-09-03 -- a six-iteration loop, its file replaced two seconds
# in:
#
#     tick 0..5 ORIGINAL      <- the buffered loop finished intact
#     finished cleanly
#     zshread.sh:9: command not found: than
#     REPLACEMENT should never run   <- the NEW file's tail, executed
#
# zsh buffers the block it is inside, so the running loop keeps the old code --
# that much the wedge watchdog's own banner says. What the banner does not say
# is what happens NEXT: the process resumes reading the file at its byte offset
# and executes whatever the replacement happens to have there. For a supervisor
# whose `while true` never exits that is only a delayed hazard, but it is a
# hazard, and it is why this tool refuses to touch such a file at all.
#
# So eligibility is not "is it out of date", it is "can the process that is
# running it hand over cleanly". A script hands over when it stats its own
# source each poll and `exec`s itself on a change -- the wedge watchdog's
# "THIS PROCESS OUTLIVES ITS OWN SOURCE" pattern. That is a property of the
# RUNNING file, not of the candidate: a marker that exists only in the new
# version cannot reload anything, because the code that would do the reloading
# is the code already in memory.
#
# Everything else is REPORTED, never written. `civvis-supervisor-safe-reload.sh`
# lands the supervisor in a verified no-game gap; the rest need a restart.
set -u

# ⚠ THIS RUNS FROM launchd, WHOSE PATH IS NOT YOURS. Caught by this tool's own
# test, which runs it under a minimal environment: with an unhelpful PATH both
# `grep` and `date` vanish, `grep` fails, and EVERY file then reads as "no
# self-reload" -- so the service would report everything pending forever and
# refresh nothing, silently and with exit 0. Absolute paths are why the wedge
# watchdog says `/usr/bin/stat`; same reason, same rule.
GIT=${CIVVIS_GIT:-/usr/bin/git}
GREP=${CIVVIS_GREP:-/usr/bin/grep}
DATE=${CIVVIS_DATE:-/bin/date}
PGREP=${CIVVIS_PGREP:-/usr/bin/pgrep}

REPO=${CIVVIS_OPS_TREE:-/private/tmp/civvis-main-management}
REF=${CIVVIS_OPS_FRESHNESS_REF:-origin/main}
LOG=${CIVVIS_OPS_FRESHNESS_LOG:-$HOME/civvis-civ6-runs/ops_freshness.log}
# The line that proves a script re-reads its own source and hands over.
RELOAD_MARKER=${CIVVIS_OPS_RELOAD_MARKER:-'exec /bin/zsh "$SELF_PATH"'}

say() { print -r -- "[ops-freshness] $($DATE -u +%FT%TZ) $*" >> "$LOG" }

[[ -d "$REPO/.git" || -f "$REPO/.git" ]] || {
  say "no git tree at $REPO; nothing to do"; exit 0
}

behind=$($GIT -C "$REPO" rev-list --count HEAD.."$REF" 2>/dev/null || print -r -- "")
[[ -z "$behind" ]] && { say "cannot compare $REPO against $REF"; exit 0 }

# ★ NOTHING IS RUNNING IT => NOTHING CAN BE CORRUPTED.
#
# The hazard in the banner is entirely about a process reading a file it is
# already executing. A one-shot script -- `civvis-verified-head-launcher.sh`
# runs to completion at each launch, this tool runs to completion on a timer --
# has no such process between invocations, so writing it is exactly as safe as
# writing any other file, and holding it back leaves a merged fix stranded for
# no reason. The first version of this tool reported both as PENDING forever.
#
# ⚠ IT MUST NOT MATCH ITSELF. Run from a schedule, `pgrep -f` for this script's
# own name finds THIS process, so the tool would declare itself permanently
# ineligible. Its own pid and its parent are excluded.
running_elsewhere() {
  local name="$1" pid=""
  for pid in ${(f)"$($PGREP -f "$name" 2>/dev/null)"}; do
    [[ "$pid" == "$$" || "$pid" == "$PPID" ]] && continue
    return 0
  done
  return 1
}

refreshed=0
pending=0
for path in $($GIT -C "$REPO" diff --name-only "$REF" -- tools/ops 2>/dev/null); do
  file="$REPO/$path"
  # ⚠ The marker is read from the file ON DISK -- the one the running process
  # started from -- for the reason in the banner above.
  if ! running_elsewhere "${path:t}" \
      || { [[ -r "$file" ]] && $GREP -qF -- "$RELOAD_MARKER" "$file" }; then
    if $GIT -C "$REPO" checkout "$REF" -- "$path" 2>/dev/null; then
      if running_elsewhere "${path:t}"; then
        say "refreshed $path (its process re-execs on change)"
      else
        say "refreshed $path (nothing is running it)"
      fi
      refreshed=$(( refreshed + 1 ))
    else
      say "could not refresh $path"
    fi
  else
    say "PENDING $path: no self-reload in the running copy, so rewriting it would be unsafe; use civvis-supervisor-safe-reload.sh or restart it"
    pending=$(( pending + 1 ))
  fi
done

(( refreshed > 0 || pending > 0 )) \
  && say "$REPO was $behind behind $REF; refreshed $refreshed, left $pending pending"
exit 0
