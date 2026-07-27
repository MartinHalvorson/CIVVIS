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
