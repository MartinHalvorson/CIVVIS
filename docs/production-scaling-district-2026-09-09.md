# Earlier industrial foundations

The production-scaling objective remains open. The Workshop reservation in
#3267 fixes a real choice, but an Industrial Zone must exist before that
reservation can do anything.

## Recorded bottleneck

Source: `civvis-20260909T010354Z/events.jsonl`, first `state` per turn.
Apprenticeship was known on turn 69 and several cities could build an Industrial
Zone immediately. Mediolanum could build one on turn 72, first placed one on
turn 125, and completed it on turn 145. Its first-state production timeline:

| Turn | Queue | Industrial Zone offer |
| --- | --- | --- |
| 90 | idle | 102 production, 14 turns |
| 91 | Library | available |
| 97 | University | available |
| 110 | idle | 126 production, 14 turns |
| 111 | Bath | available |
| 114 | idle | 129 production, 15 turns |
| 115 | Campus Research Grants | 76 production, 9 turns |
| 124 | idle | 82 production, 7 turns |
| 125 | Industrial Zone | placed, incomplete |
| 126 | Commercial Hub | Industrial Zone remains incomplete |
| 139 | Builder | Industrial Zone remains incomplete |
| 142 | Industrial Zone | resumes construction |
| 145 | idle | Industrial Zone complete; Workshop available |

On the exact event prefix ending at the first turn-114 state, both untouched
`3b280f575` and #3267's `6026a6775` choose Campus Research Grants. Replay used
`civvis_orders --serve --fresh-board --victory science --explain`, with `114`
on stdin; the journal prices the project at 47 in the Expansion plan. This is
an earlier decision that the building-only correction does not address.

This historical baseline is not current main: untouched `8cfc5ca75` already
chooses the Industrial Zone at (58, 26), worth 78, on the same prefix. The
district experiment chooses the same item at the same location, worth 107.
That replay demonstrates a changed valuation, not an improved decision
relative to the current baseline.

## Initial economic probe of #3267

[Probe source](production-scaling-probe-2026-09-09.rs),
[baseline rows](production-scaling-probe-2026-09-09-baseline.csv),
[building-treatment rows](production-scaling-probe-2026-09-09-buildings.csv).

Compile the identical standalone Rust source against each revision's freshly
built `civvis` library and dependency directory (`rustc --edition=2021 -C
opt-level=3 --extern civvis=<rlib> -L dependency=<deps> <source> -o <binary>`).
Run each binary with arguments `260909145 4`. Baseline is `3b280f575`; treatment
is `6026a6775`, the first production-compounding intervention. This comparison
does not measure the Industrial Zone change in this PR.

The source fixes six players, 74×46 Continents, nine city-states, Online speed,
a 250-turn cap, Emperor majors, Immortal barbarians, randomized civilizations,
and one named target per major seat in `VictoryTarget::ALL` order. It uses
native `AdvancedAi::targeting` and the engine's `run_game_observed` loop. It is
an exploratory fixed-genome comparison, not a gene-screen ledger source or a
live Civilization VI trial.

Means across the same 24 seats in all four matched games:

| Checkpoint | Metric | Baseline | Building treatment | Ratio |
| --- | --- | --- | --- | --- |
| 75 | Production/turn | 76.04 | 77.33 | 1.017 |
| 100 | Production/turn | 98.73 | 99.37 | 1.006 |
| 100 | Science/turn | 64.64 | 63.49 | 0.982 |
| 100 | Culture/turn | 43.55 | 44.27 | 1.017 |
| 100 | Integrated production rate | 5491.93 | 5523.99 | 1.006 |

`cumulative_production` sums start-of-turn production rates; it is not a count
of production actually spent. Later checkpoints have unequal game-ending
censoring and must not be pooled as though every seed reached them. The four
games, not the 24 interacting seats, are the independent experimental units.
This is far from a measured 10× gain and too small for a strength claim.

## Rejected earlier-investment candidate

The Industrial Zone scorer prices adjacency but omits the first production
building it unlocks. The experimental named-lane term valued that already-researched
first building's projected net production after paying its cost and waiting
for both construction steps. Future returns are discounted by half; the
existing district score still owns the district's direct adjacency valuation.
The estimate charges a worked tile's lost production when estimating the
post-district construction rate, and never spends current overflow twice.

Locked, already-built, nonproductive, second-tier and incompatible buildings
supply no credit. If there is insufficient time for positive net production,
there is no premium. Adaptive controllers retain their existing valuation.
The implementation and five passing focused tests are preserved in commit
`d4b778813`; the combined candidate is `127929515`. The final change removes
the candidate from the controller because the economic comparison below does
not support promoting it.

## District-only experiment

The identical four seeds compare untouched `8cfc5ca75` against its district
term in `d4b778813`, before incorporating #3267. All runs completed.

| Turn-100 metric | Control | District term | Change |
| --- | --- | --- | --- |
| Production/turn | 98.73 | 97.90 | −0.8% |
| Science/turn | 64.64 | 60.33 | −6.7% |
| Culture/turn | 43.55 | 42.31 | −2.9% |
| Integrated production rate | 5491.93 | 5527.54 | +0.6% |

These results do not justify promoting the district term. A separate matched
comparison against `f8124494d`, which includes #3267, tests whether actually
reserving the follow-on Workshop changes that conclusion. It uses the same
exploratory seeds, not independent confirmation seeds.

Raw rows: [district control](production-scaling-probe-2026-09-09-district-control.csv),
[district treatment](production-scaling-probe-2026-09-09-district.csv).

## Combined experiment and decision

All four seeds completed for untouched `f8124494d` (including #3267) and
`127929515` (the same main plus the district term). At turn 100 all 24 matching
seats remain available:

| Metric | Current-main control | Combined treatment | Change |
| --- | --- | --- | --- |
| Production/turn | 99.37 | 100.34 | +1.0% |
| Science/turn | 63.49 | 60.32 | −5.0% |
| Culture/turn | 44.27 | 39.42 | −11.0% |
| Integrated production rate | 5523.99 | 5629.18 | +1.9% |

Production changes per game are +4.0%, +2.5%, +9.4%, and −10.9%; culture
falls in all four games. This small development sample establishes neither
statistical significance nor live-game strength. It supplies no convincing
reason to accept the candidate's production/science/culture tradeoff.

**Decision: reject the additional district bonus.** Keep #3267; this PR's final
production controller matches its `f8124494d` baseline. The saved experiment
can inform future work without silently promoting an unsuccessful heuristic.

Raw rows: [combined control](production-scaling-probe-2026-09-09-combined-control.csv),
[combined treatment](production-scaling-probe-2026-09-09-combined.csv).
The [manifest](production-scaling-probe-2026-09-09-manifest.json) records source
and executable hashes and exact revisions for both district experiments.

The next production investigation should measure worked production tiles,
Builder charges and delivery time before trying another global score bonus.
Existing Builder-floor and first-Builder experiments already exist in the
controller; any new candidate should compare against them explicitly. No 10×
improvement has been demonstrated.
