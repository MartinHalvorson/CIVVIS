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

The named victory lanes already carry an industrial-building premium. That
premium usually decayed with `(max_turns - turn) / max_turns`, independently
of build cost or the production returned. Price it instead by the remaining
investment's payback: subtract completion time from the remaining clock, then
compare production returned with remaining production cost. A profitable
investment receives full credit, a partial return receives proportional credit,
and a building that cannot complete before the clock receives none. Preserve
the regional reach discount and the existing separate power utility policy.
Unlimited games use a rolling speed-aware investment window.

Science also reserved its cheapest owed research building before the general
production scorer ran. Both reservation paths now let legal Industrial Zone
buildings compete using that same scorer. The reservation stays within the
research and industrial foundation: it does not admit unrelated districts or
repeatable projects. Other named lanes receive the payback valuation through
their normal scoring path. Adaptive seats retain the separately screened
industrial and research-reservation expressions.

Implementation: `src/ai/advanced/production_compounding.rs`; fixtures live in
its `tests.rs`. These test the valuation, all six named targets, terminal and
unlimited clocks, regional reach, and execution through the early Science
reservation. They do not establish an economic or win-rate improvement.

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
