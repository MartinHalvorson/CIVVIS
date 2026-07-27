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

### Upstream champion confirmation

After that screen completed but before this branch was ready, `ff083ea` shipped
the repository's first evolved genome and changed both `strategic_deep` arms
from fallback weights to the new incumbent. The original screen remains final
for its population, but cannot decide a treatment on the newly shipped agent.

Before observing any games with that genome, a second 30-map development
screen is registered on fresh seed 109000 with otherwise identical settings.
Only a favorable win direction earns a disjoint 120-map gate at seed 110000;
exact ties stop again. This is a confirmation against a changed incumbent, not
an extension of the fallback-weight sample.

## Result

Across 30 fresh mirrored maps (60 games), the candidate and control split wins
30-30. Every map was neutral: zero candidate sweeps, 30 splits/draws, and zero
control sweeps (`p = 1.0`). Paired score was 50.0%, with a 33.2%-66.8% Wilson
interval and +0 Elo point estimate. The promotion gate was INCONCLUSIVE.

The diagnostic traces were effectively identical too:

- victory types were exactly two culture, 23 religious, and five score for
  each arm;
- macro-search exposure was 329/678 reviews for each arm;
- plan switches and every plan-exposure percentage were identical;
- terminal score was 50.0%, with zero favorable directions, 29 neutral, and
  one adverse (`p = 1.0`).

The outcome-only rule removed the prior treatment's harm, but also removed all
measurable headroom. At this benchmark either there was no uniquely available
one- or two-action checkmate, or the ordinary controller closed the same game
on its remaining turn. Per the pre-registration, exact win ties do not earn a
disjoint gate. The entrant remains an evaluator-only negative control, its
flag stays off, and `strategic_deep` is unchanged.
