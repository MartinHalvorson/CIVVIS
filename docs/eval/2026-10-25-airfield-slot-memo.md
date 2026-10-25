# Reuse derived city data in the airfield-slot scan

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
