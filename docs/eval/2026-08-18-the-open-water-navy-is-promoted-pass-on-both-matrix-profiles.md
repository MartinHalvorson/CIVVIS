# The open-water navy is promoted: PASS on both matrix profiles

_2026-08-18 · `agent/mbp-m5-pro-64/claude-09ea8434`_

## What was asked

`advanced_open_water_navy` (#1989) stops a city building warships it cannot
sail. `BasicAi::best_naval_unit` gated on `city_is_coastal`, which asks only
whether some adjacent tile is water — and **a lake is water**. Firaxis allows a
lakeside city to build a Galley, so the trap is faithfully reproduced, and the
controller walked into it every game.

It shipped off by default with a 40-map screen reading +44 Elo-equivalent and
`INCONCLUSIVE`. #1993 then measured why that screen could not have said
anything: at these break rates a 40-map run promotes nothing under about **+97
Elo-equivalent**. So the question was re-asked at a length that can answer it.

## How it was measured

Three runs, escalating:

1. **40 pairs** (seed 8100000): 56.2%, +44, INCONCLUSIVE — the unasked question.
2. **200 pairs, deployment profile only** (seed 8300000): 57.2%, **+51
   Elo-equivalent (CI +13..+97), PASS**.
3. **`--matrix`, 200 pairs per profile** (seed 8700000), which is the gate a
   promotion actually has to clear: a strength PASS on the six-player Online
   deployment *and* no established regression on the compact Standard safety
   profile.

## What it measured

| profile | score | Elo-equivalent | interval | verdict |
|---|---:|---:|---|---|
| deployment-online | **58.8%** | **+61** | +21..+109 | **PASS** |
| compact-standard | 52.2% | +16 | −8..+39 | INCONCLUSIVE |

**`multi-profile promotion gate: PASS — advanced_open_water_navy cleared every
required profile.`**

The safety profile is inconclusive with a positive point estimate and an
interval whose worst case is −8, so there is no established regression there;
that is what the gate asks of it, and the deployment profile carries the
strength claim.

The mechanical result that motivated it, now the production default, on three
150-turn six-player games at 74x46, 9 city-states, Online, seeds 21000000–02:

| | before | after |
|---|---:|---:|
| naval hulls built | 53 | **26** |
| hulls that never moved once | 20 | **3** |
| galley movement rate | 13.0% | **43.7%** |
| `audit` major idle-field | 21.19% | **18.38%** |

Galley was the largest single block of wasted major unit-turns in the audit —
20.0% of all major idle, idle on 54.3% of its own turns. It no longer appears
in the table's top four at all; builder, archer and scout now lead it.

## What was decided

**Promoted.** `AdvancedAi::new()` carries the rule; `AdvancedAi::legacy()` does
not, so the frozen `advanced_v1` anchor is untouched and `ANCHOR_BEHAVIOUR_FNV`
stays green on its own evidence rather than on an argument.

The evaluator arm is converted from `advanced_open_water_navy` to
`advanced_without_open_water_navy`: once a treatment is in the bundle, the arm
that measures it is the one that takes it back out. Leaving the enabling arm in
place would have left a registry entry that constructs an agent identical to
`advanced` and can only ever report parity.

⚠ The lesson worth carrying is the escalation, not the fix. The same arm, the
same code and the same profile read **INCONCLUSIVE at 40 maps and PASS at 200**.
The first run had not found a weak effect; it had not looked. Screens on this
profile start at 200 pairs.
