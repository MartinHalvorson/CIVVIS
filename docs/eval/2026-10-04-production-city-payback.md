# Early city production payback

Status: rejected after pilot and fresh confirmation. Runtime policy and tests restored to the control; the previously retained production research remains unchanged.

## Hypothesis and frozen protocol

The existing named-lane production reservation pays for legal industrial
buildings but cannot start an Industrial Zone. Test a Domination-only
reservation that starts one district-plus-Workshop chain at a time, only
when its full remaining costs can repay before the game clock. Include
build time and subtract production from a worked plot being replaced.
Keep existing commitments and earlier emergency, growth, research and
solvency reservations. Require at least two cities, maintenance coverage,
and at least one fielded-or-queued military unit per city.

Control: unmodified source at the launcher checkpoint on main cbc4cf533.
Pilot: four Emperor seeds 61005800–61005803 and four Deity seeds
61005900–61005903. No tuning after observing these games. If a useful
early ramp survives without material science/culture or survival loss,
confirm on twelve fresh Emperor seeds 61005600–61005611 and eight fresh
Deity seeds 61005700–61005707; those reserved seeds were not run for the
previous experiment.

Native configuration: four majors, 60×38 Pangaea, six city states, Online
speed, all victory conditions, 150-turn cap, Gran Colombia focal seat with
Domination target and applied gene ledger; random fleet rivals, only the
focal seat exempt from the difficulty handicap. Capture every final
outcome and explicit missing checkpoints. These are native simulations,
not controlled Firaxis A/B games.

## Recorded Firaxis observations

`-host-observations.json` extracts the first complete state at turns 75,
100, 125 and 150 from the archived continuation of run
`civvis-20261004T070716Z`. Another writer controls that game; this task
only reads the recording. This private pinned controller differs from
public main, so it is diagnostic context, not validation of this patch.

At turn 75, five of six cities had housing growth multipliers below one.
At turn 100, all eight cities lacked a completed Industrial Zone; four
cities quoted a legal district at 102 production, requiring 7–17 turns.
Production was 75.70312 versus Scotland’s public total of 111.992 across
six cities. Own city sources contained Palace production, worked plots,
a domestic route, and amenity penalties, but no industrial contribution.
This establishes an unbuilt opportunity, not that building industry
causes a better result. Growth and terrain also limit the ramp.

At turn 150, Civvis had nine cities, 53 population and 115.406 production;
Scotland had seven cities, 63 population and 230.188 production. There
were still no Industrial Zones or Workshops in the recorded Civvis cities.
At turn 100, own production per citizen was 2.61, Scotland 3.39, Japan
2.48 and Brazil 1.83. The early per-city gap therefore also reflects
population density; industry alone is not established as the cause.

The prototype requires nonnegative city amenities, net district production
of at least one, and no pending industrial district or building elsewhere.
It credits production only after both stages finish, uses the current
district quote when available, models future Workshop cost, and does not
spend city overflow twice. Population relocation, future modifiers and
food lost from district placement are not fully forecast; paired games
measure the consequences instead of treating this estimate as realized
production. Existing profitable industrial buildings retain priority and
other named lanes keep their previous district behavior.

## Pilot results and confirmation decision

All eight pairs reached turn 75. Production, science and culture were
unchanged there. At turn 100, matched Emperor production rose 6.99%
(n=3; one world ended at turn 94), science 14.29%, and culture 4.16%.
Deity production rose 5.36% (n=4), science fell 0.47%, and culture rose
0.78%. No focal seat won or was eliminated in either arm.

Late outcomes are mixed and conditional on reaching those checkpoints.
At turn 150, Emperor production fell 3.30% and culture 15.67% (n=3);
Deity production rose 9.53%, science fell 3.11%, and culture rose 19.60%
(n=3). This is not a promotion result. The unchanged runtime is now
being evaluated on the reserved fresh confirmation block to resolve the
late tradeoff and check the turn-100 signal before any integration.

## Fresh confirmation and decision

The twelve Emperor and eight Deity confirmation pairs kept the exact
frozen runtime 086cb4dd2; no policy tuning followed the pilot. All twenty
pairs reached turn 75.

