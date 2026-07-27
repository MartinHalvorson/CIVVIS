# Dense macro search on generation 14

## Hypothesis

`strategic_deep` spends four times the stock macro-search budget by splitting
it across two axes: reviews every 20 turns rather than 40, and an 80-round
horizon rather than 40. `strategic_r10` spends the same theoretical 4x budget
in the other extreme: it preserves the 40-round horizon and reviews every ten
turns.

On the original default genome, r10 was the weakest 4x allocation measured.
Against 40x40 `strategic` over 120 maps it won 19 decisive directions to seven,
but did not cross anytime evidence and trailed the combined 20x80 agent.
Generation 14 changes the premise enough to require a direct test rather than
an inherited ranking:

- the current 20x80-versus-40x40 safeguard is nearly neutral, 61-59 games and
  five map directions to four;
- halving deep's cadence while retaining horizon lost 105-135 games and 1-16
  decisive maps, crossing evidence for deep at map 54;
- halving horizon while retaining cadence leaned ahead twice, including
  308-292 games on a disjoint 300-map gate, although it did not PASS.

The causal hypothesis is therefore specific: generation 14 benefits more from
another chance to react to its rapidly changing religious and domination races
than from projecting each lane through the increasingly saturated second half
of an 80-round rollout. Dense 10x40 search may convert the same budget into
more useful decisions.

Both existing evaluator agents load the same committed generation-14 genome,
optional value net, priors, branch state, and lane policy. Only budget
allocation differs:

| agent | review cadence | horizon | theoretical compute |
| --- | ---: | ---: | ---: |
| `strategic_r10` | 10 turns | 40 rounds | 4x |
| `strategic_deep` | 20 turns | 80 rounds | 4x |

## Pre-registered evaluation

The development screen uses 120 fresh mirrored maps, skipping smaller samples
because earlier 20-map macro-search conclusions inverted at 120:

```text
cargo run --profile ci --locked --bin ai_eval -- \
  strategic_r10 strategic_deep \
  --pairs 120 --players 4 --width 24 --height 16 \
  --turns 200 --seed 119000 --jobs 12
```

A neutral or deep-favorable game-win direction stops and retains deep. Only a
favorable r10 direction earns a disjoint 300-map promotion gate at seed 120000,
with the challenger first. The development maps are never pooled into the
decision. Only the independent run's formal `promotion gate: PASS` may replace
`strategic_deep`; terminal score, plan labels, and review exposure are
diagnostic.

This is a same-compute strength test, not an efficiency concession. R10 must
beat the current top rung under its existing standard; reallocating compute is
not valuable if it merely reaches parity.

## Development screen

The pre-registered 120-map screen completed on the committed generation-14
genome. It leaned toward dense cadence without resolving the strength question:

```text
mirrored head-to-head: 120 maps, 240 games, 4 players, average 141.6 turns
game-win share: strategic_r10 125/240 (52.1%) strategic_deep 115/240 (47.9%)
paired-map score for strategic_r10: 52.1% (95% Wilson CI 43.2%..60.8%), Elo-equivalent +14 (CI -47..+76)
paired outcomes: strategic_r10 sweeps 13, neutral splits/draws 99, strategic_deep sweeps 8, draw-mixed 0
paired direction: strategic_r10-favored 13, neutral 99, strategic_deep-favored 8; exact two-sided sign p=0.3833 (INCONCLUSIVE DIRECTION)
anytime-valid betting evidence (2.5% per direction after 20 maps): strategic_r10 peak e=1.340e1, p<=0.0746 (not crossed); strategic_deep peak e=1.000e0, p<=1.0000 (not crossed)
promotion gate: INCONCLUSIVE — effect size or anytime-valid evidence has not cleared parity after 120 maps
paired terminal-score diagnostic for strategic_r10: 50.3% (not a promotion input)
terminal-score direction: strategic_r10-favored 59, neutral 7, strategic_deep-favored 54; exact two-sided sign p=0.7069
```

