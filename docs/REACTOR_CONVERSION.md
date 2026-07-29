# Reactor conversion without oscillation

Status: **preregistered and implemented on draft PR #622; the exact default-off
null is queued behind the shared simulator, and no focal seed has been read**.

## Production observation

The latest 50 completed production saves available on 2026-07-29, through
`20260729T124923.092707Z`, are one homogeneous deployment cell: eight major
civilizations, Continents, Planet topology, Poles, Online speed, a
policy-visible turn limit of 250, and external continuation until an enabled
victory. They ended in 48 Science and two Culture victories at a mean turn of
274.3 (range 219--343).

Those saves expose an unexpectedly large production sink. A completed reactor
conversion is counted in `Player::counters`, and each count represents a real
repeatable project which replaces the city's current power plant. Across the
400 major-civilization records:

| production observation | count |
|---|---:|
| seats completing at least one conversion | 297 / 400 |
| seats completing at least ten | 213 / 400 |
| seats completing at least one hundred | 57 / 400 |
| coal conversions | 8,062 |
| oil conversions | 6,515 |
| uranium conversions | 2,336 |
| all conversions | **16,913** |

The rules price coal, oil, and uranium conversion at 200, 300, and 400 base
Production. Online speed pays 50% of those costs, so the observed counts
represent approximately **2,250,650 Production** spent on conversion projects,
or 5,627 per major seat including seats that never converted. Affected seats
averaged 56.95 conversions and 7,578 Online Production. Winners averaged
102.28 conversions while non-winners averaged 33.71; that difference is
descriptive and confounded by survival, empire size, and production capacity,
not evidence that conversion helps.

Recommissioning cannot explain the count: the same saves contain only 59
recommission projects and 20 reactor accidents. The largest single record
completed 424 conversions (197 coal, 198 oil, 29 uranium), nominally 55,200
Online Production. Another completed 276 conversions in a 284-turn game.

This archive census is prevalence evidence, not a treatment result. It uses no
focal seed below and cannot establish what the freed production would buy.

## Mechanism hypothesis

The engine correctly makes a conversion to the city's *current* plant
ineligible. The Advanced production policy nevertheless values each other
conversion by the target plant's absolute utility:

```text
450 + min(strategic stockpile, 50) * fuel weight
    + climate phase * cleanliness weight
```

The fuel and cleanliness weights are coal `(18, -110)`, oil `(20, -55)`, and
uranium `(55, 130)`. The score never subtracts the utility of the plant already
owned. This creates a structural cycle even when the world is static. If A has
the highest absolute value, the city converts to A; A then becomes the one
illegal target, leaving positive-valued B as the best alternative; after the
city converts to B, A is legal and highest again. The existing unit test proves
only that uranium outranks coal in a late climate phase. It does not test the
reverse decision after uranium becomes current.

**Frozen hypothesis:** valuing only a strict improvement over the current
power plant will remove the deterministic conversion cycle, preserve legitimate
responses to fuel and climate conditions, and return enough production to
improve game strength without reducing reliable power.

## Frozen treatment

A later implementation PR may add a default-off treatment private to the
pinned evaluator runner. It must not add a public factory whose controller can
drift from the evaluator, or change shipped `AdvancedAi` while this experiment
is running.

For a coal, oil, or uranium conversion candidate, the treatment will:

1. identify the city's one current coal, oil, or nuclear power plant;
2. compute the target and current utilities with the exact existing stockpile,
   fuel-weight, climate-phase, and cleanliness-weight terms above, omitting the
   common `450` because the city already owns a plant;
3. return the existing ineligible score (`-10_000`) unless target utility is
   strictly greater than current utility by more than floating-point epsilon;
4. otherwise use `target utility - current utility` as the raw conversion
   value; and
5. leave the existing remaining-cost, turns-to-complete, and horizon discounts
   unchanged.

The treatment fails closed if the current plant cannot be identified. It does
not add a cooldown, remember hidden history, grant resources or Production,
change project costs or legality, change fuel consumption, change climate,
alter power distribution, reset reactor age independently, or touch
recommissioning. Every non-conversion production score remains byte-for-byte
the stock calculation.

Focused tests must establish all of the following before a focal run:

- in a fixed state, converting from the lower-utility current plant to the
  higher-utility target has positive marginal value;
- after applying that conversion, every lower-utility reverse conversion is
  ineligible, rather than merely receiving a mildly negative score which
  `advanced_production` could still queue;
- a real reversal of fuel/climate utility can still make a different plant
  eligible; and
