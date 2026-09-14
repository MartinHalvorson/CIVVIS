# Developing an empire into a victory

The `victory-portfolio` gene implements a persistent primary objective, an
optional affordable secondary objective, and a gradual change in investment.
Expansion, Conquest and Recovery remain immediate postures. A city defense
can take priority without erasing the primary objective. Operator-assigned
victories remain fixed contracts; `--victory civvis` chooses adaptively.

The behavior is independently screenable through the ordinary gene registry.
The existing ledger continues to select deployment defaults. Implementation
tests establish capability; a small reach probe cannot establish a win-rate
improvement. Native and live adapters execute the same policy. The existing
live experiment interface can seat it with `civvis_orders --victory civvis
--with victory-portfolio` (or add `--civvis-victory civvis --civvis-with
victory-portfolio` to a `civ6_play.py` launch). Supplying a named victory
instead preserves that assignment while testing its investment policy.

## Forecasts and selection

Each enabled victory has an absolute expected finish turn when observations
support one, an optimistic bound, confidence, preparation cost, readiness, a
bottleneck, and a durable-investment indicator. Confidence and selection
utility are engineering estimates, not calibrated win probabilities.

- Science sums missing prerequisites without double counting shared research,
  credits current study and queued production, and overlaps research with the
  sequential launch chain. Construction uses engine costs and multipliers. A
  launch requires a legal site. An expedition uses observed speed and required
  distance; future lasers are not free.
- Culture estimates visitors with per-rival tourism modifiers and solves
  against each known defender's growing domestic total. A growing defender
  can be the bottleneck even when it is not today's leader.
- Religion needs an actual religion, conversion targets, religious charges
  and purchase capacity. Funding and travel enter the estimate. Pressure and
  religious opposition make this a low-confidence estimate.
- Diplomacy uses observed earned points and Congress cadence. Favor alone
  does not predict a finish. Future votes remain uncertain.
- Domination needs known original capitals, capture capacity, wall damage
  when needed, and military advantage. Travel and campaigning enter the
  estimate; occupation and opponent responses remain uncertain.
- Score needs a finite actual clock and a leading observed score. It cannot
  win early and is not the opening objective merely because its meter rises.

Unmet opponents prevent a central forecast for conditions requiring their
defeat or standing. Missing capacity produces an explicit bottleneck, not a
fabricated infinite or zero-turn result. Forecasts never override legality.
These initial approximations do not model every future action or terrain
obstacle; the telemetry is intended to guide their further improvement.

Selection reviews every ten Standard turns and when cities, technology,
projects, diplomatic points, victory availability or assignment change. A
challenger must clear a minimum hold and a utility margin; an existing launch,
conversion economy or capital investment raises that margin. A supported
alternative can replace a blocked incumbent earlier. The secondary needs
compatible infrastructure, a credible finish near the available deadline and
a small preparation bill. A second objective is optional.

## Transition and spending

Foundation preserves expansion, defense and finite opening opportunities.
Buildup becomes eligible with four cities or Industrial-era development and
an approaching preparation deadline, durable progress, or the development
clock. Preference strength increases smoothly. A credible near finish can
trigger the finishing phase directly. Extending a verification cap does not
postpone normal development.

Research, government, policy and Great Person choices share the objective
resolution. Recovery and Conquest keep their immediate needs. Science launch
execution and Culture purchases can remain available independently of an
incidental economic posture. Primary infrastructure receives a bounded
production premium; a backup can finish installed buildings, not reserve a new
district program. Late detours and settlers that cannot repay their costs
receive a discount rather than a legality veto. Income repair, housing,
amenities, defenders and active queues retain their existing safeguards.

A secondary Culture finish can spend at most 20% of the opening Faith bank
in one turn, shared across observation refreshes. A primary Culture finish
can spend the ordinary available amount. Cheap backup research receives a
smaller preference and does not override the primary's forced milestone.

`StrategicAi` also uses credible near-term finishes at unfinished rollout
endpoints. Distant or uncertain races retain the economic/learned value. The
explicit `score_only_with_weights` factory remains an economic control.

## Observation and evaluation

The screen seat row includes `victory_portfolio`: current objectives, phase,
forecasts, switch counts and a bounded trace. The trace records observed
cities, yields, military, banks and victory milestones. Queue production rates
are classified by primary, secondary, other and idle allocation, with Settler
and Builder production identified separately. These are observed allocations
before orders, not claimed successful expenditure. A disabled seat records the legacy focus rather than reporting
counterfactual decisions as its actual decisions.

Live order replies expose the same snapshot in
`decision.victory_portfolio`. The existing persistent controller retains its
history between turns and replans. A process restart reconstructs a fresh
history, as it does for other unsaved controller memory; the first snapshot
does not claim to recover the lost commitment date.

The new report preserves the existing single-screen selection procedure:

```sh
python3 tools/gene_report_targets.py rows.jsonl --gene victory-portfolio \
  --out target-report.json
```

It separates adaptive (`civvis`) seats from each preassigned target. Gene
contrasts use on/off assignment and cluster uncertainty by game. Chosen
objectives and phases are post-treatment diagnostics, never causal strata.
Missing targets remain unknown; missing telemetry remains missing. Finish
error is reported only for realized same-lane wins, with losing games treated
as censored rather than assigned the opponent's finish turn.

