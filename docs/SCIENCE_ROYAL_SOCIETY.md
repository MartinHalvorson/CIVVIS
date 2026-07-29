# Royal Society as the Science government-plaza choice

This experiment asks whether the AI has already implemented a useful Science
finisher and then almost always chooses the mutually exclusive building that
turns it off. It is a policy test, not a bonus grant: the treatment changes one
legal same-cost production choice and leaves the engine, resources, research,
units, map knowledge, and turn budget unchanged.

## 2026-07-29 exploratory evidence

The evidence that motivated the test was read before this preregistration and
is not an effect estimate. The 70 production saves archived from 2026-07-29
through `20260729T145558.484596Z` contain 560 major-seat endpoints across many
successive live revisions. Sixty-seven games ended by Science and three by
Culture. Their government-plaza buildings were strikingly concentrated:

| building | seats holding it at the endpoint |
|---|---:|
| Ancestral Hall | 459 |
| Foreign Ministry | 424 |
| National History Museum | 399 |
| Queen's Bibliotheque | 12 |
| Grand Master's Chapel | 2 |
| Audience Chamber, Warlord's Throne, Intelligence Agency, Royal Society, War Department | **0 each** |

The same endpoints included 372 seats above 2,000 Faith without Grand Master's
Chapel and none above that balance with it. Those balances alone do not prove
that Faith was spendable or that another government-plaza choice wins games.
They only make the production policy worth inspecting.

That inspection supplies the causal mechanism. The default adaptive
`AdvancedAi` eventually delegates empty queues to `BasicAi::pick_item`, whose
last building fallback chooses the cheapest legal building and breaks a
same-cost tie by rule-data order. It does not value building effects. The
strategic production evaluator is richer, but its ordinary-building branch
also omits `builder_charge_space_project_pct`, `faith_purchase_land_units`,
`heal_on_unit_kill`, and the other government-plaza effects. National History
Museum receives value for four Great Work slots; Royal Society receives no
value for its defining effect.

The downstream policy is not missing. `BasicAi::builder_step` already routes a
Builder to a city running a Spaceport project, and `Game::do_contribute_project`
already consumes its charges for the Royal Society contribution. Engine and
AI contract tests cover both. The unmeasured link is the mutually exclusive
building choice that makes the existing behavior reachable.

## Preregistration: frozen before implementation or focal data

### Hypothesis

> When the live plan is Science and stock policy begins National History
> Museum, substituting Royal Society will cause real Builder contributions to
> Spaceport projects and improve game outcomes on the production profile.

The treatment is deliberately one cell, not a general government-plaza
rewrite. It does not change Tier 1 or Tier 2, does not alter any non-Science
plan, does not force a Builder or Spaceport project, and does not add a special
value to the shipped governor.

### Exact treatment boundary

On each focal turn, the evaluator clones the current game and focal controller,
runs the stock turn, and retains the exact successful action log plus the
controller state stock play produced. If that stock turn reports a Science
plan and contains `Produce(NationalHistoryMuseum)` for a city, the treatment
replays the same actions in the same order but replaces that one action with
`Produce(RoyalSociety)` for the same city. The replacement must be legal at the
exact replay point or the run fails; it is never synthesized by mutating a
queue. The final `EndTurn` remains deferred until all replayed actions finish.

Every other turn and every other action is stock. Once either mutually
exclusive Tier-3 building is committed, the opportunity cannot recur. The
wrapper records stock opportunities, successful substitutions, end-state
building counts, and focal `ContributeProject` actions.

This action-log splice is important. A pre-turn check would read the previous
assessment and could call a stale Recovery plan Science; a post-turn mutation
would bypass `Game::apply`; a permanently targeted Science controller would
test a different agent. Replaying the stock decision changes only the action
whose effect is under test.

### Exact null validation

