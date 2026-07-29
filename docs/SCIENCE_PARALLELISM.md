# Adaptive Science Spaceport parallelism

Status: **preregistered; no treatment result has been read**.

## Observation and hypothesis

The eight most recent production final saves available on 2026-07-29 (through
`20260729T113536.259406Z`) all ended in Science victories. Across those games,
all 17 civilizations that had launched an exoplanet expedition had exactly one
Spaceport. Nine of the 17 were losing runners at 46, 39, 37, 34, 26, 16, 16,
6, and 2 light-years: mean 24.7, with the closest only four light-years short.
This is an observational census, not evidence that another Spaceport would
have helped.

The current controller supplies a precise causal candidate. `science_production`
targets one Spaceport for an adaptive `AdvancedAi`, even when its live grand
strategy is Science. It expands to two after the Moon landing and three after
the Mars colony only when the constructor gave it an explicit Science victory
target. The normal fleet uses adaptive controllers, while its caller invokes
the same production routine for a live Science plan. Thus the explicit and
adaptive versions of the same plan disagree exactly when independent launch
sites could parallelize the late project race.

**Frozen hypothesis:** after the Moon and Mars milestones, applying the
explicit-Science controller's two- and three-Spaceport schedule to an adaptive
controller while its reported plan is Science will increase terminal Science
race progress. The construction cost may instead delay more valuable work, so
win and score outcomes are preregistered harm guards rather than assumed
benefits.

## Treatment

`science_parallelism_eval` will run the shipped adaptive fleet in both arms.
On a focal treatment turn it will clone the controller, run the stock turn,
replay every successful logged action except the final `EndTurn`, and retain
the updated controller state. It may then issue at most one additional legal
`Produce` order, after which it applies the deferred `EndTurn`.

The order is eligible only when all of these conditions hold:

1. the focal controller reports `science` as its live strategy;
2. the Moon landing is complete (desired sites = 2), or the Mars colony is
   complete (desired sites = 3);
3. fewer than that number of distinct owned cities have a built or first-queued
   Spaceport; and
4. a different city can legally produce a Spaceport.

The evaluator selects the eligible city with the highest current Production,
then the lowest city id and tile position as deterministic ties. It never
grants Production, discounts a district, creates an otherwise unavailable
site, changes research or grand strategy, or orders more than one Spaceport in
a turn. Counting first-queued sites and excluding cities that already own one
matches the production policy's separate-city invariant. The splice occurs
after the stock policy, so it is a narrow evaluation proxy for the proposed
branch change, not the gameplay integration itself.

## Frozen experiment

Every independent map is replayed four times: focal seats 0 and 7, each under
control and treatment. All other seats use stock `AdvancedAi`. The inference
unit is the map; the two focal seats are averaged within a map and are never
counted as independent samples.

The fixed profile is:

- 8 players, randomized civilizations;
- Continents, Planet topology, Poles, requested 84×54, 12 city-states;
- Online speed, 320-turn limit;
- Science, Culture, and Domination victories; and
- stock embedded rules and adaptive fleet from the exact tested commit.

Before any treatment batch, a four-map null at seed **9,982,000** must compare
a direct stock control with the action-log replay arm while applying no added
order. All eight matched focal cells must be exactly equal. Unit tests must
also prove deterministic selection, the Moon/Mars 2/3 schedule, no action
outside a Science plan, and at most one legal order.

The one allowed development screen is **30 maps starting at seed 9,983,000**
(120 games). If and only if its frozen gate passes, the one allowed holdout is
**120 maps starting at seed 9,984,000** (480 games). No seed may be replaced,
extended, or retried; no threshold or treatment detail may change after a
treatment result is read. Compile and one-map runtime smokes are diagnostic
only and may not be used to tune the policy or gate.

## Endpoints and gates

For focal seat `s` on map `m`, let `R` be the engine's terminal 0–100 Science
victory-race progress and define

`d_m = mean_s(R_treatment - R_control)`.

This map-level Science delta is the primary endpoint. The evaluator reports
its mean, favorable/neutral/adverse map counts, and an exact two-sided sign
test over non-neutral maps. It also reports completed Science projects,
exoplanet distance and speed, completed Lagrange plus terrestrial laser
projects, built/queued Spaceports, total and Science wins, the paired map win
score, and paired terminal-score share.

The 30-map development gate passes only if every term holds:

- a successful treatment order occurs in at least 10% of the 60 focal
  treatment games and at least 10 orders execute in total;
- terminal built/queued Spaceports and completed lasers are both higher in
  aggregate under treatment;
- mean `d_m` is at least +0.5 percentage point, favorable maps outnumber
  adverse maps, and the exact two-sided sign-test p-value is at most 0.20;
- treatment has at least as many Science wins and total wins as control, and
  paired map win score is at least 50%; and
- paired terminal-score share is at least 49.5%.

Failure means **STOP**: retain `AdvancedAi`, do not tune, retry, inspect the
holdout, or integrate the treatment.

The fixed holdout passes only if the coverage and aggregate mechanism terms
still hold, mean `d_m` is positive, favorable maps outnumber adverse maps with
exact two-sided p < 0.05, treatment has at least as many Science and total wins
as control, paired map win score is at least 50%, and paired terminal-score
share is at least 50%. Failure means retain the controller. Passing permits a
separate gameplay-integration PR and its normal full promotion test; it does
not itself promote the policy.

## Resource rule

The large-map evaluator must not begin while another simulator batch is using
six or more cores. It will use no more than six jobs, leaving capacity for the
production spectator, builds, and collaborators. Validation and null runs may
use fewer jobs. The exact source commit, commands, wall time, and results will
be recorded here before this study is shipped.
