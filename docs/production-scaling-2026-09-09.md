# Production scaling: baseline and first intervention

The operator asked for strategies that scale production fast alongside science,
with culture and other supporting development, and a 10× improvement. This is
an open outcome target. A larger scoring coefficient or a passing fixture does
not establish a 10× improvement in production or playing strength.

## Live baseline

Source: `civvis-20260909T010354Z/events.jsonl`, latest `kind=state` record for
each listed turn, read from the local Civilization VI control archive on
2026-09-09 UTC. Production is the sum of own cities' `yields.production`;
science and culture are the state's reported empire totals. Counts include
completed districts and standing buildings only. These fixed earlier turns
remain usable even while the run continues.

| Turn | Cities | Production/turn | Science/turn | Culture/turn | Industrial Zones | Workshops | Factories | Campuses | Universities |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 50 | 3 | 27.00 | 13.00 | 11.20 | 0 | 0 | 0 | 1 | 0 |
| 100 | 6 | 52.21 | 67.65 | 28.56 | 0 | 0 | 0 | 5 | 1 |
| 125 | 7 | 85.00 | 121.09 | 39.25 | 0 | 0 | 0 | 7 | 4 |
| 150 | 8 | 148.00 | 140.52 | 58.05 | 1 | 0 | 0 | 7 | 5 |
| 160 | 8 | 164.00 | 156.02 | 59.55 | 1 | 0 | 0 | 7 | 6 |

This is diagnostic evidence of a thin industrial foundation, not an A/B result.
It also establishes that Industrial Zone access is an earlier bottleneck than
building valuation alone. A single live trace cannot establish typical strength.

## First intervention

At the first turn-145 state, Mediolanum was idle after finishing its Industrial
Zone and had a legal Workshop. The recorded live Science run chose a Spy
(visible under construction in the first turn-146 state). The city already
held its Library and University. A fixture reproduces a Spy's higher generic
bid; industrial scoring alone does not ensure the foundation is built.

Idle, safe cities in named lanes now reserve a legal industrial building when
its projected production return repays its remaining cost before the clock.
This reservation follows the existing survival, research, growth, solvency and
amenity reservations, preserves commitments, and excludes active Science launch
cities. It makes the production foundation precede discretionary production.

The payback estimate subtracts construction time and compares printed local
production plus existing discounted regional reach against remaining cost. A
profitable investment receives full credit, a partial return proportional
credit, and a building that cannot complete receives none. This is a projection,
not a full simulation of future policies, citizen allocation or amenities.
Unlimited games use a rolling speed-aware window. Separate power utility stays
under its existing policy.

The named lanes' industrial premium uses that estimate too. The older expression
could decay with the fraction of the whole game remaining; **the recorded live
genome already enables `chain-payback-window-2`, which addresses much of that
problem**. The queue reservation, not a claim that this live gene was absent,
is the central intervention.

Science also reserved its cheapest owed research building before the general
production scorer ran. Both reservation paths now let legal Industrial Zone
buildings compete using that same scorer. The reservation stays within research
and the industrial foundation. Adaptive seats retain their separately screened
industrial and research-reservation expressions.

Implementation: `src/ai/advanced/production_compounding.rs`; fixtures live in
its `tests.rs`. They test valuation, all six named lanes, terminal and unlimited
clocks, regional reach, and actual queue selection. Fixtures and one-turn replay
do not establish a production-curve or win-rate improvement.

## Validation of this intervention

- `cargo test --profile ci --locked`: 3,216 passed; 45 library tests and four
  documentation examples ignored; no failures.
- `python3 tools/rust_quality.py --base origin/main --head HEAD`: changed lines
  formatted and warning-free.
- Eight focused `production_compounding` tests passed.
- A byte-for-byte prefix of the recorded `events.jsonl` through the **first**
  turn-145 state was replayed with `civvis_orders --serve --fresh-board
  --victory <lane> --explain`, supplying `145` on stdin and closing stdin.
  All six lane replies selected `BUILDING_WORKSHOP` for city `262147`
  (Mediolanum). The Science journal reports the industrial payback reservation.
  This emits proposed orders only; it does not actuate the live game.
- Replay input SHA-256: `0799e9d7da7987803f15abd16db7409451b894c0609baafbb40aeae82a295558`.

Use the persistent replay mode for this check: its mid-game restart skips the
opening book. A one-shot invocation at a late turn is not equivalent and was
excluded from the result. The prefix stops at the idle decision, before that
turn's subsequent orders can turn the test into a committed-queue comparison.
These are choice and compatibility checks, not measured production growth.

## Remaining outcome work

- Measure actual production curves at matched turns and cumulative output on
  comparable configurations, including city count and output per city. The
  10× target refers to an observed outcome, not a scoring premium.
- Compare science, culture, solvency and survival alongside production so the
  strategy still develops and wins. Keep the rules and difficulty constant.
- Investigate early Industrial Zone timing, builder improvements, growth,
  trade routes, policy selection and regional factory placement. Building
  premiums cannot help a city that never obtains the district.
- Validate changed choices on replayed live boards and then follow integrated
  live games. Native fixtures alone are insufficient evidence of live strength.
