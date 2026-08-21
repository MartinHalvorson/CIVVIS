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

Three opt-in genes (`PRODUCTION_OPT_INS`, off everywhere until priced), one
per gate, so the screen prices each: `theology-for-founders` (a founder
outside the Religion lane researches Theology next, after its first
government); `founder-temple` (claims an idle Holy Site city, Holy City first,
for Shrine then Temple, and under pressure preempts the Holy City's queue —
gold when the treasury covers the 480 — traced, the queue was never idle while
the cities flipped); `inquisition-on-threat` (`apostle_cap = 1` for a founder
under pressure with no Inquisition launched, bought AFTER the Missionary corps
and only when the bank covers it; the existing step walks it to the Holy City
and launches; the existing Inquisitor purchases follow; census
`inquisition_apostles`). The first cut bundled all three and held the whole
bank for the Apostle; see below for what that measured.

`holy_lane_parity` — the Holy Site priced at 850 like the Culture lane's
Theater Square instead of 210 — gains toggles and a `PRODUCTION_OPT_INS` row,
so the prophet race's other half is priced in the same screen.

**Fires-check of the first cut** (four 6p games, deployment genome + gene on
every seat): 10 Inquisitions launched (0–1 before the saving leg), every
founder's Apostle bought at turn 98–149 and launched at its Holy City, and no
game ended by religious victory (two of four before). ⚠ The screen then
priced that same cut at −8.2 pp (below): when every seat defends, nobody
wins by religion; when half do, the defenders lose. A fires-check says a
mechanism runs, never that it pays.

## How it was measured

| what | instrument | result |
|---|---|---|
| the first cut (Theology + Temple + **the bank held for the Apostle** + the Apostle ahead of any Missionary), with `holy-lane-parity` | `gene_screen --players 6 --all-seats --baseline best`, seeds 53M, stopped at 1,662 seat-pairs | `inquisition-on-threat` **−8.2 pp [−10.3, −6.1]** (z −7.7), share −0.38 pp (z −4.6) — HURTS past the family-wise bar; `holy-lane-parity` +0.8 [−1.3, +3.0] |
| the second cut (no hoard; the Apostle after the Missionary corps) as three genes, with `holy-lane-parity` | same design, 1,000 pairs (6,000 seat-pairs), seeds 54M | S6_RESULT |

**Why the first cut lost — by founder status, from its own rows (1,662 seat-pairs):**

| seat | gene | n | win | cities under a foreign faith | launched an Inquisition | Faith banked at the end |
|---|---|---|---|---|---|---|
| founder | **on** | 832 | **19.5%** | 2.93 | 88% | 1,915 |
| founder | off | 831 | **35.4%** | 2.10 | 52% | 1,424 |
| non-founder | on | 834 | 5.6% | 3.94 | 0% | 1,027 |
| non-founder | off | 833 | 6.2% | 3.88 | 0% | 993 |

Non-founders were untouched, so the whole −8 pp is the founders' −16. The
gene did what it was built to do — the Inquisition launched in 88% of the
founders' games against 52% — and the cities flipped *more*. Holding the bank
for the 400-Faith Apostle meant thirty turns without the 250-Faith Missionary
the baseline buys, and that Missionary corps is what had been holding the
pressure race. The lesson generalises: a late strong unit bought by starving a
steady weak one loses the race in between.

## What it means

S6_MEANING

## Not built, ranked

- The prophet roster: `data/great_people.json` holds three prophets against
  `max_religions() = 4` at 6p, and the fourth contender's prophet points are
  "earnable" and so never refunded as Faith. A rules change, priced by the
  anchor protocol, not by a screen.
- Moksha's *Citadel of God* (hard immunity to foreign pressure) outside the
  Religion lane's governor order; prophet-race tempo (Prayers before the
  Shrine, Divine Spark).
