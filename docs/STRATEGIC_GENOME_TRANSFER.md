# Evolved-genome transfer into deep search

## Question

`ff083ea` shipped the first evolved genome after it beat `Weights::default()`
under `AdvancedAi`: 1419-1181 games on the deciding 1300-map run, with a 54.6%
paired score and a passing promotion gate. That establishes a real policy
improvement in the architecture that trained and gated it.

The artifact also changed `strategic`, `strategic_deep`, the exhibition, and
the fleet because every `StrategicAi` factory already called
`load_champion("evolved").unwrap_or_default()`. Transfer through the strategic
planner was explicitly not established by the promotion run. It matters:
`strategic_deep_league` previously transferred a win-selected `AdvancedAi`
league genome into the same 20x80 search and lost its screen 24-36, including
14 religious victories to the default genome's 26. A policy ranking need not
survive an architecture that separately owns victory routing.

`strategic_deep_default` is the frozen integration control. It uses
`Weights::default()` with the same 20-turn review cadence, 80-round horizon,
warm branch state, and optional value-net path as the shipped
`strategic_deep`. Its provenance deliberately contains no `best.json`, so a
local or committed champion cannot silently change the control.

## Pre-registered evaluation

The current shipped agent remains the incumbent. The screen asks whether the
frozen default is strong enough to justify a reversal:

```text
cargo run --profile ci --locked --bin ai_eval -- \
  strategic_deep_default strategic_deep \
  --pairs 30 --players 4 --width 24 --height 16 \
  --turns 200 --seed 111000 --jobs 12
```

Only a favorable default-weight win direction earns a disjoint 120-map gate
at seed 112000. A neutral screen or a champion-favorable screen keeps the
shipped genome. Only `promotion gate: PASS` for the frozen challenger may
change `strategic_deep`; terminal score and plan labels are diagnostic. This
is deliberately conservative because the champion already passed its source-
architecture gate and is live.

## Result

Across 30 fresh mirrored maps (60 games), the frozen default lost 27-33:

- paired score for the default was 45.0%, with a 28.8%-62.3% Wilson interval
  and -35 Elo point estimate (equivalently about +35 for the champion);
- map directions were two default-favorable, 23 neutral, and five champion-
  favorable (`p = 0.4531`);
- terminal score was 47.2% for the default, with directions 10-0-20
  (`p = 0.0987`);
- the default had 22 religious and five score victories, while the champion
  had one culture, two domination, 27 religious, and three score victories.

The development diagnostic strongly agrees with the win direction. The
champion averaged 136.0 score to 118.5, 2.60 cities to 2.08, 15.0 population
to 11.8, 171.0 military strength to 119.4, 14.0 science to 12.3, and 18.6
culture to 16.0. It accumulated less gold (363.6 to 423.5), but converted the
larger empire into six additional wins rather than merely farming the proxy.

This is a favorable transfer screen for the shipped champion, not grounds for
a reversal. The pre-registered challenger was the frozen default, and only a
default-favorable direction could earn a disjoint gate. No gate is spent;
`strategic_deep` correctly keeps the evolved genome. `strategic_deep_default`
remains evaluator-only as the stable integration control for future champion
updates.
