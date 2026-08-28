#!/bin/zsh
# civvis-run-prune.sh — daily retention for the Civ VI ladder's run directories.
#
# "Run games indefinitely" was also "fill the disk": one setup-screenshot-heavy
# run is 2 GB, and on 2026-08-26 the ladder host reached 5 GB free on a 2 TB
# volume with the recording lane dying underneath it (macOS purges recordings
# below ~15 GB free). Operator rule, 2026-08-27: "can clear old game saves that
# are older than 48 hrs". com.civvis.run-prune runs this at 03:17 every day.
#
# What it never touches: the ledgers beside the runs (civvis_ladder.jsonl,
# ladder.json — the ladder's memory), any run a process still has open, and the
# newest run whatever its age.
#
# ⚠ This removes WHOLE run directories, events.jsonl included. The harness's own
# retention (civ6_play.prune_old_run_screenshots, #2684) drops only PNGs after
# seven days and keeps every events.jsonl because live_repair_census and the
# ladder read them. A host that wants both the screenshots gone AND the event
# streams kept sets CIVVIS_RUN_PRUNE_MINUTES to a long window (or does not
# install this job at all): the two rules are independent and this one is the
# operator's 48-hour disk rule, not a curation policy.
#
#   civvis-run-prune.sh [--dry-run]
#   CIVVIS_RUN_PRUNE_MINUTES   age threshold, default 2880 (48 h)
#   CIVVIS_RUNS_ROOT           default ~/civvis-civ6-runs/control
#   CIVVIS_RUN_PRUNE_LOG       default ~/Library/Logs/civvis-run-prune.log
set -u
ROOT=${CIVVIS_RUNS_ROOT:-$HOME/civvis-civ6-runs/control}
LOG=${CIVVIS_RUN_PRUNE_LOG:-$HOME/Library/Logs/civvis-run-prune.log}
AGE_MIN=${CIVVIS_RUN_PRUNE_MINUTES:-2880}
DRY_RUN=0
[[ "${1:-}" == --dry-run ]] && DRY_RUN=1

[[ -d "$ROOT" ]] || exit 0
mkdir -p "${LOG:h}"
stamp() { date -u +%FT%TZ }

newest=$(ls -td "$ROOT"/civvis-*(N/) 2>/dev/null | head -n 1)
free_kb() { df -k "$ROOT" | tail -n 1 | awk '{print $4}' }
before=$(free_kb)
n=0
for d in $(find "$ROOT" -maxdepth 1 -type d -name 'civvis-*' -mmin +"$AGE_MIN"); do
  [[ "$d" == "$newest" ]] && continue
  if [[ -n "$(lsof +D "$d" -Fn 2>/dev/null | head -n 1)" ]]; then
    print -r -- "$(stamp) skip in-use $d" >> "$LOG"
    continue
  fi
  if (( DRY_RUN )); then
    print -r -- "would prune $d"
    (( n += 1 ))
    continue
  fi
  rm -rf -- "$d" && (( n += 1 ))
done
after=$(free_kb)
left=$(ls -d "$ROOT"/civvis-*(N/) 2>/dev/null | wc -l | tr -d ' ')
line="$(stamp) $([[ $DRY_RUN == 1 ]] && print -n 'DRY RUN: would prune' || print -n 'pruned') $n run dir(s) older than ${AGE_MIN} min; freed $(( (after - before) / 1048576 )) GB; free now $(( after / 1048576 )) GB; runs left $left"
if (( DRY_RUN )); then
  print -r -- "$line"
else
  print -r -- "$line" >> "$LOG"
fi
