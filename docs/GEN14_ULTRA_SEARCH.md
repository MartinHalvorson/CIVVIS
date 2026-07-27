# Generation-14 ultra macro search

## Hypothesis

The strongest promoted macro-search agent, `strategic_deep`, reviews every 20
turns and projects every candidate lane for 80 rounds. Its 20x80 allocation is
four times the stock 40x40 search budget. The two doublings originally won both
alone and together, and the combined agent passed a fresh 300-map promotion
gate.

Generation 14 makes the next strength-first dose unusually well identified.
Review cadence, not the second half of the horizon, is now the load-bearing
axis:

- removing every other deep review while retaining horizon 80 lost 105-135
  games and 1-16 decisive maps;
- retaining cadence 20 while halving horizon leaned ahead of deep in two
  disjoint samples, 429-411 games descriptively;
- reallocating the same 4x budget to 10x40 dense search leaned ahead in two
  further disjoint samples, 449-391 games and 59-30 decisive maps
  descriptively. Its independent 300-map block was 324-276 games and 46-22
  map directions (`p = 0.0049`), although the formal promotion gate remained
  inconclusive.

The conservative response to that last result is not to rename dense search
after an inconclusive gate. It is to preserve the full promoted horizon and
add the cadence that generation 14 repeatedly favored. `strategic_ultra`
therefore reviews every ten turns and projects all branches for 80 rounds:

| agent | review cadence | horizon | theoretical stock compute |
| --- | ---: | ---: | ---: |
| `strategic_deep` | 20 turns | 80 rounds | 4x |
| `strategic_ultra` | 10 turns | 80 rounds | 8x |

Both entrants load the same committed generation-14 genome, optional value
net, priors, warm branch state, evaluator, lane set, and scripted controller.
Only periodic review cadence differs. This is not early stopping, pruning, or
reallocation based on a shallow ranking; every review keeps the complete
80-round counterfactual that previously passed.

## Pre-registered evaluation

The development screen uses 120 fresh mirrored maps, the minimum scale at
which earlier macro-search conclusions stopped inverting:

```text
cargo run --profile ci --locked --bin ai_eval -- \
  strategic_ultra strategic_deep \
  --pairs 120 --players 4 --width 24 --height 16 \
  --turns 200 --seed 121000 --jobs 12
```

A neutral or deep-favorable paired-map direction stops the experiment and
retains deep. Only an ultra-favorable direction earns one independent 300-map
promotion gate, fixed before the screen at seed 122000:

```text
cargo run --profile ci --locked --bin ai_eval -- \
  strategic_ultra strategic_deep \
  --pairs 300 --players 4 --width 24 --height 16 \
  --turns 200 --seed 122000 --jobs 12
```

The development maps will not be pooled into the decision. Only the
independent run's formal `promotion gate: PASS` can establish
`strategic_ultra` as a stronger top rung. Game wins, paired map directions,
terminal score, victory mix, plan labels, and review exposure are recorded,
but diagnostics cannot override the gate.

This entrant costs twice as much macro-search compute as deep. It therefore
gets no non-inferiority concession: parity is a failure to justify the extra
budget. The claim is positive and falsifiable — complete 10x80 search must
beat complete 20x80 search on fresh games.

## Development screen

The pre-registered 120-map screen completed on the committed generation-14
genome and leaned toward ultra without resolving the strength question:

```text
mirrored head-to-head: 120 maps, 240 games, 4 players, average 140.5 turns
game-win share: strategic_ultra 125/240 (52.1%) strategic_deep 115/240 (47.9%)
paired-map score for strategic_ultra: 52.1% (95% Wilson CI 43.2%..60.8%), Elo-equivalent +14 (CI -47..+76)
paired outcomes: strategic_ultra sweeps 12, neutral splits/draws 101, strategic_deep sweeps 7, draw-mixed 0
paired direction: strategic_ultra-favored 12, neutral 101, strategic_deep-favored 7; exact two-sided sign p=0.3593 (INCONCLUSIVE DIRECTION)
anytime-valid betting evidence (2.5% per direction after 20 maps): strategic_ultra peak e=1.575e0, p<=0.6350 (not crossed); strategic_deep peak e=1.000e0, p<=1.0000 (not crossed)
promotion gate: INCONCLUSIVE — effect size or anytime-valid evidence has not cleared parity after 120 maps
paired terminal-score diagnostic for strategic_ultra: 49.8% (not a promotion input)
terminal-score direction: strategic_ultra-favored 60, neutral 4, strategic_deep-favored 56; exact two-sided sign p=0.7807
```

