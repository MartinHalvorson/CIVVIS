# Converting surplus Faith into an already-chosen district

Status: preregistered; no evaluator implementation or focal seed has been
read.

## Motivation

The frozen terminal census in [TERMINAL_FAITH_OPPORTUNITIES.md](TERMINAL_FAITH_OPPORTUNITIES.md)
examined 50 production saves through
`20260729T161608.260324Z-seed-416573342-turn-302-instance-39983`. Among 258
unblocked major seats holding at least 2,000 Faith, 47 (18.2%) had a legal
Faith-purchased district. Forty of the 138 seats above 5,000 Faith (29.0%) had
one. This is descriptive motivation, not evidence that buying a district
wins.

The action is not an oracle grant. It requires an established Moksha with
Divine Architect, an ordinary legal district site, the required technology or
civic and population capacity, and enough earned Faith at the exact decision
point. The engine already enforces all of those conditions.

The shipped controller has a narrower missing link. Its Gold purchase pass
scores `BuyDistrict` with the same production evaluator used for an ordinary
district choice, but discards the action whenever its explicit currency is not
Gold. Its Faith spending passes consider Great People, religious units,
military units, buildings, Naturalists, and Rock Bands, but never a district.

The earlier cross-plan Naturalist/Rock Band experiment is closed. It made 234
legal purchases and reduced terminal Faith 62.8%, but its 50.8% paired map
score missed the frozen 52.5% development gate. This experiment does not
revive that policy or allow a treatment-side Culture asset choice.

## Question and hypothesis

> When stock `AdvancedAi` has already chosen to produce a district, does
> spending otherwise-reserved Faith to complete that exact district improve
> matched game outcomes?

The causal treatment must not choose a different district, city, tile,
Governor, promotion, technology, civic, government, policy, or victory plan.
It may only replace one stock `Produce` action with the corresponding legal
Faith `BuyDistrict` action.

The hypothesis is that immediate completion creates useful tempo and converts
a measured surplus without overruling the controller's strategic choice. The
null is that the Faith reserve and ordinary production timing are at least as
valuable.

## Frozen intervention

Every focal turn runs a cloned `AdvancedAi` from the authoritative start state
and records its successful action trace. The evaluator then replays that trace
on the authoritative game, retaining the cloned controller's resulting
internal state. The final `EndTurn` is deferred until replay and treatment are
complete. This is the same deterministic action-log seam already validated by
`faith_conversion_eval`; no opponent or future state is inspected.

Control and treatment both use this replay path. In treatment only, scan the
stock trace in its original order. Immediately before each action of the form

```text
Produce { city, item: District { district, pos } }
```

inspect the authoritative state's ordinary legal purchase actions. A
substitution is eligible only when all of these frozen terms hold:

1. this focal turn has made no earlier district substitution;
2. the controller's reported strategy is not `culture`;
3. an exact `BuyDistrict { city, district, pos, currency: "faith" }` action is
   legal at this point;
4. applying that action to a clone leaves at least 100 Faith, or 250 Faith
   when the reported strategy is `religion`.

The reserve matches the shipped Great Person pass and is at least as large as
the ordinary non-Culture building reserve. Because the substitution point is
after the stock trace's existing Faith-spending actions, Great Person,
religious, military, building, Naturalist, and Rock Band priorities have
already had their ordinary first opportunity.

For the first eligible action only, apply the exact Faith purchase and omit
the corresponding `Produce`. Replay every later stock action unchanged and
then end the turn normally. Any failed stock replay or failed selected
purchase is a harness error, never an ignored action.

There is no treatment-side scoring or tie-break. Multiple candidates are
resolved solely by the stock controller's action order. The treatment grants
no Faith and gets no discount, extra Governor title, extra action, hidden map
information, or opponent information.

### Conservative production cost

An immediate gameplay implementation could choose the city's next production
item after completing the district. This evaluator deliberately does not
invent that choice. The rest of the already-recorded stock trace is replayed,
so the purchased city normally reaches the turn boundary with an empty queue
and forfeits that production tick. Report the resulting base Production
exposure for every substitution.

A positive result therefore survives a real implementation handicap. A
negative screen rejects this exact splice and does not prove that a future,
separately preregistered same-turn replanning implementation would fail.

## Experimental unit and profile

One independent observation is a map seed. Each map is replayed from seats 0
and 7 under control and treatment, producing four games while keeping the two
mirrored seats inside one map-level observation.

