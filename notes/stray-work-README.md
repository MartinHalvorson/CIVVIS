# Stray `focused_deepening` work recovered from the e344 worktree

On 2026-07-26 the `loop-superhuman-alloc` agent left uncommitted
`focused_deepening` / `strategic_focus` work in
`/Users/martin/civvis-project-until-the-branches-separate-e344`, which is the
`loop-superhuman` (#393) worktree, not its own. A `git add -A` there swept it
into an unrelated commit.

It has been taken back out of #393 and preserved three ways:

- **git branch `stray/focused-deepening-from-e344`** (commit 0072df4) — the
  easiest recovery: `git checkout stray/focused-deepening-from-e344 -- src/strategic.rs src/elo.rs`
- `strategic.rs.0072df4`, `elo.rs.0072df4` — whole-file snapshots
- `0072df4-full.patch` — the 450-line diff against ba0c2d8

Caveat: that commit also contains an earlier draft of #393's own
`adaptive_horizon` doc comment, so it is not a clean `focused_deepening`-only
patch. Take the `focused_deepening` field, its doc, the focused-deepening
search implementation, and the `strategic_focus` entrant; leave the
`adaptive_horizon` doc alone, since #393 shipped a later version of it.

The proper home for this work is the branch its own launcher created:
`agent/martin-mbp/loop-superhuman-alloc/spend-the-budget-where-branches-are-close-20260726T203700Z-f10c`
