# Deliberate Builder repair routing

Status: **preregistered; no focal seed has been run or read**.

## Prospective candidate-legality clarification

This implementation clarification was frozen after the original preregistration
but before running or reading any focal seed. A damaged tile already occupied
by one of the focal empire's Builders is left entirely to the stock controller,
as is that Builder for the current treatment step. Such a tile is not a remote
routing problem, and counting its inevitable stock repair as treatment exposure
would contaminate the mechanism endpoint.

A remote Builder/target pair is eligible only if the Builder has at least one
currently legal adjacent move that strictly reduces wrapped distance to the
target. This operationalizes the preregistered promise that an obstacle can
cause a target to be skipped and that the treatment receives no privileged
pathfinder. It does not change resource tiers, pair ordering, action limits,
seeds, endpoints, gates, or resource priority. The implementation already had
both restrictions before any runtime diagnostic; this amendment makes them
part of the written frozen contract.

The frozen `Q` endpoint means exactly what the original endpoint section says:
a pillaged improvement on a resource-bearing tile. A post-implementation audit
found that 238 of the exploratory sample's 247 such tiles satisfy the engine's
current resource-connection predicate; nine use alternate water improvements
(Offshore Oil Rig or Fishing Boats) that the single stock-improvement predicate
does not recognize. Output therefore says **resource-bearing improvements**,
not “resource connections.” This is a label correction before focal data, not
a changed count, treatment priority, endpoint, or gate.

## Prospective champion-controller amendment

This controller-population amendment was frozen before the null at seed
9,995,000 or either treatment seed was run or read. A read-only deployment
audit established that the unattended supervisor is launched with
`--league ... --league-record`. Under that contract, `Session::ai_fleet`
seats the active rating roster rather than constructing `AdvancedAi::new()`
for every major. The live seed 5,222,428 at the audit point contained six
adaptive evolved-genome seats, one `advanced_v1` seat, and one stock
`advanced` seat. The evaluator, despite targeting that production repair debt,
constructed only the stock default-weight fleet.

The live league changes membership and ratings after recorded games and is not
a reproducible causal target. This experiment therefore fixes both arms to the
repository's embedded `advanced_evolved` champion: the stable world-class
controller a successful repair policy is intended to improve. Every major
uses the exact champion weights compiled from `data/evolved/best.json` at the
tested source commit. City-states and barbarians retain the default
minor/barbarian path. The runner reports the controller and embedded champion
generation, rejects every other controller name, and emits a formal frozen
verdict only when `--ai advanced_evolved` is explicit.

This amendment follows only from the deployed supervisor command, public live
roster metadata, and static fleet construction. No focal seed, repair outcome,
or treatment output informed it. The repair policy, deployment-world schedule,
seats, seeds, sample sizes, endpoints, gates, stop rules, horizon, and resource
cap are unchanged. Earlier one-turn diagnostics exercised stock weights and
remain audit history rather than champion evidence; the required frozen null
is still unopened.

## Observation and causal diagnosis

This study began with an exploratory census of the 20 completed production
final saves from `20260729T130510.846199Z` through
`20260729T150656.019296Z`. The sample is observational and was read before this
preregistration, so none of its values is a confirmatory endpoint.

Across 149 surviving major-civilization seat-games, 124 ended with at least
one owned pillaged improvement. Of those, 116 also had at least one living
Builder. The final states contained 600 Builders with 1,323 unused charges and
622 pillaged owned improvements; only five Builders stood on one of their
owner's repairable tiles. Of the damaged improvements, 247 carried a resource:
111 strategic, 111 luxury, and 25 bonus. The same saves also contained 710
pillaged buildings and 83 pillaged districts, but city production repairs are
outside this treatment.

The shipped controller supplies a precise mechanism for the improvement debt.
Both `BasicAi::builder_step` and `AdvancedAi::advanced_builder_step` repair a
pillaged improvement when a Builder already occupies its tile. Their remote
target searches, however, consider only `valid_improvements` or
`worthwhile_improvements`. The engine excludes a tile's existing improvement
from `valid_improvements`, including when that improvement is pillaged. An
ordinary remote repair site therefore never becomes a deliberate Builder
target. A Builder can repair it only by coincidence, or by first selecting the
same tile for a different upgrade.

Repairs are legal Builder actions, consume the rest of the unit's movement,
and consume no Builder charge. Pillage disables the improvement's yields and
any resource connection that improvement supplies. This makes one deliberately
routed repair crew a narrow, honest policy candidate rather than a grant or
rules change.

