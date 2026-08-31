Archived 2026-08-31 during the "one merged copy" consolidation.

- civvis-host-tolerant.sh, civvis-supervisor-rotating.sh: GENERATED copies
  (by tools/ops/civvis-make-rotating-host.sh and the archived
  make-rotating-supervisor generator). The live flow now runs the tracked
  scripts in /Users/martin/CIVVIS/tools/ops via civvis-verification-launch.command.
- civvis-make-rotating-supervisor.sh: deliberately archived-not-tracked
  (commit f6748c04 / PR #2290): superseded by #1960's lane handling.
- civvis-item6-rerun.sh: July one-off for docs/EVAL_INTEGRITY.md item 6
  (completed; the doc carries the recomputed results).
- civvis-sync.sh.pre-shim-backup: the pre-shim sync script, now tracked as
  tools/civvis_sync.sh.
- civvis-verification-launch.command.bak-20260827: superseded backup of the
  operator launcher.

The 18 drifted ~/civvis-*.sh copies deleted the same day were each verified
byte-identical to a blob in CIVVIS git history (recoverable), unreferenced by
any LaunchAgent, crontab, tracked script, or the operator launcher.