- the default-off entrant is replay-identical to stock when its flag is off.

## Frozen evaluator

Add an observer/evaluator-only `reactor_conversion_eval`. Each independent map
is replayed four times: focal seats 0 and 7 under the pinned generation-14
`AdvancedAi` champion and under its default-off treatment entrant. All
non-focal majors use that same pinned champion; city-states and barbarians use
the same minor path in both arms. The two focal seats are averaged inside their
map, and the map is the only inference unit.

Before implementation is committed, the controller and runner integrity are
bound prospectively. Every major uses the exact committed `advanced_evolved`
generation-14 champion embedded from `data/evolved/best.json`, with FNV-1a
fingerprint `0x40b1fbb2a5b88bc6`; minors retain the Basic path. The runner
prints and asserts both values. It rejects unknown or positional tokens,
missing values, malformed values, and every duplicate option occurrence. An
official phase requires one canonical occurrence of every frozen argument,
including `--ai advanced_evolved` and `--jobs 6`.

The registered 84x54 Planet request resolves through the engine to 105x44 and
4,412 map tiles. The runner prints realized geometry and asserts those exact
values before any registered game advances; a future topology change therefore
invalidates the phase instead of silently changing its population.

The runner asserts `Game.max_turns = 250` at construction, immediately after
every controller action, after fallback `EndTurn`, and at the terminal
boundary. A controller that does not end its live turn receives one ordinary
fallback `EndTurn`; failure or failure to advance is fatal. These are execution
contracts, not changes to the treatment, endpoint, screen, or confirmation.

The runner must preserve `Game.max_turns = 250` and continue the same game and
stateful agents externally through turn 320 or an enabled victory. It must
assert that the policy-visible horizon never changes. For each arm it reports:

- completed coal, oil, uranium, and total conversions;
- conversion completions per 100 observed focal turns and the share of focal
  games completing at least one;
- nominal Online Production spent on those projects (100/150/200 per
  completion) and the paired amount avoided;
- terminal powered-city share, plant mix, resource-shortage records,
  recommissions, and reactor accidents;
- total and Science wins, Science-race progress, finish turn, terminal score,
  cities, districts, and buildings; and
- paired map win score, paired terminal-score share, and complete
  favorable/neutral/adverse map directions with exact two-sided sign tests.

The evaluator must not read its new diagnostics when making any game decision.
Generated output is not committed.

For the frozen safety endpoints, powered-city share is the pooled share of
owned terminal cities with positive Power demand whose full demand is met;
cities with no powered building are outside that denominator, and an arm with
no demanding city has share 100%. A resource-shortage record is one unpaid
fuel unit in the terminal `strategic_resource_shortages` values, summed across
resources and focal seats. Conversion and recommission counts come only from
the engine's cumulative `project:*` counters, and reactor accidents from its
cumulative `reactor_accident:*` counters.

## Exact default-off null

Before treatment data, one four-map null at seed `9975999` compares the pinned
controller with the same controller construction path while the marginal flag
remains off:

```text
reactor_conversion_eval --null --ai advanced_evolved --maps 4 --players 8 \
  --width 84 --height 54 --city-states 12 --turns 250 \
  --observe-through 320 --speed online --map continents --shape planet \
  --poles poles --randomize-civs --victories science,culture,domination \
  --seed 9975999 --jobs 6
```

Both focal seats on all four maps must match in the complete terminal
serialized `Game` and every reported endpoint: eight exact cells or **STOP**.
No screen seed may be opened after a null mismatch.

## Fixed development screen

The one allowed screen is 12 maps / 48 games:

```text
reactor_conversion_eval --ai advanced_evolved --maps 12 --players 8 --width 84 --height 54 \
  --city-states 12 --turns 250 --observe-through 320 --speed online \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9976000 --jobs 6
```

The screen passes only if every term holds:

1. stock control completes at least 24 conversions and at least 25% of its 24
   focal games complete one, establishing the mechanism on the fresh cell;
2. treatment conversions per 100 focal turns are at most 25% of control;
3. the paired nominal saving is at least 250 Online Production per focal game;
4. treatment terminal powered-city share is no more than two percentage points
   below control, and treatment has at most two more reactor accidents;
5. paired map win score and paired terminal-score share are each at least
   49.5%; and
6. treatment total wins and Science wins are not fewer than control.

Failure means **STOP**: retain `AdvancedAi`, record the result, and do not tune
the weights, add a cooldown, change the seed, or inspect the confirmation.

## Fixed confirmation and decision

