# Developing an empire into a victory

The `victory-portfolio` gene implements a persistent primary objective, an
optional affordable secondary objective, and a gradual change in investment.
Expansion, Conquest and Recovery remain immediate postures. A city defense
can take priority without erasing the primary objective. Operator-assigned
victories remain fixed contracts; `--victory civvis` chooses adaptively.

The behavior is independently screenable through the ordinary gene registry.
The existing ledger continues to select deployment defaults. Implementation
tests establish capability; a small reach probe cannot establish a win-rate
improvement. Native and live adapters execute the same policy.

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
cities, yields, military, banks and victory milestones, not claimed successful
expenditure. A disabled seat records the legacy focus rather than reporting
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

Validation and game results are recorded here after the commands complete.

Predeclared reach and behavior comparisons: twelve games with the ordinary
mixed target distribution, seeds 914356900–914356911, and twelve games with
all measured seats adaptive, seeds 914357900–914357911. Both use the standard
six-major 74×46 Continents, nine-city-state, Online 250-turn Emperor profile,
the deployment-genome background, and `--genes victory-portfolio --p-on 0.5`.
The second batch changes target coverage and is reported separately. These
small independent-seat batches establish reach and expose regressions; neither
alone authorizes a deployment promotion.
