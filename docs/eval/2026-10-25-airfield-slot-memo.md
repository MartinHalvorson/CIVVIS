# Avoid redundant city menus in the airfield-slot scan

The final hoisted candidate passes its own full validation and all fixed-board
output comparisons. Median planner CPU changes are −7.62% on eight native
boards and −8.79% on 42 simulation snapshots; the complete controls, individual
rounds and limits appear in the final results below. Historical registrations
and failed attempts remain in this document with their original source scope.

## Profile observation

A one-second sample of the already-running registered handover screen
(PID 33195, binary `886617e4704f310765b6143797a1c6e7d437feb3a38e1c23ae825d269180f540`)
contains 787 samples on its serial game thread. Of these, 314 are inside
`surge_field_slot_refusals`' district filter and 199 reach
`air_surge_reserves_field_slot`; repeated city-yield and amenity derivation
appears below that call. This is a short, selected profile and does not
measure whole-game cost or a speed improvement. The ongoing screen keeps
its frozen binary and original registration; do not restart it.

The scan enumerates district candidates on one immutable `&Game` board.
Each candidate can rank the same city yields again. The engine's existing
`Game::query_memo` is designed for this scope: nested guards share answers,
and the outermost guard discards them before the board can change. A new
scope after the existing disabled/unfuelled early exit should avoid those
repeated derivations without retaining data across production changes.

## Validation contract

Preserve the existing early exit, candidate order, reservation predicates
and returned refusals. The existing Bomber-wing and field-slot tests cover
the behavior. Run those tests, required full Rust validation and changed-file
quality. Check immutable-scan output equivalence against the parent on
identical representative late-game inputs before interpreting timing.

Measure baseline and candidate built with matching compiler/profile settings,
on fixed inputs with the airfield reservation actually active. Retain source,
binary, input and output digests and interleave the timings. A standard CI
cost check whose gene is off does not measure the activated scan. Report a
changed output as a behavior change rather than a pure optimization. Do not
infer a game-wide gain from the sampled fraction or from a small hot-loop
measurement. The results below record the subsequent functional and cost checks.

## Fixed native-board comparison, registered before timing

Use the first captured decision at each of these fixed cases: native
`civvis-20261025T055158Z` at turns 122, 130, 140 and 150; native
`civvis-20261025T060426Z` at turns 118, 130, 145 and 155. Retain missing,
multi-frontier or inactive cases explicitly, without replacing them after
seeing timings. Reconstruct the captured bytes and preserve their hashes,
native descriptors, actual forced/withheld settings and first state. Before
timing, require an untimed positive reservation journal on both builds.

Run these as fresh, one-shot decider queries over immutable captured inputs.
This compares parent and candidate decisions on the same boards; it does not
reproduce the original persistent planner state or prove replay equivalence
to the native decisions. Do not treat it as a win-rate or whole-game test.

The existing parent `ci` decider from the completed #4119 validation may be
reused only after verifying its Rust/data/build inputs match parent
`a605bd591` and its Cargo fingerprint matches the candidate's compiler,
features, profile, target and rustflags. Freeze its bytes before using it.
Record the original build source honestly rather than relabelling it as a
different revision. If those conditions fail, build the actual parent with
matching settings instead.

Require byte-identical complete replies before interpreting cost. Measure
three same-binary control rounds and five parent/candidate rounds, alternating
arm order for each case and round. Record user CPU, wall time and load; retain
all cases, receipts and divergent replies. Report per-case effects and paired
totals alongside the controls. This is a fixed-board decider measurement and
does not estimate a full-game speedup.

The parent qualification activates the reservation on five of the eight
fixed boards (055158Z turns 122/130 and 060426Z turns 130/145/155).
The other three stay in the fixed comparison as inactive controls. The
initial qualification script looked in `why.log`, but the one-shot CLI
prints reasoning on stderr and the native brain normally writes that to
`why.log`. Its erroneous eight inactive readbacks and complete source/result
are retained before the instrument correction. No case or planner behavior
changed and no speed result was interpreted from that attempt.

The input archive stores the longest captured prefix for each game plus
every selected prefix length and hash. All eight reconstructions were
checked against their original captured bytes before packaging. It also
retains each native decision descriptor and the parent build fingerprint.

The complete cold-query comparison finishes with identical replies on all
eight cases. Its five paired totals are +0.20%, +0.75%, -1.66%, -2.76% and
-0.89% CPU; median -0.89%. Same-binary control totals are -1.17%, +1.18%
and +1.53%, median +1.18%. This does not resolve a speed improvement.

