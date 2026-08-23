# Pre-registration: does strategic_deep's 4x still earn its keep?

Written before the run.

## Why
`strategic_deep` (review 20, horizon 80) was promoted at **+45 Elo** over
`strategic` (40/40) on a pre-registered 300-map run — measured on the DEFAULT
weights. A generation-14 champion genome has since been committed, and both
entrants now load it. #477 measures the same comparison on that genome as
"nearly neutral, 61-59 games and five map directions to four" — 0.2 SE over
120 games, underpowered.

The cost is not underpowered: `strategic_deep` spends 4x the macro-search
compute on every soak, league, fleet and exhibition game that opts in.

## The run
`ai_eval strategic_deep strategic --players 4 --pairs 100 --turns 500
--seed 4100000` — both entrants on the committed genome, paired and
seat-mirrored, fresh seed.

## Decision rule, fixed now
- **The promotion still holds** if map directions favour `strategic_deep` with
  sign p < 0.05.
- **The promotion no longer replicates** if the result is a null. That is not
  a call to revert on its own — a null at 100 pairs bounds the gain rather
  than zeroing it — but it would mean the 4x cost is unbacked on the shipped
  agent and the burden moves to whoever wants to keep paying it.
- Terminal score is reported beside it and is NOT part of the rule.

## ⚠ CONFOUND, caught by the provenance line before the run got going

  strategic_deep: plays as strategic_deep with untrained defaults (missing valuenet.json)
  strategic:      plays as strategic_score (missing valuenet.json)

No `valuenet.json` exists on this machine, so `strategic` **silently degrades
to `strategic_score`** while `strategic_deep` blends an untrained-default net.
The arms therefore differ in **two** ways: search budget (the intended
treatment) and value-net handling (not intended).

**This is not a clean isolation of the search budget, and must not be reported
as one.** What it does answer is the practical question — *as deployed, with
the artifacts that actually exist on this machine, is `strategic_deep` better
than `strategic`?* That is what soak, league, fleet and the exhibition really
run, so the number is worth having under that narrower claim.

A clean budget isolation needs both arms on identical net status, which
`elo::builtin_ai`'s fallback does not currently give from entrant names — it
is what my own `search_dose` does by constructing both `StrategicAi`s directly.

## Prior
I expect a null, on #477's reading. I also expect that to be the less
comfortable outcome, since it questions a promotion this repository earned
carefully rather than one it took casually — and the same discipline that
justified promoting it is what requires re-checking it.
