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
