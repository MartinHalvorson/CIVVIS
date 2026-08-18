# Evaluation rounds

One round, one file. A round finished from 2026-08-18 onward belongs here as
`YYYY-MM-DD-what-it-was-about.md`, and `tools/eval_round.py "<title>"` writes
the file with the right name and the right headings.

## Why this is not appended to `docs/EVAL.md`

Because that file has exactly one write point and everybody queues at it.
`EVAL.md` is 10,386 lines, and in the seven days to 2026-08-18 thirty-two
commits touched it — every single one editing the last few lines, at 10242,
10335, 10362. Two agents finishing two evaluations on the same afternoon
conflicted over a document neither had read the same part of.

That is the shape `src/main.rs` had before its embedded changelog moved out,
when it was serializing roughly half of all merges. One file per round has no
shared tail, so the conflict is not resolved, it is absent.

## What stayed where it was

All 168 historical rounds remain in `docs/EVAL.md`. It is cited by
`src/elo.rs`, `docs/AI_GUIDE.md` and `tools/eval_manifest.py` among others, and
rewriting it into 168 files would break those citations to solve a problem that
only ever affected NEW rounds. The archive is not the thing that collides.

`docs/EVAL.md` also still holds the measurement doctrine every round here has
to satisfy — gate on the deployment shape, one seed is never a result, a
composite gate licenses the composite and never its parts, and actuation
repairs pay where valuation tunes do not. Read it before writing a round; it is
the standard, not just the history.

## What a round has to say

The template asks four questions and each exists because skipping it has cost
this project something real:

- **What was asked.** An evaluation that cannot state its question measured
  something, but nobody can say what.
- **How it was measured.** Arms, games, seeds, and the shape they ran at.
- **What it measured.** Numbers *with intervals*. A point estimate with no
  interval cannot be compared against the next one, which is how a 40-game
  figure came to be read against a 480-game figure on 2026-08-17 and reported
  as a 21.7-point regression that did not exist.
- **What was decided.** Shipped, withheld, or unresolved, and why. A null
  result is a result and belongs here in the same detail as a win.