The budget treatment fired cleanly. Ultra reached 2,685 of 4,871 reviews while
deep reached 1,516 of 2,716, with nearly identical rollout exposure rates of
55% and 56%. Ultra switched plans 3.80 times per game versus 2.81 and converted
92 religious wins to deep's 79, but deep led terminal score 145.0 to 142.9,
military 159.6 to 149.5, and domination wins nine to three. The point estimate
therefore supports a gate, not a claim that extra search improved the terminal
empire.

The pre-registered paired-map rule is mechanical: 12 ultra-favorable maps to
seven deep-favorable maps earns the already fixed independent run at seed
122000. The development block remains excluded from that decision.

## Independent gate result

The fixed 300-map gate completed at near-exact parity:

```text
mirrored head-to-head: 300 maps, 600 games, 4 players, average 140.5 turns
game-win share: strategic_ultra 302/600 (50.3%) strategic_deep 298/600 (49.7%)
paired-map score for strategic_ultra: 50.3% (95% Wilson CI 44.7%..56.0%), Elo-equivalent +2 (CI -37..+42)
paired outcomes: strategic_ultra sweeps 25, neutral splits/draws 252, strategic_deep sweeps 23, draw-mixed 0
paired direction: strategic_ultra-favored 25, neutral 252, strategic_deep-favored 23; exact two-sided sign p=0.8854 (INCONCLUSIVE DIRECTION)
anytime-valid betting evidence (2.5% per direction after 20 maps): strategic_ultra peak e=1.000e0, p<=1.0000 (not crossed); strategic_deep peak e=1.575e0, p<=0.6350 (not crossed)
promotion gate: INCONCLUSIVE — effect size or anytime-valid evidence has not cleared parity after 300 maps
paired terminal-score diagnostic for strategic_ultra: 50.0% (not a promotion input)
terminal-score direction: strategic_ultra-favored 138, neutral 16, strategic_deep-favored 146; exact two-sided sign p=0.6779
```

Again the treatment fired. Ultra reached 6,461 of 12,226 reviews and switched
plans 3.66 times per game; deep reached 3,533 of 6,595 and switched 2.76 times.
The extra decisions changed conversion mix without improving total conversion:
ultra won 226 religious games to 206 and added five culture wins, while deep
won 85 score games to 66 and seven domination games to five. Their terminal
economies were nearly indistinguishable, with ultra score 142.6 to deep 142.4
but small deep leads in cities, population, production, military, and gold.

Across the development and gate blocks, the descriptive total is 427-413 games
and 37-30 decisive maps for ultra over 420 maps. The point estimate is mildly
favorable, but the development maps earned the gate and are excluded from its
decision. The independent block is the result that controls promotion.

## Decision

`strategic_ultra` does not earn promotion. Its doubled compute clearly produced
more reviews and more plan switching, yet the independent gate measured +2 Elo,
25-23 map directions, and exactly even terminal score. With no
non-inferiority concession for an agent that costs twice as much, this is a
failure to justify the budget even before applying the formal gate; the gate
itself is **INCONCLUSIVE**.

The result also closes a tempting extrapolation. Generation 14 benefited from
10-turn cadence when horizon fell to 40, but that gain does not add to a full
80-round horizon. Review frequency and depth interact: buying both beyond
20x80 changes routing without a measurable strength return. `strategic_deep`
remains the evidence-backed top rung, `strategic_ultra` remains evaluator-only,
and no runtime default changes.

## Upstream reconciliation

`13f7d3a` landed while the gate was running. It adds an observer-only AI
reasoning journal; headless mode is off by default, cloned rollout agents are
silent, and its validation produced byte-identical game results on all ten
soak seeds. It does not alter either compared policy, so the gate remains
current after reconciliation.