## Completed comparisons

Both predeclared batches completed all twelve games and all 72 intended seats:
the ordinary mixed targets, seeds 914356900–914356911, and all-adaptive targets,
seeds 914357900–914357911. Both used six majors, 74×46 Continents, nine
city-states, Online 250 turns, Emperor, the deployment-genome background and
`--genes victory-portfolio --p-on 0.5`. They are independent-seat experiments;
uncertainty is clustered by game. These small batches do not justify changing
the deployment selection.

| Target distribution | On wins/seats | Off wins/seats | Win difference | Approximate 95% interval |
| --- | ---: | ---: | ---: | ---: |
| Ordinary mix | 7/41 | 5/31 | +0.9 pp | −17.3 to +19.2 pp |
| All adaptive | 7/37 | 5/35 | +4.6 pp | −10.9 to +20.2 pp |

The target report preserves every preassigned cohort separately. In particular,
the mixed batch's eight adaptive seats had a **−26.7 pp** contrast (three on,
five off; interval −83.7 to +30.4 pp), illustrating why neither this tiny
subgroup nor the all-adaptive field establishes an improvement. The mixed
batch ended eight times by Science, twice by Culture, once by Religion and
once by Score; the all-adaptive batch ended by Science in all twelve games.
It therefore supplies no adaptive win evidence for the other five endings.

In the all-adaptive batch, the recorded commitment median was turn **88 on
versus 125 off**, and mean primary changes were **1.30 versus 7.69**. These are
descriptive diagnostics: commitment medians omit seats without a recorded
commitment, and primary-change counts include provisional Foundation choices.
They show the intended timing and persistence behavior without establishing a
causal relationship between those diagnostics and wins.

Calibration remains a limitation. Across the seven treated Science winners,
the first recorded same-lane forecast overshot the realized finish by 292.1
turns on average; the latest recorded forecast before the win overshot by
24.2 turns (also its mean absolute error). The report now preserves both
horizons rather than treating a long-range capacity estimate as a last-minute
prediction. The continuation model omits future economic growth and unbuilt
accelerators; its clocks and confidence values must not be sold as calibrated
finish predictions or win probabilities. Larger preregistered comparisons and
forecast calibration are needed before promoting this policy.

Reports:

- [Mixed screen](gene_screens/fires/2026-09-14-victory-portfolio-mixed.json)
  and [preassigned-target breakdown](gene_screens/fires/2026-09-14-victory-portfolio-mixed-targets.json).
- [Adaptive screen](gene_screens/fires/2026-09-14-victory-portfolio-adaptive.json)
  and [target diagnostics](gene_screens/fires/2026-09-14-victory-portfolio-adaptive-targets.json).

Both batches used the archived binary built from checkpoint
`95095e0e7a6369e0127b86450eafd9a4d6f59577`, SHA-256
`2f87933f501ebf31586812788a41f31bbee8c8c72f88a3998edb0aefad547467`.
Their headers conservatively mark the source tree dirty at launch: the
registry marker/comment ordering had been corrected after that clean binary
build. The original stamps are preserved. These are prototype reach and
behavior checks, not ledger-eligible sources or strength estimates for the
later integration fixes. They predate queue-allocation telemetry, which stays
missing in their reports; the clean smoke below exercises that addition.
Source paths, SHA-256 hashes, intended sample sizes and build metadata are
retained in the target reports. The games evaluated AdvancedAi's portfolio;
StrategicAi's endpoint change is covered by Rust tests, not this comparison.

A subsequent two-game smoke check used source `b8a832f6f9cd`, seeds
914358900–914358901, three majors on 40×28, no city-states, Online 100-turn
Emperor, all-adaptive targets and `--p-on 0.75`. Both games completed; all six
seats recorded queue allocation, and five enabled seats exercised the new
policy. Its clean build stamp and reports are in
[`2026-09-14-victory-portfolio-smoke.json`](gene_screens/fires/2026-09-14-victory-portfolio-smoke.json)
and the adjacent `-targets.json`. This shortened, nonstandard profile proves
execution and serialization; its five-on/one-off outcome contrast is not a
strength estimate or a deployment-selection source.

Focused tooling checks passed: 12 target-report tests, 14 registry append-point
tests, 17 gene-reach tests, the zero-unproven-genes ratchet, the gene-ledger
consistency check and the evaluation-manifest check. The broad local tooling
run was stopped after unrelated macOS launcher tests invoked GUI scripts and
timed out; the isolated GitHub tooling job supplies the full tooling gate.

Implementation revision `b8a832f6f9cd` passed 3,720 tests with zero failures (53 ignored),
including all 25 portfolio tests. The game-screen tool passed all 71 tests
and built successfully for the clean smoke run above. Exact commands:

```sh
cargo test --profile ci --locked -- --test-threads=4
cargo test --profile ci --locked --features developer-tools --bin gene_screen
```

Integration with `main` through `57696e76d` preserved the incoming housing,
luxury and hostile-memory treatments, resolved appended-registry conflicts,
and regenerated evaluation documentation. The combined source passed the command
below, plus all 14 append-point and seven documentation-command tests. CI
checks the full combined tree independently.

```sh
cargo check --profile ci --locked --features developer-tools --lib --bins
```
