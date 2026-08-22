# Superseded: the rotating supervisor pair, plus the generated tolerant host

Archived 2026-08-22 from `~` on `mbp-m5-max-128`. All three failed
`git cat-file -e $(git hash-object FILE)` — this disk was the only copy.

- `civvis-make-rotating-supervisor.sh` (102 lines) — generator. Made the game
  supervisor choose a victory lane per game from `$HOME/.civvis-victory-lanes`,
  for an operator request on 2026-08-18. **Superseded by #1960**, which routes
  the lane through `civ6_play.DEFAULT_CIVVIS_VICTORY`;
  `tools/test_ops_ladder_objective.py::NoOperationalScriptHoldsALaneOfItsOwn`
  now forbids an ops script owning lane selection. It is also already dead
  against current main: its own guard exits `REFUSING: the --victory call site
  moved` because #2210, #2211 and #2280 moved the tracked supervisor.
- `civvis-supervisor-rotating.sh` (461 lines) — its generated output.
- `civvis-host-tolerant.sh` (211 lines) — generated output of
  `tools/ops/civvis-make-rotating-host.sh`, which IS tracked (PR #2290).
  Kept here only because the copy on disk was built from the 2026-08-18 host;
  regenerate from the tracked generator rather than reusing this.

Retrieve with:

    git fetch origin refs/civvis/wip/superseded-rotating-supervisor-20260822
    git checkout FETCH_HEAD