Before a further timing interpretation, use the same eight boards for an
additional planner-only diagnostic. Reconstruct each board once outside the
timing region; create a fresh game clone and controller outside each measured
call; time `AdvancedAi::plan_observed_turn` with the C process CPU clock and
wall time. Retain complete resulting serialized games, action logs and
reasoning, and require exact equality both within and between builds. This
omits native transport/refusal history and is not the native decider itself.
Preserve inactive cases and require explicit reservation exposure.

Build the exact parent library separately with the same `ci` profile and
default features; link the same small probe source against parent and
candidate libraries with matching rustc flags. Register three same-binary
rounds and five comparison rounds, alternating order per case/round, with
five fresh planner calls per process. Keep the earlier inconclusive complete
query result alongside this narrower diagnostic; neither one alone proves
a whole-game speedup.

The planner-only comparison completes with identical serialized games,
action logs, strategic plans and reasoning across all qualifications and
measured calls. Five cases expose the reservation and three remain inactive.
The comparison round totals are +0.50%, -0.30%, -3.56%, -6.15% and +1.38%
process CPU; median -0.30%. Same-binary controls are -4.40%, -0.89% and
+2.01%, median -0.89%. This narrower diagnostic also does not resolve an
overall speed improvement. Keep the optimization in draft while determining
whether the originally profiled simulation shape benefits; do not promote a
selected active-case statistic into a native or whole-game claim.

`measurements.tar.gz` retains both complete result ledgers, canonical replies
and semantics, qualifications, library/compiler fingerprints and probe build
receipts. Its SHA256 is
`138f880a2366ab5f4f77b28a437bbc357c929251de71bf4f11388bfa62ab5d47`.
The exact parent library was rebuilt at a605bd591 with the matching default
features and `ci` profile. The two library fingerprints match on compiler,
features, profile, target and rustflags; both probes use identical source and
rustc flags. Binary and source hashes are in the archived receipts.

All 134 focused air-surge tests pass. The initial full suite fails two server
timing tests (5,907 pass, 2 fail, 52 ignored); both failing tests pass when run
individually with one test thread. The original failures and isolated retries
are archived in `tests-initial.tar.gz`. The complete suite is rerunning with
`RUST_TEST_THREADS=1`, retaining its normal assertions and time limits.

That complete one-thread rerun now passes all six test targets: 6,255 tests
pass, zero fail and 56 are ignored including four doc examples. The complete
log is retained in `tests-serial.log.gz`, uncompressed SHA256
`e1a6e7b62c6113625824764d79d9784942706d2adbf079bb0d81af9e60d3e5cf`.

## Simulation-board follow-up, registered before generation or measurement

Generate two unmodified-parent games from fixed seeds 59002700 and 59002701,
six majors, 74x46 Continents, nine city states, Online, Emperor majors and
Deity barbarians, all six victory conditions and a 250-turn clock. These
seeds come from the beginning of the already selected profiling screen;
this is post-hoc cost investigation, not independent strength evidence.
Freeze the existing `fires-live-genome.json` treatment list, set every
registered treatment explicitly and target all majors at Domination; keep
ordinary `AdvancedAi::new()` minor/barbarian seats. Record unknown gene tags
and stop rather than silently substituting them. This omits the handover
screen's randomized treatment and does not reproduce that experiment.

Save the board immediately before each living major acts at raw turns 50,
100, 150 and 200. Keep all 48 assigned seed/turn/seat cases, including dead
seats, early-ended games and missing or inactive reservations; no replacement.
Stop generation after the last major turn-200 opportunity. Use the public
headless loop's fog-memory and war-ledger settings. Do not alter any live or
registered experiment, and run the two generation jobs sequentially.

These fixed simulation snapshots will qualify fresh-planner semantic equality
and reservation exposure before another cost interpretation. Register the
measurement execution details before timing them. The earlier two unresolved
native-board measurements remain part of the evidence, regardless of what
the larger simulation boards show.

For those simulation snapshots, qualify all available assigned cases with
fresh controllers under the frozen explicit treatment list. Time only
`plan_observed_turn`, excluding file reconstruction, controller construction
and game cloning. Require complete game, new action log, plan and meaningful
reasoning equality within each process and across builds. Use the same
source and compiler flags linked to the two existing matching libraries.
Keep missing cases explicitly. Require positive reservation exposure before
interpreting cost; a failure aborts interpretation without replacing cases.

Run three same-parent-binary control rounds and five parent/candidate rounds,
alternating arm order by assigned case index and round. Each process performs
ten fresh planner calls, records process CPU ticks and wall nanoseconds,
and checks every resulting semantic output. Report all-case totals and the
predefined exposure strata (qualification active/inactive), per-case paired
changes and same-binary controls. A stratum is a cost diagnostic on these
boards, not an estimate of native frequency or full-game speed. Preserve
timeouts, divergences, all raw samples and source/input/binary hashes.

