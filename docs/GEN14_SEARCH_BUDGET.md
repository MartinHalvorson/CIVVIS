# Generation-14 macro-search budget ablation

## Question

`strategic_deep` reviews every 20 turns and projects every candidate lane for
80 rounds. The original default-genome promotion established that the combined
20x80 budget beat the 40x40 `strategic` control. Generation 2 preserved that
ordering over a powered 300-map audit, and generation 14 retained a slight
61-59 win direction in a fresh 60-map safeguard.

That safeguard controls the ranking, but it does not show whether generation
14 still needs **both** compute doublings. Its evolved policy changed empire
size, war thresholds, tactical weights, and game length enough that either the
extra reviews or the extra horizon might now be redundant. A half-cost agent
that independently passes against deep would be a real across-the-board
improvement: the same evidence-backed strength at twice the macro-search
throughput.

The existing evaluator-only agents isolate the two axes without changing
weights, priors, branch policy, or value-net loading:

| agent | review cadence | horizon | macro-search compute |
| --- | ---: | ---: | ---: |
| `strategic_r20` | 20 turns | 40 rounds | 2x |
| `strategic_h80` | 40 turns | 80 rounds | 2x |
| `strategic_deep` | 20 turns | 80 rounds | 4x |

## Pre-registered screens

Both half-budget arms are measured on the same 120 fresh mirrored maps. The
sample size is fixed before either run; previous 20-map conclusions inverted
at 120 maps, so smaller exploratory screens are deliberately skipped.

```text
cargo run --profile ci --locked --bin ai_eval -- \
  strategic_r20 strategic_deep \
  --pairs 120 --players 4 --width 24 --height 16 \
  --turns 200 --seed 117000 --jobs 12

cargo run --profile ci --locked --bin ai_eval -- \
  strategic_h80 strategic_deep \
  --pairs 120 --players 4 --width 24 --height 16 \
  --turns 200 --seed 117000 --jobs 12
```

The two screens are a factorial diagnosis and are not pooled. If neither
cheaper arm has a favorable game-win direction, the audit stops and deep is
retained. If one or both are favorable, exactly one earns a fresh 300-map
reversal gate: highest paired-map score wins selection, then most favorable
map directions, with `strategic_r20` the final deterministic tie-breaker.

The selected challenger is run first at seed 118000 so the existing formal
promotion gate judges it. Only `promotion gate: PASS` can replace
`strategic_deep`; terminal score, plan labels, and search exposure are
diagnostic. Selection uses only the development maps and the decision uses
only the disjoint gate, so the gate is not repeatedly tested across arms.

If both arms pass separate future gates, a direct head-to-head would be needed
before choosing between them. This audit does not infer non-inferiority from a
null result: lower compute is valuable, but it is not permission to weaken the
top rung.

## Screen results

### Keep cadence, halve horizon: `strategic_r20`

Across 120 mirrored maps (240 games), the 20x40 challenger won 121-119:

- paired score 50.4%, 95% Wilson interval 41.6%-59.2%, and +3 Elo point
  estimate;
- map directions 15 challenger-favorable, 91 neutral, and 14 deep-favorable
  (`p = 1.0000`);
- terminal score 49.7%, with directions 56-1-63 (`p = 0.5825`);
- no anytime-valid crossing in either direction and an **INCONCLUSIVE** formal
  gate.

Deep built the larger terminal empire: 141.7 score to 139.0, 2.57 cities to
2.43, 15.7 population to 14.9, and 151.4 military to 140.2. The challenger
nevertheless converted eight more religious wins, 90 to 82, and finished two
games ahead overall. Search exposure was nearly equal because cadence was
held: 1,517 rollout reviews for r20 and 1,542 for deep. The arm is favorable
by the pre-registered game-win direction and remains eligible for selection.

### Keep horizon, halve cadence: `strategic_h80`

Across the identical 120 maps, the 40x80 challenger lost 105-135:

- paired score 43.8%, 95% Wilson interval 35.2%-52.7%, and -44 Elo point
  estimate;
- map directions one challenger-favorable, 103 neutral, and 16 deep-favorable
  (`p = 0.0003`);
- deep's anytime-valid evidence reached `e = 1,092`, crossing at map 54
  (`p <= 0.0009`);
- terminal score leaned the other way at 50.3%, with directions 63-5-52
  (`p = 0.3511`).

The mechanism is victory conversion rather than terminal economy. The
half-cadence agent built the larger empire and won terminal score, but deep won
115 religious games to 71 and reached 1,457 rollout reviews to 923. Removing
every other review is decisively harmful on generation 14 even when every
remaining review keeps the full 80-round horizon.

## Selected reversal gate

`strategic_r20` has the higher paired score (50.4% versus 43.8%) and is the
only favorable cheaper arm, so the fixed selection rule chooses it. The fresh
decision run is:

```text
cargo run --profile ci --locked --bin ai_eval -- \
  strategic_r20 strategic_deep \
  --pairs 300 --players 4 --width 24 --height 16 \
  --turns 200 --seed 118000 --jobs 12
```

The development screen does not contribute to the decision. Only this
disjoint run's formal `promotion gate: PASS` can replace the 20x80 incumbent.
