# There is no second galley

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

The Galley fix (#1989, promoted #1997 at +61 Elo-equivalent) was found by asking
a question the audit could not answer: not *how much of a Galley's time is
spent standing still* — the idle table said 54.3% and had said so for a while —
but **how many Galleys were built that never moved once**. The answer was 20 of
53, because a lakeside city counts as coastal and its hull can never leave the
lake.

Those are different questions. A kind can idle half its turns because it works
in bursts, or because half the ones built were never usable, and only the second
is a production defect. The first framing had been visible for weeks and nobody
acted on it; the second was actionable within an hour.

So: **is there another one?**

## How it was measured

`audit` gains a whole-life ledger beside its per-turn one. For every major unit
it records whether the unit ever changed tile and whether its `work_mark` ever
moved — the same mark the livelock tracker already uses, so "did this unit ever
accomplish anything" is one comparison rather than a second definition of work.
A unit must have lived 15+ turns before never acting counts, so a unit built on
the last turn is not evidence about the controller.

Three 150-turn six-player games at 74x46, 9 city-states, Online, seeds
21000000–02, at `main` with the open-water rule in production.

## What it measured

**Nothing like the Galley remains.**

| kind | never acted (15+ turns) | of all built | share |
|---|---:|---:|---:|
| builder | 1 | 371 | 0.3% |
| trader | 1 | 136 | 0.7% |
| settler | 1 | 88 | 1.1% |
| battering_ram | 1 | 20 | 5.0% |
| galley | 1 | 8 | 12.5% |
| siege_tower | 1 | 2 | 50.0% |

Six units across three games, every one a singleton. No kind is being built in
quantity and left unusable. `siege_tower` reads worst as a percentage and is one
unit of two.

The Galley row is the confirmation: **8 built, 1 never-actor**, against 40 built
and 20 never-actors on these same seeds before the fix. The promotion did what
the mechanism said it would at the population level, not just on the sampled
metric that motivated it.

## What was decided

**The census ships; no fix follows it, because it says none is available.**
That is the useful outcome. "Units built that can never act" was the class the
single largest strength win of this session came from, and it is now closed —
so the next lead is not another instance of it, and time spent looking for one
is time wasted.

⚠ What this does **not** say is that the idle table is exhausted. `builder`
(19.8% of major idle), `archer` (13.7%) and `scout` (12.9%) are all still
unexplained, and all three have now had at least one fix screened to a null.
What the census rules out is that any of them is idle because it was built into
a situation it could never act in. Whatever is wrong with them, it is not that.
