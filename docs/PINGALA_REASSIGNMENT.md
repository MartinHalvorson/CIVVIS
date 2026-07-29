# Preregistered Pingala reassignment evaluation

Status: frozen before evaluator implementation and before any registered seed
is run. This document describes an evaluator only; it does not change the
engine or a shipped AI policy.

## Question

Advanced AI chooses Pingala's city when it spends the appointment title, but
does not revisit that economic placement after its cities grow, specialize,
or are captured. Basic AI can later move a Governor into a Loyalty emergency;
that is a separate safety policy. This study asks whether periodically applying
the **same stock Pingala city score** to economically safe cities improves the
focal empire enough to repay five turns of re-establishment.

The frozen hypothesis is:

> After all stock actions, relocating an established Pingala from a fully loyal
> city to an unoccupied, fully loyal city only when the stock score improves by
> at least 180 points and 25% will improve terminal strategic strength without
> harming wins.

The 180-point and 25% requirements, cadence, cooldown, seeds, sample sizes, and
gates below are fixed before the treatment controller exists. A failure retires
this policy; it does not authorize tuning on the same seeds.

## Pre-registered observational evidence

Before this registration, a read-only census loaded the moving latest 50
production endpoints ending at
`20260729T161608.260324Z-seed-416573342-turn-302-instance-39983.save.json`.
The census used a source-identical library checkout, enabled the per-game query
memo, and deserialized all 50 files without failure. The endpoints contained
400 major-seat records, 376 living majors, and 286 domestic Pingala
appointments. For every appointment, it scored the incumbent and every own
city not occupied by another Governor with the exact stock expression:

```text
max(100 - loyalty, 0) * 2 + population * 14
    + city_science * 9 + city_culture * 9
```

Fifteen of 286 appointments (5.2%) had a strictly better city. Their mean score
gap was 193.5 and mean population gap was +3.07. The largest gaps were 606.5
for Maori (population 8 to 22), 599.3 for Indonesia (population 11 to 17), and
280.8 for Poland. There were also several marginal gaps below 120.

The census is cross-sectional and the incumbent's yields can already include
an established Pingala, so it is conservative about moving but cannot estimate
causal value. Winner status, treatment outcomes, and the focal seeds below were
not examined. The evidence establishes a sparse, sometimes large decision
opportunity—not that reassignment helps.

## Existing mechanism and exact treatment

`ReassignGovernor` is already a legal action. It changes the assignment and
resets `assigned_turn`; Pingala's local effects remain inactive for the
speed-scaled equivalent of five standard turns. It spends no title or currency.
The treatment never edits a Governor, city, yield, Loyalty value, rule, or
establishment time directly.

For each focal treatment turn, the evaluator:

1. clones the current `Game` and focal `AdvancedAi`, runs one complete stock
   turn, retains the stock-produced controller state and successful action log,
   then replays every successful stock action in order except the final
   `EndTurn`;
2. considers an extra action only from turn 80 onward, only while at least 20
   external observation turns remain, only when
   `turn % 10 == focal_seat % 10`, and only when 40 turns have elapsed since
   this evaluator last relocated Pingala;
3. requires Pingala to be assigned to a living focal city, fully established,
   and not disabled; requires the source city's Loyalty to be at least 90; and
   requires every candidate to be an own, at-least-90-Loyalty city occupied by
   no other Governor;
4. scores the source and candidates from the post-stock state with the exact
   stock expression printed above, choosing highest score and then lowest city
   id;
5. requires both `target_score - source_score >= 180` and
   `target_score >= 1.25 * source_score`; and
6. applies exactly one ordinary `ReassignGovernor { pingala, target }`, then
   applies the deferred `EndTurn` and resumes ordinary play with the exact
   stock-produced controller state.

The policy does not move an unestablished or neutralized Pingala, does not
replace stock's Loyalty-emergency move, does not evict another Governor, does
not inspect another player's hidden state, and cannot act more than once per
40 turns. Because a stock reassignment resets establishment, step 3 naturally
blocks a second move on that turn. The post-stock insertion also cannot
invalidate a purchase, production choice, promotion, war action, or deal that
stock already made.

The relative gate is a frozen re-establishment/churn guard. Under the minimum
40-turn treatment dwell, a five-turn establishment loss costs five incumbent
score-turns while a 25% gap supplies at least 8.75 incumbent score-turns across
the remaining 35 turns. This is a policy proxy, not an outcome claim; the
registered experiment decides whether the proxy translates to gameplay.

## Exact null and focused contract

Before treatment data, a four-map null at seed `10029999` uses the same wrapper
with the extra reassignment disabled. For both focal seats on all four maps,
ordinary stock and null replay must have identical focal results, census, and
serialized terminal `Game`: eight exact cells or **STOP**.

Focused tests must prove:

- the deployment cycle and profile-override rejection are exact;
- the city score, 180-point absolute gate, 25% relative gate, lowest-id tie
  break, turn-80 floor, cadence, 40-turn cooldown, and final-20-turn exclusion;
- unestablished, disabled, foreign, sub-90-Loyalty, and other-Governor-occupied
  source or target states cannot produce a treatment action;
- a legal relocation uses `ReassignGovernor`, resets establishment through the
  engine, and increments the mechanism census exactly once;
- all successful stock actions and the stateful controller survive null replay,
  and null replay defers only `EndTurn`;
