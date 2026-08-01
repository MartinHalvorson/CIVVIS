# Live policy plus direct envoy production

## Question

Can the two independently favorable but unresolved deployment directions compose
into a controller strong enough to replace `advanced` under the repository's
two-profile promotion matrix?

`advanced_policy_live_control` enables the counterfactual policy deck. Across
its fixed 300-map matrix it scored 52.3% on compact and 54.3% on deployment,
but neither Wilson lower bound cleared parity. `advanced_envoy_priority` routes
the first incomplete Diplomatic Quarter, Consulate, or Chancery stage into an
otherwise idle adaptive-production queue. Across its 120-map matrix it scored
48.3% on compact and 54.4% on deployment, again without clearing the gate.

Both numbers point in the same deployment direction, but they can overlap or
move opportunity cost elsewhere. This document fixes one composite experiment;
it is not evidence that the components add.

## Treatment

`advanced_policy_envoy_priority` differs from stock `advanced` on exactly three
reported axes:

1. `policy-deck-live`: use the existing counterfactual policy deck.
2. `envoy-infrastructure`: price only reachable future envoy income and the
   unlocked Consulate stream.
3. `envoy-priority`: reserve at most one legal, empty adaptive-production queue
   for the first incomplete Diplomatic Quarter, Consulate, or Chancery stage.

It deliberately does **not** change `pol_influence`. The separate
`advanced_envoy_policy` control found that influence weight flat, and the older
`advanced_envoy_economy` arm omits the direct production reservation. This
composite therefore tests the live deck with the mechanism that actually moved
envoys and suzerainty, rather than a three-mechanism amalgam.

All existing queue, Recovery, local-danger, rush, major-war, met-city-state,
contestability, and remaining-horizon guards remain in force. The arm is
evaluator-only and leaves production `advanced` unchanged until the gate passes.

## Pre-registered evaluation

Run this exact command from a release build after the focused construction
tests pass:

```sh
target/release/ai_eval advanced_policy_envoy_priority advanced \
  --matrix --pairs 300 --jobs 12 --seed 80000000
```

The matrix owns its profiles and uses stable, disjoint streams:

| profile | discovery seed prefix | confirmation seed prefix | purpose |
|---|---:|---:|---|
| 4p 24x16 Standard | 80000000..80000299 | 82000000..82000299 | compact safety |
| 6p 74x46 Online | 81000000..81000299 | 83000000..83000299 | deployment strength |

The matrix is the sole decision rule. Compact must not return `RETAIN`; the
deployment profile must return `PASS`. Any other result leaves the composite
evaluator-only. No parameter, seed, profile, sample size, or component will be
changed after either outcome stream is read. If and only if the discovery matrix
passes, run this already-reserved confirmation matrix:

```sh
target/release/ai_eval advanced_policy_envoy_priority advanced \
  --matrix --pairs 300 --jobs 12 --seed 82000000 --confirm 80000000
```

Matrix mode forwards the base confirmation seed to each matching profile stream,
so the compact child labels 82000000 against 80000000 and the deployment child
labels 83000000 against 81000000. Only that disjoint confirmation can supply a
quotable effect size or support a later production-default change.

## Evidence to record

For each profile, record the paired score, Wilson interval, map-direction sign
test, terminal-score direction, policy swaps, envoy count, suzerainty share,
and queued Diplomatic Quarter/Consulate/Chancery stages. The last four are
mechanism diagnostics only; they cannot override the win-based matrix verdict.

## Recorded outcome

The release evaluator built from commit `1e1993f` ran the registered discovery
matrix exactly once. Its combined gate returned `PASS`: compact was 51.1% (95%
Wilson 45.4%..56.7%, +8 Elo-equivalent) and therefore safely inconclusive; the
deployment profile returned `PASS`. That result authorized the reserved,
disjoint confirmation without changing any component or evaluation input.

The confirmation matrix also returned `PASS`:

| profile | streams | paired score | 95% Wilson interval | Elo-equivalent | gate |
|---|---|---:|---:|---:|---|
| compact-standard | 82000000 vs 80000000 | 52.2% | 46.5%..57.8% | +15 (-24..+54) | `INCONCLUSIVE`, accepted as no regression |
| deployment-online | 83000000 vs 81000000 | 57.2% | 51.6%..62.7% | **+51 (+11..+90)** | `PASS` |

On deployment, the challenger was favored on 115 map directions to 52 against
133 neutral, its anytime-valid evidence crossed at map 42, and the evaluator
printed the +51 estimate as `CONFIRMED`. The mechanism diagnostics move in the
same direction—20.6 versus 14.4 envoys and 0.72 versus 0.37 suzerainty share—
but are not inputs to the promotion decision.

The evaluator-only arm is now independently verified. A production-default
change may use this result, but must be a separate, narrow change that preserves
the three reported axes and all existing priority safety guards.