The fixed profile is:

- 8 major civilizations, randomized independently by map seed;
- 84x54 requested Planet map (105x44 stored), Continents, Poles;
- 12 city-states;
- Online speed, 250-turn horizon;
- Science, Culture, and Domination victories enabled; Religious, Diplomatic,
  and Score victories disabled;
- stock `AdvancedAi` for every seat; and
- fog memory disabled equally in both arms, matching the existing paired
  evaluators and changing no legal observation used by the controller.

The control and treatment for a seat start from identical options and
controller state. Results are aggregated by map, not by seat-game.

## Measurements

For each arm report games, wins and victory types, mean reported turn, score,
terminal Faith, city count, and completed district count.

For the focal mechanism report:

- focal turns replayed;
- stock district-production choices;
- exact legal Faith matches;
- reserve-eligible opportunities;
- seat-games in which the treatment fired;
- purchases by district family;
- total and mean Faith spent; and
- total and mean base Production forfeited at the purchase-city turn
  boundary.

For paired inference report:

- helped, hurt, and unchanged matched seat cells, descriptively;
- map win score, where equal map wins score 0.5 and each net treated-seat win
  moves the score by 0.25;
- favorable, neutral, and adverse maps plus an exact two-sided sign-test
  p-value over directional maps;
- a conservative 95% Wilson interval for the map win score; and
- paired terminal-score share and its map-level direction/sign test.

## Exact replay sanity

Before reading treatment data, run a null replay in which both arms use the
same stock trace with substitution disabled:

```sh
target/release/faith_district_eval \
  --null --maps 6 --players 8 --width 84 --height 54 --city-states 12 \
  --turns 250 --speed online --map continents --shape planet --poles poles \
  --randomize-civs --victories science,culture,domination \
  --seed 9995000 --jobs 6
```

All 12 matched seat replays must reproduce exactly across every recorded
`GameResult` field. Any mismatch stops the study. The null command may begin
only when every older simulator owner in the shared queue has released its
cores.

## Development screen

Only after the exact null passes, run:

```sh
target/release/faith_district_eval \
  --maps 30 --players 8 --width 84 --height 54 --city-states 12 \
  --turns 250 --speed online --map continents --shape planet --poles poles \
  --randomize-civs --victories science,culture,domination \
  --seed 9996000 --jobs 6
```

The 30-map screen advances only if every term holds:

1. the treatment fires in at least 10% of its 60 focal seat-games;
2. at least 10 exact Faith district purchases occur;
3. paired map win score is at least 52.5%;
4. favorable maps outnumber adverse maps; and
5. paired terminal-score share is at least 50.0%.

If any term fails, stop. Do not tune the reserve, Culture exclusion,
candidate order, map count, seed, or gate, and do not retry this policy under
a new name.

## Disjoint holdout

Only a passing screen licenses this fixed holdout:

```sh
target/release/faith_district_eval \
  --maps 120 --players 8 --width 84 --height 54 --city-states 12 \
  --turns 250 --speed online --map continents --shape planet --poles poles \
  --randomize-civs --victories science,culture,domination \
  --seed 9997000 --jobs 6
```

The holdout advances only if every term holds:

1. the treatment fires in at least 10% of its 240 focal seat-games;
2. at least 40 exact Faith district purchases occur;
3. favorable maps outnumber adverse maps;
4. the exact two-sided map sign-test p-value is below 0.05;
5. paired map win score is above 50.0%; and
6. paired terminal-score share is at least 50.0%.

A pass permits a separate gameplay PR implementing this same stock-selected,
reserve-protected, once-per-turn purchase before district production, with the
city allowed to choose its next item normally. It does not itself change
`AdvancedAi`. A holdout failure retains shipped gameplay unchanged.

## Queue and stopping rule

This evaluator owns only `src/bin/faith_district_eval.rs` and this document.
It does not touch the currently claimed AI, strategic, game, oracle, or shared
evaluation files. Its exact null and focal runs are queued behind the older
Strategic Expansion, Spaceport, horizon, Royal Society, idle-reserve,
lane/doctrine, repair-routing, and timed-war work in their already-frozen
order. Compilation and deterministic unit tests are allowed while those jobs
run; simulator execution is not.

No result from seeds 9995000, 9996000, or 9997000 may be inspected before the
implementation, tests, commands, and gates above are committed. There is one
screen and at most one holdout. Failed gates close this exact policy.
