#!/bin/zsh
# civvis-worktree-prune.sh — bounded retention for finished CIVVIS task trees.
#
# A finished agent worktree carries an untracked `target/` cache that often
# dwarfs its source.  The safe question is not whether its branch merged, but
# whether GitHub already has its content.  civvis_worktree_audit.py answers
# that after fetching pull refs and refuses dirty, active, local-only, main,
# detached deployment, and non-agent worktrees.  This wrapper adds the host
# policy: a clean, inactive task tree is eligible only after 24 hours.
#
# The launchd job runs hourly, so eligible build artifacts stay no more than
# roughly an hour beyond that window.  `--no-scan` is intentional: the regular
# 15-minute audit owns loose-file rescue; this job only reaps registered task
# worktrees through `git worktree remove`, never by deleting a guessed path.
#
#   CIVVIS_WORKTREE_PRUNE_MINUTES  inactivity threshold, default 1440 (24 h)
set -u

OPS=${0:A:h}
REPO=${OPS:h:h}
AUDIT=$REPO/tools/civvis_worktree_audit.py
AGE_MIN=${CIVVIS_WORKTREE_PRUNE_MINUTES:-1440}

[[ "$AGE_MIN" == <-> ]] && (( AGE_MIN > 0 )) || {
  print -u2 -r -- "CIVVIS_WORKTREE_PRUNE_MINUTES must be a positive integer: $AGE_MIN"
  exit 64
}
[[ -f "$AUDIT" ]] || {
  print -u2 -r -- "missing CIVVIS worktree audit: $AUDIT"
  exit 66
}

exec python3 "$AUDIT" --repo "$REPO" --reap --apply --idle-minutes "$AGE_MIN" --no-scan
