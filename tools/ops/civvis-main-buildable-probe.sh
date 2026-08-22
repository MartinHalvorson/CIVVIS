#!/bin/zsh
# Cheap static check: can origin/main's LIVE_TREATMENTS still break the build?
#
# History: main stopped compiling 2026-08-19T02:05Z when two merges each appended
# a treatment row against the same `[LiveTreatment; 80]` declaration (each green
# alone; non-strict branch protection meant the combination was never CI'd). The
# ladder was pinned off head to keep playing, and this probe answered "can we
# unpin yet?" without paying for a 3-minute cargo build.
#
# ✅ #2106 ("The treatment list counts itself") replaced the fixed-size array with
# a SELF-COUNTING SLICE `&[LiveTreatment]`, so appending a row can no longer
# desynchronise a length. When the declaration is a slice this class of breakage
# is gone and the probe reports buildable.
#
# ⚠ An earlier version only understood the array form and reported `declared=?
# entries=0 / STILL MISMATCHED` once the slice landed -- a FALSE alarm that would
# have re-pinned the ladder onto a stale revision for no reason. A probe that
# cannot tell "shape changed" from "still broken" is worse than none.
set -u
cd "${CIVVIS_REPO:-$HOME/CIVVIS}" || exit 2
git -c gc.auto=0 fetch --quiet origin main 2>/dev/null || { print -r -- "FETCH FAILED"; exit 2 }
SRC=$(git show origin/main:src/ai/advanced/treatments.rs 2>/dev/null) || { print -r -- "PATH MOVED -- re-read the probe"; exit 2 }
SHA=$(git rev-parse --short origin/main)

if print -r -- "$SRC" | grep -qE "LIVE_TREATMENTS: &\[LiveTreatment\]"; then
  print -r -- "origin/main $SHA: self-counting slice (#2106) -- length cannot desync"
  print -r -- "LOOKS BUILDABLE"
  exit 0
fi

DECL=$(print -r -- "$SRC" | grep -oE "LIVE_TREATMENTS: \[LiveTreatment; [0-9]+\]" | grep -oE "[0-9]+")
if [[ -z "$DECL" ]]; then
  print -r -- "origin/main $SHA: LIVE_TREATMENTS declaration not recognised -- probe is stale, CHECK BY HAND"
  exit 2
fi
# Entries are 3-tuples, one per line: `    ("name", "slug", AdvancedAi::fn),`
N=$(print -r -- "$SRC" | awk '/LIVE_TREATMENTS: \[LiveTreatment;/{inside=1; next} inside && /^\];/{inside=0} inside && /^    \("/{n++} END{print n+0}')
print -r -- "origin/main $SHA: declared=${DECL} entries=${N}"
[[ "$DECL" == "$N" ]] && { print -r -- "LOOKS BUILDABLE"; exit 0 }
print -r -- "STILL MISMATCHED"; exit 1
