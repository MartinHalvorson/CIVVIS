# The founder that never launched its Inquisition

_2026-08-21 · `agent/mbp-m5-pro-64/claude-fable-frames` · PR #2207_

## What was asked

Ship measured improvement to the CIVVIS algorithm. The screens keep saying
where the native game is decided: 59% of 6-player games end by a rival's
religious conversion.

## What the rows said (6p 60k screen, 13,446 seat-pairs, `docs/gene_screens/2026-08-20-p4-…`)

| seat | games | win |
|---|---|---|
| founded a religion | 13,437 | **26.7%** |
| did not | 13,461 | **6.7%** |
| founded, no city under a foreign faith at the end | 6,244 | **52.5%** |
| founded, three or more cities under a foreign faith | 6,232 | **3.0%** |

Exactly three religions are founded in every 6-player game (4,471 of 4,483
games; the other 12 had two). Of the 13,335 seats lost to conversion, 60%
never founded, 58% died with 300+ Faith banked, and **12% had ever launched
an Inquisition**. Founding is worth twenty points of win rate and is
structurally a three-in-six lottery; keeping the cities once founded is the
other twenty-five points, and it is where the controller was not playing.

## Why the founder never defended — four gates in series

Traced with per-turn prints on four 6p games (every seat the repair bundle
plus the gene):

1. **Theology.** The Temple's civic is forced only by the Religion lane, which
   a founder leaves the turn it founds. Outside it, Theology arrived at turn
   100–130.
2. **The Temple.** `religious_production` builds Shrine → Temple only in the
   Religion lane. Two of three founders stood at `temples = 0` from turn 75
   to 145. An Apostle `requires_building: temple` (an Inquisitor too), so
   `unit_purchase_cost` answered `None` the whole time.
3. **The bank.** `BasicAi::cities` buys a Missionary whenever a founder holds
   250 Faith (cap two alive). Founders under pressure sat at 100–250 Faith
   for sixty turns — the Apostle costs 400 (Online) — buying a 200-pressure
   Missionary into a pressure race each time the bank reached one.
4. **The caps.** `religious_spending_with_reserve` sets `apostle_cap = 0`
   outside an offensive posture and buys Missionaries first under pressure;
   `inquisitor_cap` needs the Inquisition launched, which needs the Apostle.

The engine's primitives were all there (`LaunchInquisition`, `RemoveHeresy`
at ×0.25 to every rival pressure, the Apostle walk in
`advanced_religious_step`); no path led to them.

## What was built

`inquisition_on_threat` (`PRODUCTION_OPT_INS`, off everywhere until priced):
a founder outside the Religion lane researches Theology next; `founder_temple`
claims an idle Holy Site city (Holy City first) for Shrine then Temple, and
under pressure preempts the Holy City's queue (gold when the treasury covers
the 480) — traced, the queue was never idle while the cities flipped;
`saving_faith_for_inquisition` holds every Faith sink while home is under
pressure and no Inquisition has launched (Missionaries, Great Person
patronage, Faith buildings, Faith-bought soldiers, and the baseline's
Missionary and Faith Builder through `base.saving_faith_for_inquisition`);
`apostle_cap = 1` with priorities `["apostle"]` while saving; the existing
step walks the Apostle to the Holy City and launches; the existing
Inquisitor purchases follow. Census: `inquisition_apostles`.

`holy_lane_parity` — the Holy Site priced at 850 like the Culture lane's
Theater Square instead of 210 — gains toggles and a `PRODUCTION_OPT_INS` row,
so the prophet race's other half is priced in the same screen.

**Fires-check** (four 6p games, deployment genome + gene on every seat): 10
Inquisitions launched (0–1 before the saving leg), every founder's Apostle
bought at turn 98–149 and launched at its Holy City, and **no game ended by
religious victory** (two of four before, at t152 and t174).

## How it was measured

| what | instrument | result |
|---|---|---|
| both genes against the best genome | `gene_screen --players 6 --all-seats --baseline best --genes inquisition-on-threat,holy-lane-parity`, 1,500 pairs (9,000 seat-pairs), seeds 53M | S5_RESULT |

## What it means

S5_MEANING

## Not built, ranked

- The prophet roster: `data/great_people.json` holds three prophets against
  `max_religions() = 4` at 6p, and the fourth contender's prophet points are
  "earnable" and so never refunded as Faith. A rules change, priced by the
  anchor protocol, not by a screen.
- Moksha's *Citadel of God* (hard immunity to foreign pressure) outside the
  Religion lane's governor order; prophet-race tempo (Prayers before the
  Shrine, Divine Spark).