A passing screen earns one unchanged 60-map confirmation at seed `9977000`
(240 games). Every other argument, including `--ai advanced_evolved`, and the
treatment remain fixed. Confirmation requires every term below:

- stock again has at least 25% focal-game conversion coverage;
- treatment conversion completions per 100 focal turns remain at most 25% of
  control and save at least 500 nominal Online Production per focal game;
- treatment powered-city share is no lower than control, resource-shortage
  records are no higher, and reactor accidents are no more numerous;
- paired map win score is at least 52%, favorable win directions outnumber
  adverse directions with an exact two-sided sign-test `p < 0.05`, and
  treatment has no fewer total or Science wins; and
- paired terminal-score share is at least 50%.

The confirmation is a direct test of stock `AdvancedAi`, not automatic evidence
about the four-times-costlier `strategic_deep` controller. A pass permits only
a separate, preregistered Strategic transfer/integration PR; it does not flip a
default or promote an entrant here. A failure retains the stock policy and the
negative result. There is no pooled rescue, seed retry, threshold fitting, or
post-result treatment change.

The six-job batches remain behind the shared-host simulation queue. They must
not start while another process owns six or more simulator cores.

## Prospective implementation checkpoint

The runner-integrity and controller-binding amendment above was committed as
`6cd15b9`, endpoint definitions as `eed0293`, and realized geometry as
`0c217ab`, all before the evaluator implementation commit and before any null,
screen, or confirmation read. They add only a disjoint default-off null and
reproducibility/liveness guards; the already-frozen treatment, screen seed,
confirmation seed, endpoints, thresholds, stop rules, and map-level inference
are unchanged.

The default-off treatment was implemented at `d63fad7`; the matched runner
followed separately at `5d2d5f5`. A transient public entrant constructed
default weights instead of the runner's pinned generation-14 genome, so it was
removed before any registered seed was opened. Before any
registered run, `f1f10b8` restored the stock expression's exact left-to-right
floating-point evaluation order and added a fixture where regrouping changes
the IEEE-754 result, while `b320c93` bound conversion rates to counted focal
actor turns instead of reported game turns. The focused runner contract is 8/8
green, and the underlying marginal-valuation and bit-level stock-arithmetic
tests pass. A one-map, one-turn, one-job null diagnostic used
nonregistered seed `97590`: it printed generation 14 and FNV-1a
`0x40b1fbb2a5b88bc6`, realized 105x44 / 4,412 tiles, reproduced both focal
results and complete serialized terminal Games exactly, and labeled itself
diagnostic only. Unknown, positional, duplicate, valueless, and wrong-controller
CLI probes all exit 2 before game construction.

## Registered result (2026-07-29)

The exact default-off null used seed `9975999` and passed. All eight matched
focal results and complete serialized terminal `Game` values reproduced
exactly. Both arms recorded 0/8 wins, mean turn 294.0, mean score 598.5,
92/79/17 coal/oil/uranium conversions, 5/8 conversion coverage, 2,304 observed
focal turns, 27/29 powered demanding cities, 17 shortage records, and one
nuclear plant. Every paired map and reported endpoint was neutral.

That pass admitted the one fixed development screen at seed `9976000`. Its
requested 84x54 Planet geometry again realized exactly 105x44 with 4,412
tiles. The complete 12-map / 48-game screen produced:

- 845 control conversions (414 coal, 371 oil, 60 uranium) versus 620 treatment
  conversions (298 coal, 301 oil, 21 uranium), with coverage 18/24 versus
  17/24 focal games;
- conversion rates of 12.410 versus 8.975 per 100 observed focal turns, so the
  treatment retained 72.3% of control rather than the required maximum 25%;
- 29,900 nominal Online Production avoided, or 1,245.8 per focal game;
- powered-city share of 75.00% versus 82.50%, shortage records of 41 versus
  60, recommissions of 1 versus 9, and reactor accidents of 1 versus 4;
- 3/24 control wins versus 2/24 treatment wins, all Science victories; and
- paired map win score 47.9% (0 favorable, 11 neutral, 1 adverse; exact
  two-sided sign-test p=1.0000) and terminal-score share 49.86% (4 favorable,
  2 neutral, 6 adverse; p=0.7539; mean score delta -4.58).

The mechanism, savings, powered-city, and terminal-score-share gates passed.
The conversion-rate, accident, paired-win-score, total-win, and Science-win
gates failed. Per the frozen rule this is **STOP**: retain stock `AdvancedAi`,
do not tune or retry this treatment, and do not inspect seed `9977000`. The
confirmation seed remains unopened.
