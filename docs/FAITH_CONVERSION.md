# Converting surplus Faith when Religion cannot win

This experiment asks whether `AdvancedAi` leaves a useful currency stranded on
the exact exhibition ruleset. It is a policy test, not a resource grant: the
treatment receives no Faith, discount, information, or extra action that the
control could not legally use.

## 2026-07-29 preregistration

The production exhibition enables Science, Culture, and Domination, but not
Religious Victory. In the most recent 100 completed league records at the time
of preregistration, 95 games ended in Science and five in Culture. A live
turn-197 snapshot contained four major civilizations holding 818 to 2,847
Faith, including three above 2,200. Those observations are motivation, not an
effect estimate: leader mix, plans, legal purchase opportunities, and the
winner are confounded.

The mechanism is explicit in `AdvancedAi`. `culture_spending` buys a Naturalist
when a park site exists and otherwise maintains at most two Rock Bands after
Cold War. The units already have routing and execution policies. That purchase
pass runs only while the current grand strategy is `Culture`. On the
three-victory exhibition profile, a Science, Expansion, Conquest, or Recovery
plan can therefore finish with spendable Faith even though Religion cannot
convert it into a victory.

### Fixed hypothesis and treatment

> When Religious Victory is disabled and Culture is enabled, using otherwise
> legal surplus Faith for cultural assets should improve conversion without
> weakening the current production, research, military, or Gold plan.

The treatment is deliberately narrower than “spend more Faith.” After the
stock `AdvancedAi` has completed a focal turn, and only when its reported plan
is not `Culture`, it executes at most one of the same purchases the existing
Culture pass would make:

1. buy a Naturalist if none is active and a legal Faith purchase is offered;
2. otherwise buy a Rock Band if fewer than two are active and a legal Faith
   purchase is offered.

Naturalists retain priority, matching the shipped pass. The action is selected
from `Game::legal_actions_within`; the treatment cannot synthesize an illegal
purchase or see through fog. Applying it after the normal turn is conservative:
the new unit waits until the next turn, whereas an integrated purchase in the
existing pass could act immediately. Culture-plan turns are untouched, so the
test isolates removal of the plan gate rather than doubling the purchase
cadence of an already committed Culture agent.

Every other major remains stock `AdvancedAi`; city-states and barbarians follow
the same minor path as that controller. The focal policy is evaluated from
seats 0 and 7 on each independently generated map. Each seat is played once
with and once without the treatment, so every comparison preserves seed, map,
civilization, opponents, and focal start. The two seats are aggregated before
inference; one map, not one seat-game, is the independent unit.

### Fixed development screen

The untouched screen is 30 maps (60 matched seat cells, 120 games):

```text
faith_conversion_eval --maps 30 --players 8 --width 84 --height 54 \
  --city-states 12 --turns 250 --speed online --map continents \
  --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9980000 --jobs 6
```

The evaluator must print the resolved profile, progress, per-arm wins and
victory types, terminal Faith/tourists/score, non-Culture purchase
opportunities, purchases by unit, seat-game coverage, and favorable/neutral/
adverse map directions for both wins and terminal score. A null replay mode
must reproduce the control exactly before the treatment result is interpreted.

The treatment advances only if every term below holds:

- at least 10% of treatment seat-games make a purchase;
- the treatment makes at least ten total purchases, so a one-off cannot be
  read as the mechanism;
- paired map win score is at least 52.5%;
- favorable win directions outnumber adverse directions;
- paired terminal-score share is at least 50%; and
- treatment Culture wins are not fewer than control Culture wins.

This is a development/mechanism screen and cannot change gameplay. A miss on
any term stops the line: no purchase-cap change, unit-priority change, seed
retry, or threshold fitting on these maps.

### Disjoint holdout and promotion rule

Passing the screen earns one unchanged 120-map holdout at seed 9981000 on the
same profile. It must retain at least 10% seat-game coverage and ten purchases,
have more favorable than adverse win directions, retain at least 50% terminal
score, and clear a two-sided exact sign test at `p < 0.05` in the favorable
direction. Only that result permits a separate gameplay PR which moves the
already-tested call into `AdvancedAi`; the development PR itself remains an
evaluator and record.

The holdout is specific to the exhibition's three-victory cell. All-victory
games retain a competing religious use for Faith and are outside the claim.
