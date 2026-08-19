# A Builder repairs only what it is standing on

_2026-08-18 · `agent/mbp-m5-pro-64/claude-opus5-20260818`_

## What was asked

#2069 measured that Builders reach a decision and find no improvable tile on
90% of their turns, and recorded the open question: *what to do with a Builder
that has charges and no work.* The obvious answer — stop over-supplying them —
was tested first and **disconfirmed**.

## How it was measured

Counters inside the production decision and inside `builder_step`, over three
250-turn six-player games at the deployment shape, seeds 51000000–02.

## What it measured

**Builders are not over-supplied.** At the moment production decides to train
one, over 436 such decisions:

| | |
|---|---:|
| improvable tiles in the empire, mean | **15.6** |
| Builder charges already in the field, mean | **4.3** |
| decisions where charges already covered the work | 66 of 436 (15%) |

So the empire is short of Builder charges 85% of the time it decides to build
one. Gating harder on supply would have been a change in the wrong direction,
and the counter said so before any of it was written.

**But two definitions of "work" disagreed.** `has_builder_work` — the gate that
decides whether to *train* a Builder — counts a pillaged improvement anywhere in
the empire. The Builder's own empire-wide target sweep tested only
`valid_improvements`, which a pillaged-but-improved tile fails; repair was
handled for the tile the Builder already stood on and nowhere else.

| | no target | of which the gate says work exists |
|---|---:|---:|
| before | 3,704 | **508** |
| after | 4,229 | **0** |

508 builder-turns where the empire knew there was work and its Builder could not
walk to it. A razed farm earns nothing until it is repaired, so it is the one
kind of Builder work with a running cost — and it was reachable only by
accident.

⚠ The total rose from 3,704 to 4,229. That is not a regression: once Builders
start walking to repairs the games diverge from turn one, so the two runs are
not the same games. The number that is comparable is the second column, which is
the disagreement, and it is closed.

## What was decided

**Shipped, with the ledger bumped.** The empire-wide sweep now tests the same
predicate the gate does. The mutation check is the argument: with the sweep
restored to ignore repairs, the new test fails on *"a Builder with a repair to
reach must do something"* — the Builder does not move at all.

The frozen anchor moves, 18,572 decisions to 18,586, so `ELO_PROTOCOL_VERSION`
goes to **15** and `docs/ELO_REPINS.md` carries the entry. Unlike almost every
entry there, it is not an argument that the change was free.

⚠ **The strength effect is not measured.** The justification is that two
definitions of the same thing disagreed and one of them was reachable only by
accident — a defect repair, not a demonstrated gain. Rows before and after v15
are not comparable in any game where an improvement was pillaged.

## What is still open

**3,704 of the no-target turns are genuinely no work in the empire**, and this
does not touch them. Builders trained while work existed outlive it; every
acquisition path is gated at the moment of purchase and nothing retires a
Builder afterwards. A Builder costs no gold upkeep in this engine — checked,
`units.json` gives it none, matching Civilization VI — so the waste is the ~50
production already spent, and disbanding recovers nothing. The honest reading is
that the remaining idle Builder is a *sunk* cost, not a running one, and the
lever is upstream in how many are trained rather than downstream in what they
do.

`galley` at 27.8% idle and `horseman` at 23.7% remain the larger untouched
rates, and neither has anything to do with Builders.