| Difficulty | Turn | Matched pairs | Production | Cumulative observed production | Science | Culture |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Emperor | 75 | 12 | +0.53% | +0.04% | −0.67% | 0.00% |
| Emperor | 100 | 11 | +2.57% | +1.12% | +1.13% | +0.14% |
| Emperor | 125 | 8 | −6.32% | +0.26% | −2.14% | −0.81% |
| Emperor | 150 | 7 | +4.32% | +1.18% | +11.57% | +3.52% |
| Deity | 75 | 8 | 0.00% | 0.00% | 0.00% | 0.00% |
| Deity | 100 | 6 | +4.37% | +1.12% | +4.56% | −1.78% |
| Deity | 125 | 5 | +9.75% | +1.19% | +7.54% | +9.71% |
| Deity | 150 | 5 | +11.78% | +3.75% | +11.07% | +5.05% |

The late Deity increases are conditional on five surviving game timelines,
not evidence of an early catch-up. No focal seat won. All Emperor focal
seats survived; one Deity focal seat was eliminated in each arm. The
summary retains every final outcome and explicitly lists missing seeds
at every checkpoint. At turn 100, production increased in three Emperor
pairs, fell in one, and was identical in seven; Deity had one higher,
one lower and four identical pairs.

The mean own/strongest-rival production ratio at turn 100 moved only
0.419→0.424 on Emperor and 0.182→0.191 on Deity. The policy is not a
competitive early-production solution. It buys some additional industry,
but changes too few early states, has a mixed later Emperor ramp, and
leaves a large high-difficulty gap. We restored the control because this small, uneven improvement does not
reliably accelerate the early ramp.

## Read-only host replay and opportunity check

The cold single-state replay proposed identical orders in both arms. That
was an incomplete diagnostic: a new AI had no opening-book history. A
second replay requests turns 1–100 through the persistent `--serve
--fresh-board` interface over the original run through turn 61 and its
continuation through the first turn-100 state. Warm proposed order lists
differ at turns 81, 82, 99 and 100. The candidate journal proposes an
Industrial Zone in Cuenca at turn 81, Guayaquil at 98, and Caracas at
99–100. These proposals were not actuated, and recorded subsequent states
are outcomes of the other controller, not outcomes of these proposals.
This proves historical branch reachability, not host acceptance or strength.

The `-host-sites.rs` diagnostic rebuilds the recorded turn-100 board and
prints legal Industrial Zone sites and net current production after
subtracting a worked plot's yield. Bogotá, Guayaquil, Caracas and Popayán
have legal sites with best net production gains of 2, 3, 3 and 1. Other
cities have no legal sites in the reconstruction, and the two newest
cities also have amenity deficits. This supports a real opportunity but
does not identify the best allocation of construction.

## Validation and reproduction

The frozen runtime passed all CI checks, including changed-line Rust
quality and the paired-cost check. The prototype full local suite passes
4,545 tests with 53 ignored, including three new tests for actual queue
placement, the complete two-stage cost, and serial investment without
blocking the owed Workshop. Runtime code and tests are restored afterward.
The restored control passes all 4,542 tests with 53 ignored. Final
changed-line Rust quality also passes; both summary reproductions and
all artifact hashes are verified.

Build the control at 2146d93a2 and candidate at 086cb4dd2 in separate
checkouts with `cargo build --profile ci --locked --lib`, then link the
archived probe against each library using `rustc --edition=2021 -O`,
`--extern civvis=target/ci/libcivvis.rlib`, `-L dependency=target/ci/deps`,
and the native mimalloc output directory for that build. The probe takes
`START_SEED GAME_COUNT DIFFICULTY`. Recompute all paired evidence with:

```sh
python3 docs/eval/2026-10-04-production-city-payback-summarize.py pilot
python3 docs/eval/2026-10-04-production-city-payback-summarize.py confirmation
```

These measurements distinguish native simulations from read-only Firaxis
replay. They do not verify AI production parity or a new live-game ramp.
The next hypothesis should address the earlier bottlenecks: housing
limited five of six recorded cities at turn 75, while seven of eight
pilot native games had not unlocked Apprenticeship by that checkpoint.
Industry must be accessible and the city must be able to grow before
reserving a late chain can produce a fast, broad gain.