The treatment fired strongly: r10 reached 2,497 of 4,782 reviews while deep
reached 1,400 of 2,628, and r10 switched plans 3.93 times per game versus
2.68. Its extra ten game wins were mostly score wins (31 versus 25), with
religious wins essentially level (87 versus 86). Terminal score was nearly
flat, so the screen does not support a mechanism claim beyond the intended
cadence increase.

The 13-to-8 favorable game-win direction satisfies the pre-registered rule for
earning, but not passing, the independent gate. The decision run is therefore:

```text
cargo run --profile ci --locked --bin ai_eval -- \
  strategic_r10 strategic_deep \
  --pairs 300 --players 4 --width 24 --height 16 \
  --turns 200 --seed 120000 --jobs 12
```

Seed 120000 is disjoint from the development screen. Its result will stand
alone: the 120 development maps will not be pooled into the promotion
decision.

## Independent gate result

The fixed 300-map gate completed cleanly and gave dense cadence a substantial,
consistent point estimate:

```text
mirrored head-to-head: 300 maps, 600 games, 4 players, average 141.4 turns
game-win share: strategic_r10 324/600 (54.0%) strategic_deep 276/600 (46.0%)
paired-map score for strategic_r10: 54.0% (95% Wilson CI 48.3%..59.6%), Elo-equivalent +28 (CI -12..+67)
paired outcomes: strategic_r10 sweeps 46, neutral splits/draws 232, strategic_deep sweeps 22, draw-mixed 0
paired direction: strategic_r10-favored 46, neutral 232, strategic_deep-favored 22; exact two-sided sign p=0.0049 (SIGNIFICANT strategic_r10 DIRECTION)
anytime-valid betting evidence (2.5% per direction after 20 maps): strategic_r10 peak e=2.232e1, p<=0.0448 (not crossed); strategic_deep peak e=1.005e0, p<=0.9954 (not crossed)
promotion gate: INCONCLUSIVE — effect size or anytime-valid evidence has not cleared parity after 300 maps
paired terminal-score diagnostic for strategic_r10: 50.9% (not a promotion input)
terminal-score direction: strategic_r10-favored 163, neutral 16, strategic_deep-favored 121; exact two-sided sign p=0.0148
```

The gate reproduced the screen's direction and made the mechanism more
concrete. R10 reached 6,371 of 12,112 reviews while deep reached 3,533 of
6,580. It switched plans 3.98 versus 2.69 times per game, won 233 religious
games to 191 and 78 score games to 65, and averaged 146.6 terminal score to
140.2. Deep retained small military, gold, and faith leads, but r10 led cities,
population, food, production, culture, and completed games.

Across the development and gate samples, the descriptive total is 449-391
games and 59-30 decisive map directions for r10 over 420 maps. That agreement
is useful evidence that the effect is not peculiar to one seed block, but it
is not the promotion statistic: the first block selected the challenger and
was pre-registered out of the decision.

## Decision

Dense 10x40 search is the strongest point estimate measured on generation 14,
and the independent gate gives significant directional evidence that review
cadence is more valuable than the second half of the 80-round horizon. It did
not satisfy the rule set before the run, however. The fixed-sample Wilson
interval still includes parity and anytime evidence peaked at 22.32 rather
than crossing the two-sided threshold of 40, so the formal result is
**INCONCLUSIVE**.

The pre-registration permits replacement only on `promotion gate: PASS`.
`strategic_deep` therefore remains the evidence-backed top rung and no runtime
behavior changes. R10 remains an evaluator-only challenger. A future
replication may treat this audit as prior evidence, but it must allocate its
error budget before observing new maps rather than extending this gate after
seeing a favorable result.

## Upstream reconciliation

`528cb11` landed while the gate was running. It adds a genome-selection probe
and documentation but does not change any agent, entrant, default, champion,
or evolution behavior. Both compared policies are consequently identical on
current main, so repeating the 420-map audit would test the same contrast.