The initial generation attempt was stopped during snapshot output: a selected
one-second sample was entirely inside unbuffered Game-to-file serialization.
Its source, build receipt, all ten completed boards, partial file, log and
termination receipt remain in `sim-instrument-initial.tar.xz`. The writer
now buffers the JSON in memory before writing; the probe also reads its file
into memory before parsing. No cost comparison was interpreted from this
attempt. All ten completed original boards match the buffered rerun byte for
byte; seeds, genome, engine library and all 48 assigned cases are unchanged.
The corrected generator is still completing the registered milestones.

Before running the registered comparison, correct the simulation probe to
call `Game::player_decision_view(pid)` outside timing, as the shared simulator
runtime does (`src/ai/player.rs:127`). `plan_observed_turn` requires a
disposable observation-limited board; the first two-call case-0 smoke check
had passed equality on the full authoritative snapshot and is rejected as
the wrong observation scope. The corrected two-call check passes complete
semantic equality on the player-visible view. Both checks, the old source,
build receipts and correction reason are retained in `sim-projection.tar.xz`.
No cost result was interpreted from the unprojected probe. Generation,
input snapshots, genome and all 48 assigned cases are unchanged. The measured
method remains `plan_observed_turn` on a fresh controller; observation
projection, cloning and construction are excluded from its timing.

The first observed-view comparison stops during parent qualification at case
20 (59002700, turn 200, seat 2): ten calls exceed the registered 120-second
process deadline. No comparison rounds ran. Preserve that failed attempt and
all prior qualification semantics. A one-call diagnosis on the exact board
finishes in 1.64 seconds; a profiled repetition of the exact ten-call command
finishes in 11.23 seconds and passes within-process semantic equality. These
diagnostics do not identify the earlier timeout's cause or support a speed
claim. Register one complete retry with unchanged cases, binaries, ten calls,
three control rounds, five comparison rounds and the same 120-second limit;
use a new output directory and retain the failed attempt separately.

That retry qualifies all 42 available snapshots with matching complete
semantics, retains six missing assignments, then stops at the same 120-second
deadline during the second same-binary round on case 43 (59002701, turn 200,
seat 1). Eighty-one paired control records complete; no candidate-comparison
round runs. `sim-retry-failure.tar.xz` preserves the complete ledger, all
qualification semantics, partial control and failure. No simulation speed
result is inferred. A separate phase-logging diagnostic is being compiled
against the frozen parent to locate the stall without printing huge values
through an equality-assertion panic.

## Hoisted reservation candidate, registered before source edits

Keep every memo-only result and its frozen binaries. The running simulation
batch has one positively journaled reservation among 42 available snapshots
(six of the 48 assigned cases are unreached/dead seats). Fresh controllers
omit the persistent appointment history of the original profile, so this is
limited hotspot coverage; no whole-game gain follows.

The source also enumerates every city's production menu when the invariant
reservation requirement is false, and repeatedly ranks the same cities when
it is true. Extract the existing requirement, city ranking and item-local
checks without changing their predicates. On the immutable refusal scan,
resolve the requirement and selected city once, then enumerate only that
city's menu. There is at most one reserved city under the existing rule;
retain its item order and foundation exclusions, with no fallback city.
Skip ranking when no city is down to its last slot; if the ranked city has
spare capacity, keep the slower cities unreserved. A regression fixture
checks that the slower city's Commercial Hub remains available in that case.
Preserve the disabled/unfuelled return and end the query memo before the
caller mutates production. The ordinary single-item scorer must use those
same predicates and preserve its early cheap rejection of non-districts.

This is a new candidate, not the binary measured in the earlier batches.
Compile it separately, rerun focused/full Rust validation and require complete
reply and planner-state equality on the same fixed native and simulation
cases before cost interpretation. Retain all controls, missing cases and
failures. The earlier inconclusive or failed measurements stay attached to
their original memo-only source/library hashes. Do not attribute their
functional passes or timing to the unbuilt hoisted candidate.

The phase-logging parent diagnosis on case 43 reproduces the 120-second
aggregate deadline: calls 0–3 finish planning, serialization and equality
comparison, then the process is killed while planning call 4. No equality
failure values are emitted. This locates that diagnosis in repeated planner
work, rather than an assertion dump or JSON serialization, without assigning
a cause to the earlier case-20 timeout. The complete trace and build receipt
are retained separately.

