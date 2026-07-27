# Religious checkmate search

## Hypothesis

`strategic_deep_conversion` proved that exact local religious improvement is
still a proxy: it built larger empires but lost its disjoint win gate 114-126,
with 65 religious victories to the control's 81. The search could spend a
missionary's move or charge to improve foreign conversion geometry before the
ordinary controller defended its own core or pursued a longer-lived target.

The narrow repair is to remove the proxy. `strategic_deep_checkmate` searches
the same immediately legal religious effects and move-then-effect sequences on
cloned states, but executes the first sequence only when `Game::apply` records
an actual religious victory for that civilization. Pressure, city majorities,
faith, and terminal score cannot authorize an action. Without executable
checkmate, the ordinary `strategic_deep` controller receives an identical
state and takes the whole turn.

This retains the useful engine-level capability: `Action::Spread` names a unit
but not a city, so cloning the exact result can find a winning adjacency or
one-step approach the scripted target route misses. It removes the repeated
local optimization that the previous gate rejected. The flag is off by
default and team games abstain because their victory predicate accepts any
religion founded by a teammate.

## Pre-registered evaluation

The development screen is 30 fresh mirrored maps against the strongest
shipped search agent:

```text
cargo run --profile ci --bin ai_eval -- \
  strategic_deep_checkmate strategic_deep \
  --pairs 30 --players 4 --width 24 --height 16 \
  --turns 200 --seed 108000 --jobs 12
```

Only a favorable win direction earns a disjoint 120-map promotion gate at
seed 109000. Exact ties mean the treatment is too rare or redundant and stop
the experiment. Terminal score and plan labels are diagnostics only. Only the
existing win-based promotion gate may change the incumbent.

## Result

Pending.
