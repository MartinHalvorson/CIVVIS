# An idle scout has somewhere to go, and two obvious reasons are not why

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

With the Galley removed by #1997 and the Builder and fortify leads both
screened to nulls, `scout` is the largest block in `audit`'s per-unit table that
still has no explanation: **751 idle unit-turns per three games, 15.5% of a
scout's own turns.**

This round did not fix it. It records what it ruled out, because two plausible
explanations are now dead with evidence and the next person should not spend
their turn on either.

## How it was measured

A probe linking the crate, following every major scout over three 150-turn
six-player games at 74x46, 9 city-states, Online, seeds 21000000–02, and asking
for each stationary scout-turn whether unexplored land was reachable — twice,
once ignoring borders and once through the engine's own `unit_can_traverse`.

## What it measured

**4,644 major scout unit-turns, 818 stationary (17.6%).** Of those:

| | |
|---|---:|
| unexplored land reachable, ignoring borders | 800 (97.8%) |
| unexplored land reachable, **by `unit_can_traverse`** | **800 (97.8%)** |

So a stationary scout almost always has somewhere legal to go. The two numbers
being identical kills the first hypothesis outright:

**1. It is not closed borders or foreign territory.** The engine's own
traversal rule admits exactly the same ground the naive land walk does. The
scout is not penned in.

**2. It is not `exploration_goal` returning `None`.** `explore_step` returns
`false` immediately when the goal search comes up empty, on the comment's
premise that *"the route search below would only flood the unit's whole region
to prove the same thing"* — and that premise is false, because the goal search
also filters out fog near another explorer's committed goal
(`EXPLORE_COMMIT_SEPARATION`) and near visible hostiles, which the exhaustive
search does not. That looked like the bug. Removing the early return so the
exhaustive `route_step_to_any` always runs produced a **byte-identical** probe
result — 4644 / 818 / 800 / 800, unchanged — so this path is not being taken.

Also confirmed, so it is not re-checked: `BasicAi::should_explore` returns
`true` unconditionally for `UnitDoctrine::Recon`, so a scout is always offered
the exploration branch. It is not being benched by that gate.

## What was decided

**Nothing shipped.** What remains is one of: an earlier branch in
`advance_unit_serial` claims the scout and ends its turn without moving it
(`fortify_or_stop` is every controller's "nothing better to do" ending), or
`step_toward` and `route_step_to_any` both fail on a route that exists. Telling
those apart wants instrumentation on the dispatch itself, not another guess
from the outside.

⚠ Recorded because the ruled-out half is the expensive half. Two well-motivated
fixes were written and measured here and both changed nothing; the border
hypothesis cost a probe and the goal-search hypothesis cost a build. Neither is
worth repeating.

One thing worth carrying separately: **the comment on that early return is
wrong** regardless of whether it is the cause. The goal search and the
exhaustive search do not answer the same question, so "the route search would
only prove the same thing" is not true. It was not corrected here because the
correction is a behaviour change to a hot path with no measured benefit behind
it, and this session has shipped enough of those to know better.