Before reading a focal seed, a four-map diagnostic uses seed `9987999` on the
same profile below. Each focal seat is played once by ordinary stock and once
through the action-log replay with substitution disabled. The terminal
serialized `Game`, focal result, and census must match exactly for all eight
seat cells. Any mismatch blocks the treatment run.

### Fixed development screen

The untouched screen is 30 maps, two focal seats per map, each seat replayed
once as stock and once as treatment: 60 matched seat cells and 120 games.

```text
science_royal_society_eval --maps 30 --players 8 --width 84 --height 54 \
  --city-states 12 --turns 250 --speed online --map continents \
  --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9988000 --jobs 6
```

All non-focal majors use stock `AdvancedAi`; city-states and barbarians use
that controller's normal minor paths. Focal seats are 0 and 7. The two seats
are aggregated before inference, so the independent unit is the map, not an
individual start. The screen must run alone in the six-core simulator slot and
is queued behind every older registered batch, currently #561, #567, #570,
#574, #579, #589, and #591.

The evaluator reports, by arm:

- focal wins and victory types, reported turn, score, cities, Builders, and
  terminal Faith;
- National History Museum and Royal Society ownership;
- stock Science-plan substitution opportunities, successful substitutions,
  seat-game coverage, and `ContributeProject` actions;
- completed required Science projects and normalized Science progress, where
  each of Earth Satellite, Moon Landing, Mars Colony, and Exoplanet Expedition
  contributes one point and launched expedition distance contributes
  `min(distance, 50) / 50`, for a fixed 0-to-5 endpoint; and
- favorable, neutral, and adverse map directions for wins, score, and Science
  progress, with exact two-sided sign tests.

The treatment advances only if every term holds:

1. at least 10 of 60 treatment seat-games make the substitution;
2. at least 10 focal `ContributeProject` actions occur in treatment and they
   occur in at least five treatment seat-games;
3. treatment owns more Royal Societies and no more National History Museums
   than control;
4. paired-map win score is at least 52.5% and favorable win directions exceed
   adverse directions;
5. paired terminal-score share is at least 50%;
6. Science-progress favorable directions are not fewer than adverse
   directions and mean normalized Science progress is not lower; and
7. treatment Science wins are not fewer than control Science wins.

The first three terms are the mechanism gate. They prevent a null from being
read when the building never appeared or its already-implemented Builder policy
did not use it. The remaining terms prevent a faster project statistic from
advancing a policy that loses games or merely trades away another winning
route. Missing any term stops the line: no threshold fitting, broader
government-plaza rewrite, seed retry, or holdout.

### Disjoint holdout

Passing the complete screen earns one unchanged 120-map holdout at seed
`9989000` on the same profile. It must retain at least ten substitutions, ten
contributions across five seat-games, nonnegative score and Science-progress
directions, at least as many Science wins, and a favorable win direction with
a two-sided exact sign-test `p < 0.05`. Only that result permits a separate
gameplay PR. This PR remains an evaluator and scientific record regardless of
outcome.

No seed `9988000` or `9989000` map was generated or read before this document
was committed. The implementation, null replay, latest-main merge, and full CI
suite all precede any focal run.

## Implementation checkpoint

Commit `a147e1b` implements the frozen design in the new
`science_royal_society_eval` binary. Before this checkpoint was recorded:

- its six focused CI-profile tests passed, including exact stock-turn replay,
  the treatment boundary, map-level scoring, Science-progress accounting, and
  the screen/holdout gates;
- Clippy passed for the binary and its tests (the checkout retains unrelated
  pre-existing warnings elsewhere in the crate);
- a one-map, two-player, five-turn diagnostic at non-focal seed `9987998`
  reproduced both matched stock games exactly through null action-log replay;
  and
- no preregistered null, screen, or holdout seed had been generated or read.

The exact four-map null remains queued behind the older six-core experiments.
It will run only after merging the then-current `origin/main`, and its result
will be recorded before the development screen is permitted to start.
