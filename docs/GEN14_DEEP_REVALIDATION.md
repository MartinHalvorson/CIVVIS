# Generation-14 deep-search revalidation

## Question

`strategic_deep` is the evidence-backed top macro-search rung. It reviews every
20 turns and projects each candidate victory lane for 80 rounds, spending four
times the theoretical search budget of stock `strategic`, which reviews every
40 turns for a 40-round horizon.

That ranking needs a direct current-policy check. Deep was originally promoted
on default weights by a 339-261 result over 300 mirrored maps, about +45 Elo.
The committed genome subsequently changed the policy population. Generation 2
reproduced the ordering over 300 disjoint maps, but generation 14 has only a
60-map, 200-turn safeguard: deep won 61-59 games and five decisive maps to
four. That result correctly prevented a reversal, but it is too small to show
that the current genome still earns a fourfold compute budget.

Recent generation-14 ablations sharpen the question without answering it.
Removing half the reviews was harmful, while reducing horizon and reallocating
the same budget toward cadence both leaned ahead on independent maps without
passing the formal gate. Doubling deep again to 10x80 changed routing and plan
switching but landed at 302-298 games over its independent 300-map gate. The
remaining uncertainty is therefore the baseline contrast itself: current
20x80 deep search versus current 40x40 search.

Both existing entrants load the same committed generation-14 genome, scripted
controller, lane set, priors, branch state, and optional value evaluator. The
repository has no committed `valuenet.json`, so the provenance line reports
stock `strategic`'s effective name as `strategic_score` and deep as
`strategic_deep` with an untrained optional net. This naming asymmetry does not
create a treatment difference: both agents use the identical score-share
evaluator. Their only behavioral difference is macro-search budget.

| agent | review cadence | horizon | theoretical stock compute |
| --- | ---: | ---: | ---: |
| `strategic_deep` | 20 turns | 80 rounds | 4x |
| `strategic` (effective `strategic_score`) | 40 turns | 40 rounds | 1x |

## Pre-registered evaluation

One fixed independent run uses the same 300-map scale as the original
promotion and extends the cap from 200 to 500 turns so unresolved long games
cannot hide a late conversion difference:

```text
cargo run --profile ci --locked --bin ai_eval -- \
  strategic_deep strategic \
  --pairs 300 --players 4 --width 24 --height 16 \
  --turns 500 --seed 123000 --jobs 12
```

Seed 123000 is disjoint from every generation-14 search-budget block through
the ultra gate at seed 122000. The run is not a development screen: there is
no candidate selection, early stopping, result-dependent extension, or second
seed. Earlier 200-turn maps are not pooled into its decision because they used
a different cap and motivated this audit.

The repository's unchanged paired-map promotion gate is the decision rule:

- `promotion gate: PASS` supplies current-genome evidence that deep remains
  stronger and retains the ranking;
- `promotion gate: RETAIN strategic` establishes a current-genome reversal and
  makes the cheaper 40x40 agent the evidence-backed recommendation;
- `promotion gate: INCONCLUSIVE` establishes neither superiority nor
  non-inferiority, so the prior evidence-backed ranking remains unchanged and
  the fourfold cost is recorded as unvalidated on generation 14.

Game wins are the promotion input. Paired map directions, terminal score,
victory mix, plan commitment, and search exposure are recorded to explain a
result but cannot override it. A game still unresolved at turn 500 contributes
a paired draw rather than a fabricated loss. Deep receives no strength credit
merely for being more expensive, while the cheaper agent receives no
non-inferiority concession from a null result.
