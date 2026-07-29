# Adaptive Science Spaceport parallelism

Status: **preregistered; no treatment result has been read**.

## Prospective horizon amendment

This amendment was frozen before running the null at seed 9,982,000 or either
treatment seed. A concurrent, independently preregistered deployment-horizon
audit (#570) identified a mismatch in the phrase “320-turn limit”: production
constructs an Online game with `Game.max_turns = 250`, disables Score, and
continues the outer server loop until an enabled victory, often after turn 250.
Constructing this evaluator with `Game.max_turns = 320` would instead change
policy-visible expansion deadlines, payback calculations, and late production
values.

The fixed profile therefore preserves `Game.max_turns = 250` and observes the
unchanged game and controllers externally through turn 320. The runner must
assert that the nominal value remains 250 throughout continuation. This is a
prospective external-validity correction based only on production configuration
and code, not on a focal result. The treatment, map unit, seats, seeds, sample
sizes, endpoints, thresholds, stop rules, and resource cap below are unchanged.
The only earlier runtime treatment smoke used the diagnostic seed 9,982,999 and
is not part of any frozen batch.

## Superseded prospective deployment-topology amendment

This second amendment was frozen before running the null or reading any focal
seed. The production supervisor's independently merged shape-redraw fix (#543)
made topology an explicit deployment mixture rather than a stable Planet-only
cell. A read-only census of the latest 50 completed production saves through
`20260729T140735.561985Z` found 27 Flat and 23 Planet worlds; all 50 retained
the eight-player Continents, Online, Poles, and Science/Culture/Domination
profile below. The supervisor samples uniformly from `flat,planet`, so the
near-even archive split is expected deployment behavior, not a treatment
result.

The frozen batches therefore balance topology deterministically inside the
existing map sample: a zero-based even map offset uses Flat and an odd offset
uses Planet. The four-map null contains two maps of each shape, the 30-map
screen contains 15 of each, and the 120-map holdout contains 60 of each. Every
control/treatment replay for one map retains that map's shape. Requested size
remains 84x54: Flat stores 84x54, while Planet stores the engine's corresponding
105x44 globe. This removes random shape imbalance without changing the map as
the inference unit.

The runner will report map counts and descriptive mechanism/outcome summaries
for each topology so a shape-specific failure stays visible. Only the pooled
30-map or 120-map gate controls the decision; topology-specific results cannot
promote, extend, or rescue the treatment.

This is a prospective external-validity correction based only on the deployed
supervisor contract and archived control saves. The treatment, seats, seed
ranges, sample sizes, endpoints, thresholds, stop rules, and resource cap are
unchanged. In particular, no extra maps are added and neither shape may rescue
a failing pooled gate after results are read.

This topology-only design was itself superseded prospectively, before any focal
seed was read, by the deployment-population amendment below. It is retained as
an audit trail: the 50-save census was accurate, but those saves came from an
open viewer repeatedly handing an explicit eight-player Continents setup to the
next game. That setup-panel handoff correctly overrides the supervisor's random
draw and therefore described one viewer-selected operational stratum, not the
unattended deployment population.

## Prospective deployment-population amendment

This third amendment was frozen after the topology implementation but before
the null at seed 9,982,000 or either treatment seed was run or read. After the
viewer disconnected, the next automatically rolled production world started
with nine players on True Start Earth/Flat. Inspection of the already-merged
supervisor contract (#543) confirmed that every unattended rollover after a
completed world independently draws three axes uniformly:

- player count from 4, 5, 6, 7, 8, 9, and 10;
- map script from Land Only, Lakes, Inland Sea, Pangaea, Continents, Small
  Continents, Islands, Water World, and True Start Earth; and
- topology from Flat and Planet.

The first world after a cold supervisor launch still uses the operator's
startup flags because no prior world exists to trigger a rollover. It is a
separate, rare startup stratum rather than one of these draws. This experiment
targets the repeated rollover population that the supervisor explicitly varies;
it does not estimate an uptime-dependent mixture of cold starts and rollovers.

The previous fixed eight-player Continents design would therefore estimate the
treatment in only one of 126 equally likely rollover profiles. The frozen
batches now use a deterministic space-filling cycle over all 126 profiles. For
zero-based map offset `i`, the axes are:

- players: `[4, 6, 8, 10, 5, 7, 9][i mod 7]`;
- script: `[land_only, water_world, continents, true_start_earth, lakes,
  inland_sea, pangaea, small_continents, islands][i mod 9]`; and
- topology: Flat at even `i`, Planet at odd `i`.

Because 7, 9, and 2 are pairwise coprime, every joint profile appears exactly
once before the schedule repeats at offset 126. Axis ordering changes no
weight: it makes the four-map replay null deliberately span all four deployed
map-size rows and exercise land-heavy, water-heavy, ordinary rolled, and fixed
geography. Those null maps are 4-player Land Only/Flat, 6-player Water
World/Planet, 8-player Continents/Flat, and 10-player True Start Earth/Planet.

Player count derives requested dimensions and city-state count through the same
`MapSize::for_players` table as `civvis play`: 4 players request 60x38 with 6
city-states; 5--6 request 74x46 with 9; 7--8 request 84x54 with 12; and 9--10
request 96x60 with 15. Planet then stores the corresponding 75x32, 90x38,
105x44, or 120x50 globe; Flat retains the requested rectangle. No evaluator
copy of the size table is permitted.

Each phase restarts at offset zero and uses disjoint frozen game seeds. The
30-map screen has 15 maps per topology, 4 or 5 maps per player count, and 3 or
4 maps per script. The 120-map holdout has 60 maps per topology, 17 or 18 per
player count, and 13 or 14 per script; it covers 120 distinct cells of the
126-cell factorial. Every control/treatment replay for a map shares the exact
profile and seed. The map remains the inference unit.

The runner will report descriptive mechanism and outcome summaries separately
by player count, map script, and topology. Only the pooled 30-map or 120-map
gate controls the decision; no axis-specific result may promote, extend, or
rescue the treatment. This amendment changes only the prospectively sampled
deployment population. The treatment, seed ranges, map counts, endpoints,
thresholds, stop rules, observation horizon, and resource cap are unchanged.
The only intentional runtime smoke remains diagnostic seed 9,982,999. The
execution audit below separately records a terminated stale-binary partial null
startup that produced no completed map or result.

## Prospective champion-controller amendment

This fourth amendment was frozen before the null at seed 9,982,000 or either
treatment seed was run or read. A read-only deployment audit found that the
supervisor is launched with `--league ... --league-record`. Under that contract,
`Session::ai_fleet` seats the active rating roster; it does not construct
`AdvancedAi::new()` for every major. The live seed 5,222,428 at the audit point
contained six adaptive evolved-genome seats, one `advanced_v1` seat, and one
stock `advanced` seat. The evaluator, however, constructed only the stock
default-weight fleet. Map sampling had been brought into line with deployment
while the focal controller had not.

The active league is a moving population whose membership and ratings change
after every recorded game, so copying that incidental eight-seat table would
not define a reproducible treatment target. This study instead fixes the
controller that a successful gameplay change is intended to improve: the
repository's embedded `advanced_evolved` champion. Every major in both arms
uses the exact champion weights compiled from `data/evolved/best.json` at the
tested source commit. City-states and barbarians retain the controller's
default minor/barbarian path. The runner names the controller and embedded
champion generation, and a formal frozen batch requires the explicit
`--ai advanced_evolved` flag.

This is a prospective target-population correction based only on the deployed
supervisor command, the already-public live roster metadata, and static fleet
construction. No treatment output, focal seed, endpoint, or outcome informed
it. The treatment, world-profile schedule, seats, seed ranges, map counts,
endpoints, thresholds, stop rules, observation horizon, and resource cap are
unchanged. The earlier one-turn smoke at diagnostic seed 9,982,999 exercised
the stock-weight replay path and is not evidence for the amended champion
population. After implementation, the same non-focal seed was replayed for one
turn under the champion solely to validate mechanics; the required frozen null
remains unopened and will validate the full-horizon champion replay exactly.

## Prospective fail-closed invocation amendment

This fifth amendment was frozen before the null at seed 9,982,000 or either
treatment seed was run or read. A static command-path audit found that the
runner's convenience parser silently substituted defaults for a missing or
malformed numeric value. Its formal-profile label also required only the
presence of most frozen flags, did not bind `--jobs 6`, and could therefore
classify an omitted, duplicated, or default-substituted option as a registered
batch. No focal output exposed this bookkeeping defect: all three registered
seed ranges remain unopened.

The parser must now exit 2 before constructing a game whenever a supplied
numeric or text option lacks a usable value. Formal recognition is stricter
than diagnostic execution: every frozen option must occur exactly once with
its raw preregistered value. The common options are `--deployment-mix`,
`--ai advanced_evolved`, `--turns 250`, `--observe-through 320`,
`--speed online`, `--poles poles`, `--randomize-civs`,
`--victories science,culture,domination`, and `--jobs 6`. Each phase must also
bind its exact `--maps` and `--seed`; the null must contain `--null` exactly
once, while both treatment phases must omit it. Defaults and duplicated flags
remain available only for clearly labelled diagnostics and cannot receive a
frozen null, development, or holdout decision label.

This is a prospective integrity correction derived solely from source and the
already-frozen command contract. It changes no controller, treatment, world
profile, seat, seed, sample size, endpoint, threshold, stop rule, observation
horizon, or resource cap.

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

`science_parallelism_eval` will run the embedded `advanced_evolved` adaptive
champion in both arms. On a focal treatment turn it will clone the controller,
run the stock turn, replay every successful logged action except the final
`EndTurn`, and retain the updated controller state. It may then issue at most
one additional legal `Produce` order, after which it applies the deferred
`EndTurn`.

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

Every independent map is replayed four times: focal seats 0 and the final major
seat, each under control and treatment. All other seats use stock `AdvancedAi`.
The inference unit is the map; the two focal seats are averaged within a map
and are never counted as independent samples.

The fixed deployment population is:

- the deterministic 7-player-count × 9-script × 2-topology schedule above,
  with randomized civilizations and size/city-state defaults derived from each
  player count;
- Poles on every map;
- Online speed, policy-visible 250-turn limit, externally observed through turn
  320 without changing `Game.max_turns`;
- Science, Culture, and Domination victories; and
- stock embedded rules and the embedded `advanced_evolved` champion from the
  exact tested commit.

Before any treatment batch, a four-map null at seed **9,982,000** (the four
cross-size profiles named above) must compare a direct untreated champion
control with the action-log replay arm while applying no added order. All eight
matched focal cells must be exactly equal. Unit tests must also prove the
deterministic 126-profile schedule and frozen batch balances, deterministic site selection,
the Moon/Mars 2/3 schedule, no action outside a Science plan, and at most one
legal order.

The one allowed development screen is **30 maps starting at seed 9,983,000**
(120 games). If and only if its frozen gate passes, the one allowed holdout is
**120 maps starting at seed 9,984,000** (480 games). Both use the deployment
schedule and balances above. No seed may be replaced, extended, or retried; no
threshold or treatment detail may change after a treatment result is read.
Compile and one-map runtime smokes are diagnostic only and may not be used to
tune the policy or gate.

The exact frozen invocations must include `--ai advanced_evolved`,
`--deployment-mix`, `--turns 250`, `--observe-through 320`, `--speed online`,
`--poles poles`, `--randomize-civs`, and
`--victories science,culture,domination`, plus the phase's frozen map count and
seed. Supplying a player count, dimensions, city-state count, script, or shape
alongside `--deployment-mix` is an error, not a new profile. Any other
controller name is rejected rather than treated as a new experiment.

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

## Validation and execution audit

Commit `d3226f3` implemented the deployment-population amendment after its
independent preregistration commit `0d643d3`:

- `cargo test --release --locked --bin science_parallelism_eval -j 2` passed
  all 7 focused tests, including 126 unique joint profiles, the exact null
  profiles, and the frozen 30/120-map marginal counts;
- a rebuilt standalone binary rejected `--deployment-mix --players 8` with
  exit 2 before simulation; and
- a four-map, one-turn replay diagnostic at seed 9,982,999 used two jobs and
  exercised the four null profiles under the then-frozen stock-weight
  controller. It reported the expected derived sizes and axis counts and
  reproduced all 8 direct/replay focal cells exactly. The prospective
  champion-controller amendment above supersedes that controller population,
  so this smoke is diagnostic history rather than champion-null evidence.

The champion-controller design was frozen independently at `2a14f26` and then
implemented at `e7058a1960fdeedbef94ec2d0e85bbe16411437d`:

- `cargo test --release --locked --bin science_parallelism_eval -j 1` passed
  all 8 focused tests, including exact loading/application of the committed
  champion weights;
- the rebuilt standalone binary rejected `--ai advanced` with exit 2 before
  constructing a game; and
- a four-map, one-turn null diagnostic at seed 9,982,999 used one job, reported
  `advanced_evolved` embedded champion generation 14, exercised all four
  deployment-null profiles, and reproduced all 8 direct/replay focal cells
  exactly. It was not a focal batch and cannot spend a gate.

The fail-closed invocation design was frozen independently at `3588829` and
then implemented at `a55f555d17f3ceb453ec37eac6d9165c75b60e4a`:

- `cargo test --release --locked --bin science_parallelism_eval -j 1` passed
  all 10 focused tests, including missing/malformed value rejection, canonical
  one-occurrence formal flags, the deployment schedule, champion binding, the
  policy horizon, and treatment mechanics;
- the rebuilt standalone binary rejected a malformed `--turns nope`, a
  valueless `--speed`, `--ai advanced`, and
  `--deployment-mix --players 8` with exit 2 before simulation; and
- a one-map, one-turn null diagnostic at the pre-existing non-focal seed
  9,982,999 used one job, selected the first deployment profile, named the
  embedded generation-14 champion, reproduced both direct/replay focal cells
  exactly, and emitted only the diagnostic null label. It cannot spend a gate.

Execution audit: immediately before the standalone rebuild, an intended
conflict check invoked the stale pre-amendment release executable. That binary
ignored `--deployment-mix`, accepted `--players 8`, and began its default null
at seed 9,982,000. Its exact process was terminated after 26 seconds. It printed
only the static profile header: no map completed, no progress or result line
appeared, and no focal outcome was read. This happened after the final design
was frozen and did not inform any code, treatment, endpoint, threshold, or
sample choice. The frozen four-map null has not yet completed or been read; no
treatment seed has been touched.

## Resource rule

The large-map evaluator must not begin while another simulator batch is using
six or more cores. It will use no more than six jobs, leaving capacity for the
production spectator, builds, and collaborators. Validation and null runs may
use fewer jobs. The exact source commit, commands, wall time, and results will
be recorded here before this study is shipped.