**Frozen hypothesis:** reserving at most one focal Builder turn at a time for
legal, resource-first routing to an owned pillaged improvement will reduce
terminal repair debt and improve the focal civilization's paired terminal
score. The lost opportunity to improve a new tile, contribute to a project, or
avoid danger may instead make the treatment harmful, so victory and score
guards are binding.

## Treatment

`repair_recovery_eval` will run the embedded `advanced_evolved` adaptive
champion in both arms. At the start of each focal treatment turn, before the
untreated champion acts, it may execute one repair-crew order. Every action is
applied through the public game action interface; the evaluator does not mutate
tiles, grant movement or charges, reveal information, alter rules, or change
controller state.

The repair crew follows this frozen policy:

1. If the empire owns a Royal Society and any owned city is currently running
   a non-repair project, defer to the stock controller. This conservatively
   preserves the shipped controller's Builder project-contribution priority.
2. Enumerate living focal Builders with movement remaining and owned tiles
   whose existing improvement is pillaged.
3. Choose one Builder/target pair by resource tier first (strategic, luxury,
   bonus, no resource), then wrapped hex distance, target position, and unit id.
4. If the Builder is already on the target, issue the legal
   `RepairImprovement` action. Otherwise repeatedly choose a legal adjacent
   move that strictly reduces wrapped distance, breaking ties by position,
   until it reaches the target, exhausts its movement, or has no reducing
   step. Repair immediately if it arrives with movement remaining.
5. Run the unchanged stock controller for the remainder of the turn. At most
   one Builder is redirected per focal turn; all other seats and units remain
   stock.

A target behind an obstacle can therefore be skipped rather than receiving a
teleport or bespoke pathfinding advantage. A failed or blocked move hands the
turn back to the stock controller. The intervention records eligible turns,
project deferrals, routed turns, movement steps, completed repairs, and blocked
attempts. No treatment detail may be changed after a focal result is read.

## Deployment population

The experiment targets the unattended production supervisor's rollover
population. That population samples player count uniformly from 4 through 10,
map script uniformly from nine scripts, and topology uniformly from Flat and
Planet. A deterministic 126-profile cycle provides the same weights without
random imbalance. For zero-based map offset `i`:

- players: `[4, 6, 8, 10, 5, 7, 9][i mod 7]`;
- script: `[land_only, water_world, continents, true_start_earth, lakes,
  inland_sea, pangaea, small_continents, islands][i mod 9]`; and
- topology: Flat at even `i`, Planet at odd `i`.

Because 7, 9, and 2 are pairwise coprime, every joint profile appears once per
126 offsets. Player count must derive requested dimensions and city-state count
through `MapSize::for_players`, not an evaluator copy: 4 players request 60x38
with 6 city-states; 5--6 request 74x46 with 9; 7--8 request 84x54 with 12; and
9--10 request 96x60 with 15. Planet realizes the corresponding globe geometry;
Flat retains the requested rectangle.

Each phase restarts at offset zero. The four-map null deliberately spans the
four production size rows: 4-player Land Only/Flat, 6-player Water
World/Planet, 8-player Continents/Flat, and 10-player True Start Earth/Planet.
The 18-map screen has two maps per script, nine per topology, and two or three
per player count. The 63-map holdout has seven maps per script, nine per player
count, 31 or 32 per topology, and 63 distinct joint profiles. Axis-level
reports are descriptive only; no subgroup can promote or rescue the pooled
gate.

Every world uses randomized civilizations, Poles, Online speed, Science,
Culture, and Domination victories, a policy-visible `Game.max_turns` of 250,
and external observation through turn 320 without modifying that value. This
matches the production horizon contract already identified by the independent
deployment-horizon study. The evaluator must assert that `max_turns` remains
250 throughout continuation.

## Frozen experiment

Each independent map is played four times: focal seat 0 and the final major
seat, each under untreated champion control and treatment. All non-focal major
seats use the same embedded `advanced_evolved` champion; city-states and
barbarians retain the default minor/barbarian path. The two focal seats are
averaged within a map. The map, not the seat-game, is the inference unit.

Before any treatment batch, a four-map no-op null at seed **9,995,000** must
compare two executions of the same evaluator loop with repair routing disabled.
All eight matched focal cells must reproduce exactly. Unit tests must also
prove the 126 unique deployment profiles and phase balances, resource-tier and
tie ordering, no mutation when no legal target exists, the one-Builder limit,
project deferral, legal remote movement, and charge-free repair completion.