- establishment follow-through counts only the evaluator's recorded target and
  only after Pingala is truly established there; and
- screen and holdout gates reject every individual harm or missing-mechanism
  condition.

## Deployment population and endpoints

The evaluator targets the unattended production rollover population with the
same deterministic 126-profile cycle used by the registered Spaceport,
horizon, recon, repair, and captive-Spy studies. For zero-based map offset `i`:

- players are `[4, 6, 8, 10, 5, 7, 9][i mod 7]`;
- scripts are `[land_only, water_world, continents, true_start_earth, lakes,
  inland_sea, pangaea, small_continents, islands][i mod 9]`; and
- topology is Flat for even `i`, Planet for odd `i`.

`MapSize::for_players` supplies dimensions and city-state counts.
Civilizations are randomized; Poles, Online speed, and
Science/Culture/Domination victories are fixed. `Game.max_turns` remains 250
while unchanged stateful agents are observed externally through turn 320. The
runner must assert that the policy-visible horizon never changes.

Each map is played four times: focal seat 0 and the final major seat, each as
ordinary stock and comparison. Every other major is stock `AdvancedAi`; minors
retain their stock paths. The two focal seats are aggregated within their map,
and the map is the only inference unit.

The evaluator reports:

- cadence checks, eligible opportunities, absolute and relative gate passes,
  successful and failed relocations, source/target scores and populations,
  establishment follow-through, treatment seat-game coverage, and any later
  stock reassignment away from the recorded target;
- terminal Pingala assignment, establishment, city population, city Science,
  city Culture, total focal city Science per turn, total focal city Culture per
  turn, stockpiled Great Person points, and claimed Great People;
- wins and victory types, finish turn, terminal score, cities, technologies,
  civics, Science-project progress, lifetime Culture and Tourism, military
  power, and terminal Loyalty risk; and
- paired map win score, paired terminal-score and Science-progress shares,
  complete favorable/neutral/adverse map directions, and exact two-sided sign
  tests.

## Fixed development screen

After the exact null, the one allowed screen is 36 maps / 144 games:

```text
pingala_reassignment_eval --deployment-mix --maps 36 --turns 250 \
  --observe-through 320 --speed online --poles poles --randomize-civs \
  --victories science,culture,domination --seed 10030000 --jobs 6
```

It advances only if every term holds:

1. treatment completes at least six relocations across at least five of 72
   focal treatment seat-games, with zero failed applications;
2. at least four relocations across at least three seat-games establish on the
   recorded target before a later stock move, capture, or game end;
3. treatment has no more sub-70-Loyalty terminal cities and no more lost focal
   capitals than control;
4. terminal-score favorable map directions outnumber adverse directions, the
   exact two-sided sign-test p-value is at most 0.20, paired terminal-score
   share is at least 50%, and mean treatment score is not lower;
5. paired Science-progress share and mean Science progress are each at least
   control, paired map win score is at least 50%, and treatment has no fewer
   total focal wins; and
6. treatment has no fewer Science, Culture, or Domination wins than control.

Any failed term means **STOP**: retain the stock policy, record the negative or
underexposed result, do not change the score, thresholds, cadence, cooldown,
gate, sample size, or seed, and do not inspect the holdout.

## Disjoint holdout

A complete screen pass earns one unchanged 63-map holdout at seed `10031000`
(252 games). It must retain the full mechanism and safety gates, paired
terminal-score and Science-progress shares of at least 50.5%, positive mean
score and Science-progress differences, more favorable than adverse score maps
with exact two-sided sign-test `p < 0.05`, paired map win score at least 50%,
and no loss of total or per-type wins. Only that conjunction permits a separate
gameplay-integration PR; this evaluator cannot ship the policy.

Undefined ratios pass only when both corresponding counts are zero. No pooled
rescue, seed retry, sample extension, subgroup promotion, or post-result
treatment change is allowed.

## Resource and integration order

The exact null, screen, and any earned holdout use at most six jobs and run
alone in the shared simulator slot. They are queued behind every older active
registered job, including #561, #567, #570, #574, #579, #584, #589, #592,
#593, #597, #598, #599, and #600. Studies that stop or land release their
place; this task never jumps a still-live older batch.

The implementation, latest-main merge, focused checks, and full locked CI suite
must precede the exact null. Exact commands, source commit, wall time, and all
results will be recorded before this evaluator leaves draft.

## Implementation checkpoint

The frozen controller is implemented at source commit `64e35ff` in
`src/bin/pingala_reassignment_eval.rs`. It replays the complete successful stock
action log except deferred `EndTurn`, retains the cloned `AdvancedAi` state,
and inserts only the registered legal reassignment. No library, engine, or
shipped-policy source changed.

The focused contract is 8/8 green:

```text
cargo test --profile ci --locked --bin pingala_reassignment_eval
test result: ok. 8 passed; 0 failed; 0 ignored
```

`rustfmt --edition 2021`, `git diff --check`, and a normal bin-target Clippy
pass are clean for the new evaluator. A global Clippy invocation with
`-D warnings` reaches the library and fails on 295 pre-existing migration
warnings; none names `pingala_reassignment_eval.rs`.

No simulation has used null seed `10029999`, screen seed `10030000`, holdout
seed `10031000`, or a map derived from them. The exact null remains behind the
older registered simulator queue.