For the hoisted candidate, register one fresh planner call per process on
every available assigned snapshot, still with a 120-second process limit.
Keep all 48 assignments, the same three same-parent control rounds and five
comparison rounds, alternate order by case/round, and require exact complete
semantics across all processes and builds. This is a new experiment; it does
not retry or overwrite the ten-call failures. The underlying planner work per
call, observation projection and frozen genome stay the same. Report noise
alongside all-case and exposure-stratum effects. Cost and functional claims
for the hoisted source remain pending until its own binaries and checks pass.


## Completed hoisted-candidate results

All results here use source `d65449ebdd4da414ce9a5197a1302851fb34cf6a`,
production-tree SHA-256 `cd0c550636cdc5211454a1e139c354cefc9516974ac919b3b5878d09d37e2623`.
The frozen library SHA-256 is `06203ba5661b6272f036c9f867cca55588000ca28654c90e2916862f4f94e500`;
the decider SHA-256 is `6d65f0b2cbce7fcd1738d844646cafa536879f06b0c23e703c27400b9f8ffadc`.
The candidate and parent match compiler, features, profile, target and rustflags.
The parent library is built from a605bd591; intervening merged Rust changes
before this build are three CLI test fixtures, not AI or game behavior.

The original hoisted comparison harness was generalized to select the native
or simulation dataset before either planner batch ran. Each fresh process
plans once under the unchanged 120-second limit. Every qualification, three
same-parent control rounds and five paired comparison rounds completed.
All complete native replies and planner semantic outputs match: serialized
resulting board, actions, strategy and reasoning. Native boards are one-shot
captured queries, not a replay of persistent native planner history. Simulation
boards use the same observation projection as the shared runtime.

| Measurement | Cases | Median control CPU change | Median candidate CPU change |
| --- | ---: | ---: | ---: |
| Native cold decider, including startup and parsing | 8 | −0.28% | −1.22% |
| Native-board planner only | 8 | −1.02% | −7.62% |
| Simulation-snapshot planner only | 42 of 48 assigned | +0.11% | −8.79% |

Each number is the median of round-total paired changes; negative means lower
candidate CPU. The cold query uses user CPU; planner probes use process CPU
ticks and exclude parsing, observation projection, cloning and controller
construction. The control and comparison round changes, in execution order:

- Native cold controls: −0.1350%, −0.2811%, −0.9296%; comparisons: −0.9883%,
  −1.2220%, −1.2935%, −1.7923%, −0.9725%.
- Native planner controls: +0.1000%, −3.4235%, −1.0206%; comparisons: −9.2163%,
  −7.6249%, −10.8720%, −3.8601%, −4.9782%.
- Simulation planner controls: +0.1138%, +3.0582%, −0.8893%; comparisons:
  −8.7901%, −11.4825%, **+3.9994%**, −13.5164%, −7.4741%.

The positive simulation round and every per-case sample are retained. Five
of eight native boards and one of 42 simulation boards positively journal the
reservation. In simulation the predefined inactive stratum (41 boards) has
control median +0.0473% and comparison median −8.7714%; the single active
board has control median +6.2133% and comparison median −8.8812%. Its noisy
one-board stratum does not support an active-case population estimate. Six
unreached/dead assignments remain missing and were not replaced. These are
fixed-board fresh-controller costs, not whole-game speed, native frequency,
Domination strength or a new native victory. The code avoids unnecessary
menus even when no reservation is active, explaining why inactive boards
can benefit without changing their decisions.

Local validation on that production tree completes with exit 0 for all five
stages: `cargo check --profile ci --locked --lib`, library and decider builds,
`cargo test --profile ci --locked ai::advanced::air_surge` (135 passed), and
`RUST_TEST_THREADS=1 cargo test --profile ci --locked` (6,256 passed, zero
failed, 56 ignored across all targets, including four ignored doc tests).
Changed-file rustfmt and whitespace checks pass. After the successful library
build, a metadata helper initially expected exactly one Cargo fingerprint but
found both check and build profiles. It resumed by selecting the fingerprint
matching the compiled rlib suffix, without repeating successful cargo stages.
Both helper versions and the failure record remain attached.

Receipts are self-contained archives with a SHA-256 inventory of every member:

- `hoisted-results.tar.xz`: 2,136,116 bytes, SHA-256
  `8fedae96e7473f8817a8b26f9238aa720d31eb2716e08fd9f2f8eccea1f2a34e`.
  Contains all three batches, raw samples, complete semantic outputs, captured
  harness source, input/build identities and source receipts.
- `hoisted-validation.tar.xz`: 119,064 bytes, SHA-256
  `455b4a7947f6f2f74428fdc78827d2c20eb14460f3b1c5a5b951782bb4d0f370`.
  Contains complete raw check/build/test logs, completed stage ledger and the
  metadata-helper failure and recovery. Every member digest was verified
  before commit.