The one allowed development screen is **18 maps starting at seed 9,996,000**
(72 games). If and only if every screen gate passes, the one allowed holdout is
**63 maps starting at seed 9,997,000** (252 games). Seeds may not be replaced,
extended, or retried. Focal outcomes may not tune the treatment, endpoints,
thresholds, or sample sizes. Compile, unit, and non-focal runtime smokes are
diagnostic only.

Frozen invocations must specify `--ai advanced_evolved`, `--deployment-mix`,
`--turns 250`, `--observe-through 320`, `--speed online`, `--poles poles`,
`--randomize-civs`, and `--victories science,culture,domination`, plus the
phase's exact seed, map count, and `--jobs` cap. Supplying an explicit player
count, dimensions, city-state count, script, or shape with `--deployment-mix`
is an error. Any other controller name is rejected rather than treated as a
new experiment.

## Endpoints and gates

For focal seat `s` on map `m`, let `S` be the engine's terminal score and let
`D` and `Q` be the terminal counts of owned pillaged improvements and damaged
resource-carrying improvements. Define the primary map-level outcome

`d_m = mean_s(S_treatment - S_control)`.

The evaluator reports the mean `d_m`, favorable/neutral/adverse map counts, and
an exact two-sided sign test over non-neutral maps. Mechanism reports include
all treatment-census fields, aggregate and paired terminal `D` and `Q`, living
Builders and charges, and base yields represented by the damaged improvements.
Harm guards report total and victory-type focal wins, paired map win score,
paired terminal-score share, final cities, and completion turn.

The 18-map screen passes only if every term holds:

- at least 18 of the 36 focal treatment games complete a deliberate repair and
  at least 36 deliberate repairs complete in aggregate;
- aggregate terminal `D` and `Q` are each at most 85% of control;
- mean `d_m` is positive, favorable maps outnumber adverse maps, and the exact
  two-sided sign-test p-value is at most 0.20;
- treatment loses no more than one total focal win relative to control, paired
  map win score is at least 48%, and paired terminal-score share is at least
  49.5%; and
- treatment's aggregate surviving-city count is at least 98% of control.

Failure means **STOP**: retain the shipped controller, do not tune or retry,
and do not inspect the holdout.

The holdout passes only if every term holds:

- at least half of focal treatment games complete a deliberate repair and the
  aggregate completion count is at least the number of focal games;
- aggregate terminal `D` and `Q` are each at most 85% of control;
- mean `d_m` is positive, favorable maps outnumber adverse maps, and the exact
  two-sided sign-test p-value is below 0.05;
- treatment has at least as many total focal wins as control, paired map win
  score is at least 50%, and paired terminal-score share is at least 50%; and
- treatment's aggregate surviving-city count is at least control.

Passing permits a separate gameplay-integration PR with normal promotion
tests; this evaluator cannot promote the policy by itself. Failure retains the
controller and closes the candidate without post-hoc rescue.

## Validation and execution audit

Preregistration commit `deb1475` was pushed before implementation commit
`7ac1e46`. No frozen seed had been run or read when either commit was made.

- `cargo test --release --locked --bin repair_recovery_eval -j 1` passed all
  ten focused tests, including stock-covered target exclusion and stable
  equal-tier/equal-distance selection.
- `cargo clippy --locked --bin repair_recovery_eval -j 1` emitted no warning
  in the new evaluator; the repository's existing warnings remain outside this
  claim.
- The rebuilt release binary rejected `--deployment-mix --players 8` with exit
  2 before constructing a world.
- A one-map, one-turn fixed-cell null at diagnostic seed 79,991 reproduced both
  matched focal cells exactly under the then-frozen stock-weight controller.
- A one-map, one-turn fixed-cell treatment-loop smoke at diagnostic seed 79,992
  completed under stock weights with no eligible damage and no intervention.
  The champion-controller amendment supersedes that controller population, so
  neither diagnostic is champion evidence or can alter the frozen design or
  gate.

An initial attempt to invoke the standalone executable after `cargo test`
failed immediately with exit 127 because a test harness does not create
`target/release/repair_recovery_eval`. `cargo build --release` then created the
binary used for the diagnostics above. No simulation or seed was touched by
the failed invocation. The null seed 9,995,000 and both treatment ranges remain
untouched.

## Resource rule

The evaluator must not begin a large-map batch while another simulator job is
using six or more cores. It will use no more than six jobs, leaving capacity
for production, builds, and collaborators. Its null and treatment batches are
queued behind the already-preregistered simulator work. The exact source
commit, commands, wall time, and results will be recorded here before shipping.
