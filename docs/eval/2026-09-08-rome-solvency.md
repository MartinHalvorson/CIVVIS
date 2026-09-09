# Rome's science governor bypassed solvency recovery

Run: `civvis-20260908T173748Z`, Emperor, Trajan, Online, science target,
controller revision `cc4655df2`. Evidence is the retained `events.jsonl`,
`why.log`, and `runtime_updates.jsonl` in the host's
`civvis-civ6-runs/control/civvis-20260908T173748Z` directory. The completed game ended in a rival culture victory on turn 208: the
explicit outcome records `won: false`, `team: 3`, `victory: 3`; the seat
export maps victory index 3 to `VICTORY_CULTURE`. The process reason
`stopped` does not override that in-game result. This single loss is not a
controlled estimate of win-rate improvement.

## Observed failure chain

The following are the first exported state frames at each listed turn.

| Turn | Cities | Science/turn | Gold/turn | Military | Situation |
| --- | ---: | ---: | ---: | ---: | --- |
| 50 | 4 | 20.5 | 11.0 | 167 | Georgia already produces 85.3 science |
| 100 | 5 | 89.5 | -9.9 | 265 | Aquileia lost after negative loyalty |
| 125 | 5 | 120.2 | -14.6 | 119 | No major war; repeated unit losses |
| 150 | 8 | 153.5 | -18.0 | 50 | Rome constructing its Spaceport |
| 175 | 8 | 175.2 | -6.8 | 50 | Moon Landing still in production |
| 190 | 10 | 231.0 | -8.0 | 50 | Georgia has launched its exoplanet expedition |
| 200 | 8 | 148.3 | -10.2 | 66 | Ostia and Rome lost to war |

The first observed negative income was turn 87 (-3.5 Gold/turn). At turn
150, building and district maintenance were 42 and 16, while unit maintenance
was already zero. Military pruning alone therefore could not repair the
budget. Spies, Bombards, Field Cannons, and Artillery repeatedly disappeared
with zero or negative Gold before the major war. This is consistent with
bankruptcy attrition; the loss events do not explicitly label their cause, so
they are not all counted as proven disbands. Builder and Settler consumption
and great-person activation must not be counted as combat losses either.

The expansion count concealed poor timing and retention: the fifth city was
founded at turn 55, Aquileia at 93 was lost at 100 (last loyalty 8, change -14),
and the next retained foundations were at 139, 140, and 149. More cities late
in the game did not substitute for productive early cities.

Georgia produced 280.9 science at turn 150 and had already completed Moon
Landing. We had only Earth Satellite at 175 and Moon Landing by 190. War
then removed Ostia at 194 and Rome at 200. The science gap preceded the war;
the invasion compounded an existing economic and development failure. The
actual terminal result was culture, so Georgia's science lead must not be
misreported as the winning condition. Low domestic culture also left our
science attempt vulnerable to a different rival's victory clock.

## Implemented correction

`live_war_economy_requires_recovery` used to return false whenever
`war_economy` was disabled. The actual run's first `why.log` genome lists
`war-economy` in `ledger_withheld`, while its victory target is `science`.
Targeted science uses advanced production too, so that early return silently
bypassed the baseline solvency priority throughout peace.

Targeted science now receives the same low-reserve, negative-income guard
independently of the optional war gene. Empty queues select a route-ready
Trader, profitable building, or commercial district through the existing
recovery selector; its existing fallback uses productive upkeep-free items.
Committed queues and the undermanned major-war defense exception retain
their existing behavior. Solvent science production and untargeted agents
with the war gene off retain their prior routing. The regression exercises
actual production in peace with an explicit science target and war economy
disabled, and checks the positive-income and sufficient-reserve boundaries.

## Next-game acceptance criteria

Use the existing frozen-revision batch process and preserve complete outcomes.
Record the actual genome and binary identity, rather than assuming a merged
revision is already active in a running game. Compare exact turn 100, 150,
180, and 200 frames using `tools/civ6_race_audit.py` plus treasury, income,
maintenance, and retained-city history from the source states.

The immediate acceptance criterion is recovery production appearing while
income is negative and the reserve is below `100 + 25 * cities`, followed by
sustained positive income and retained military strength before invasion.
A production regression passing establishes routing, not positive income or
a live win. Further deficits with the guard active would require examining
commercial-district legality, trade safety, and committed queues.

Track rival culture pressure as well as science project completion; reaching
our science milestones is insufficient if another victory resolves first.
Separately measure early retained cities, Campus/Library completion, loyalty
losses, and Spaceport/project milestones. These remain improvement targets;
this patch does not claim to solve settling, loyalty, or the whole science
race. Judge success by completed games and science milestones, not score
alone, and do not count resumed copies as independent games.
