# One production player, two execution adapters

The `observed-player-v1` tournament contract shares the deployment policy
bundle, gene ledger, tactical finishing pass, and main planner with the Civ VI
driver. Native execution obtains a disposable `player_decision_view`; live
execution obtains its board from `LiveMirror`. Neither adapter copies simulated
treasury, units, cities, or combat results back into the authoritative engine.
Frozen historical controller anchors are explicitly outside this contract.

Every standard tournament chair still draws its own genome and contributes a
row. Victory objectives are sampled independently per chair, from
`ai::player::TRAINING_TARGETS`. `gene_screen --target-mix` accepts a weighted
list (repeat a name to give it additional weight); `civvis` means adaptive.
No Firaxis-style chair is reserved or removed from the six-seat sample.
The new standard enables native scored competitions so diplomatic objectives
have their recurring point sources. `--no-native-competitions` is a nonstandard
probe. Historical files retain their old classification; competition settings
and player contracts cannot be pooled by the analyzer or reporting columns.

The mixture is an initial coverage prior, **not a fitted Firaxis emulator**.
Pace telemetry is collected for every major every 25 turns. Measure its city,
research, culture, and military distributions against public live rivals:

```sh
python3 tools/civ6_transfer_calibration.py --live /path/to/run --native /path/to/rows.jsonl
```

Missing readings remain missing. Repeated combat frames do not multiply a
turn's weight. Prince and Emperor observations are not pooled. Matching speed
and difficulty is necessary, not sufficient: map, leader mix, ruleset, game
modes, and survivor/contact selection must also be reviewed before fitting a
new prior. The tool reports pace; it does not change the deployment policy.
`--fit-target-mix` proposes a mixture only when complete map/rules/modes and
competition profiles match. It splits independent game IDs deterministically,
fits log-pace error on training games with a 5% floor for every objective, then
checks the frozen weights on untouched games. Each split needs three live and
three native games plus per-target feature coverage. The candidate must improve
held-out error by more than 10%; a proposal still needs prospective tournament
validation. This is a prior-fitting tool, not proof of Firaxis policy recovery.
Continuation directories share their original game identity and duplicated
observations cannot inflate fitting evidence.
Handicap/seat-asymmetry work is deliberately excluded. Barbarians retain the
existing Immortal setting rather than inventing a new non-Civ difficulty band.

## Information and execution

The native planning board retains own assets, current visible units, last-seen
terrain/cities, and met rivals' public aggregates. Unrevealed resources, rival
research queues, hidden units, and barbarian mission plans are withheld. Unknown
terrain uses the same domain-specific frontier helper as the live mirror.
Planning has its own random stream rather than the authoritative future rolls.
Native execution refreshes its observation after discoveries/combat/refusals,
with an opening batch and up to two additional frames by default.

Entity IDs remain opaque execution handles, including the native allocator;
this is not a security boundary for arbitrary untrusted agents. The production
controller must not treat allocation history as intelligence. Live-only model
corrections and blocked-action facts remain the execution adapter's inputs.

## Action evidence

The live brain appends `decisions.jsonl` before publishing a response. Each
record contains the native actions, finishing lines, native/host ID maps, final
transport orders, frame, contract, binary path/on-disk SHA-256, and a content digest. Records
are explicitly `not_observed`: an emitted order is not proof of execution.
Existing host verification and combat ledgers remain the outcome evidence.
The disk digest is not loaded-process attestation if an executable is replaced
while its old process remains alive.

For a dedicated diagnostic run, `civ6_play.py --action-transitions` enables
full before/after observations around each host request. The flag enables
state export and must be read back in the seat report. Diagnostic snapshots
have distinct event names, so they cannot wake the brain mid-batch. Replay:

```sh
cargo run --profile ci --locked --features developer-tools --bin live_divergence -- /path/to/events.jsonl --actions
```

The replay emits JSONL cases for the transition checker below. Initial coverage
is research, civics, walking, founding, and fortifying; other requests emit
explicit coverage gaps. Predictions come from applying the request to a rebuilt
pre-action board, never from the observed after-state. Request-boundary movement
and combat can still be asynchronous: a discrepancy is not yet a settled-result
rule error. These diagnostics do not establish full actuation parity.

`--isolated-action-probes` is a separate, opt-in diagnostic mode: it holds the
normal planner while making one adjacent visible-land movement per opening
turn (at most ten). It waits for observed arrival and spent movement, and
timeouts/refusals never count as settled evidence. Do not use its deliberately
perturbed games as strength measurements. The first ten consecutive probes in
`transfer-3386-isolated-20260910` matched both destination and remaining movement.
Their raw observations are retained in
`data/fixtures/player_transfer/isolated_movement_20260910.jsonl`, with source
digest, and replayed by a Rust regression. This covers only those ten movement
cases, not combat, founding, or all terrain/ability combinations. Reproduce
fixture extraction with `tools/civ6_extract_action_fixture.py`; existing files
cannot be overwritten. Replay freezes map information before each request.

For independently captured, causally isolated transition fixtures:

```sh
python3 tools/civ6_decision_trace.py /path/to/action-cases.jsonl
```

Each case provides `same_turn`, `intervening_actions`, `predictions`, and
`observed`. Deterministic facts compare exactly; stochastic numerical predictions
use explicit `low`/`high` bounds fixed before reading the observed result.
Missing coverage, intervening actions, empty inputs, and mismatches fail the
check. This does **not** upgrade passive `live_divergence` projections into
action replay: their measurements remain confounded by unmodeled orders.

## Evidence still required

This contract is a structural change, not a claim that every Civ VI rule or
host-side strategic fallback is now equivalent. In particular, action-specific
live transition capture/reconstruction, adapter rewrite parity, latent rival
state estimates, policy-counter attribution, and held-out calibration of the
target mixture still need measured coverage. The three visibility invariance
tests and same-observation planner test are necessary but not exhaustive.

Tournament headers record `player_contract` and `target_mix`. The analyzer
refuses to pool mismatching contracts; reporting batches must start a fresh
epoch. Old tournament evidence remains history and the initial deployment
prior, not fresh validation of the new information boundary.
