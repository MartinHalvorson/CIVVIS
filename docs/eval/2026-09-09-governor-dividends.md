# Higher-level reflection: governor dividends (2026-09-09)

The retained live corpus still points to an economic deficit before the final
victory race. At the initial report snapshot, nine of 28 run directories had a
terminal result and none won. Five completed segments reached turn 200; none
led science at its last observation, although all five had a Spaceport and at
least one launch. Their completed launch counts were 1, 1, 2, 3 and 4 of four.
Continuation segments are not independent games, and missing early observations
are not early eliminations. These counts do not estimate a population win rate.

Earlier investment and science-endgame work already addresses city growth,
buildings, builders and launches. The existing policy code also performs
counterfactual evaluation. This round targets five remaining governor allocation
weaknesses. They are prioritized engineering hypotheses, not a statistical claim
that governor placement explains the five largest shares of all losses.

| Issue | Evidence and consequence | Added opt-in gene |
| --- | --- | --- |
| Research support stays at an old population center | In 341 of 1,276 established domestic Pingala observations, another city had over 1.5 times its population. Researcher/Connoisseur and yield multipliers can become more valuable elsewhere. | `pingala-follows-research` compares engine science plus half-weighted culture, including establishment downtime. |
| Settler population protection misses the production city | In 182 of 1,446 established Magnus observations, a Settler was queued elsewhere while his own city was not training one. Provision can save population only where the Settler completes. | `magnus-follows-settlers` moves Provision Magnus to an existing Settler job he can reach before completion. |
| Builder charges miss the current builder job | In 23 of 89 established Liang observations, a Builder was queued elsewhere while her city was not training one. | `liang-follows-builders` follows a committed Builder queue only when arrival is timely. |
| Revenue support is not reconsidered as cities mature | Reyna appeared in 92 domestic established observations. The source chooses a post at appointment or when unassigned, but does not compare established economic posts; this is a source hypothesis, not a measured count of profitable missed moves. | `reyna-follows-revenue` compares empire gold with each possible post and charges for downtime. |
| Envoy support can remain after raw envoys secure a city-state | Amani's effective envoy contribution is modeled by the engine, but economic governor review does not recycle it into another achievable suzerainty. The domestic-city census does not measure foreign Amani posts. | `amani-follows-suzerainty` requires a newly secured suzerainty without surrendering the old one. |

The census used 29 event files (one started after the initial report), retaining
the last state per integer turn in each file, turn 60 onward, and counting only
established governors assigned to an observed own city. Example observations:
Pingala at Ostia in `civvis-20260908T222710Z-cont3`, turn 91; Magnus at Setia in
`civvis-20260908T173748Z`, turn 157; Liang at Rome in
`civvis-20260908T225920Z-cont1`, turn 212. These are opportunities to investigate,
not necessarily legal or profitable moves: promotions, timing, occupied posts,
loyalty and war can rule them out. Repeated turns and continuations overlap.
The corpus lives under `civvis-civ6-runs/control`; aggregate reporting uses
`python3 tools/civ6_run_report.py --aggregate <control-directory>`.

## Behavior and limits

Each gene is independent, registered and disabled in default controllers. A
shared review allows at most one successful relocation every 16 Standard-duration
turns. It requires 20 Standard-duration turns of residence, sufficient remaining
time, no major war or war plan, and no Recovery/Conquest strategy. Both posts
must pass threat checks; own posts need at least 90 loyalty and no modeled
loyalty emergency or local barbarian alarm. Occupied destinations are excluded.
Durations use `Game::standard_duration`, including its shipped speed table.

Pingala and Reyna compare cloned engine worlds with the governor absent and
established at each candidate. The candidate must beat the old contribution by
25 percent plus one yield, and repay downtime within at most 40 Standard-duration
turns. Pingala's Spaceport and advanced space/tourism promotions are protected.
Magnus and Liang preserve an existing same-unit or district job and protect key
advanced promotions. Queues are never replaced by these genes. Amani needs
contact, peace, and an engine-confirmed new suzerainty; an existing old
suzerainty must survive removal.

Existing appointment/promotion, unassigned-governor repair and BasicAi loyalty
rescue remain in place. The new economic comparison is an approximation of
future value: it does not predict growth, wars, queue changes or all passive
promotion effects. Native counterfactual yields do not prove fidelity of every
host-observed effect. Defaults remain off pending stronger outcome evidence.

## Predeclared validation

Focused tests must execute actual reassignment actions, verify establishment and
relevant benefits, protect prior jobs/suzerainty, exercise shared guards and
confirm counterfactual evaluation leaves the real state unchanged.

Five independent fixed reachability probes: six games per gene, seeds starting
at 9268100, 9268200, 9268300, 9268400 and 9268500 respectively in the table's order.
Each uses Emperor, `firaxis-mix` rivals, rival-only handicap, three rival chairs,
and on probability 0.5. Use the native screen's remaining defaults. This is 30
games with 90 measured and 90 excluded rival chairs, not live one-versus-five
Firaxis AI. Do not extend the sample in response to results. These probes test
reachability and obvious regressions; neither significance nor default promotion
is expected from such a small sample.

Validation and screen results will be recorded below after completion.
