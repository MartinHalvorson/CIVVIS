# A Domination lane that waits until turn 125 to fight

_2026-09-27 · `claude-opus55-0927-5d` on `martins-m4-air` · simulator rounds, no native games_

Follow-up to `docs/eval/2026-09-27-domination-ignores-city-states.md`, same
instrument and profile: the private ladder proxy with seat zero Gran Colombia
assigned Domination, the live controller and forced genes (now including
`domination-ignores-city-states`), three King rivals, six city-states, Tiny
60×38 Pangaea, Online, the full 250-turn clock, paired by seed.

## The clock

`assess` keeps an assigned lane in its development half — Expansion, while it
has fewer cities than it wants and a site to settle — until
`phase_specialization_active`, which is halfway through the clock: turn 125 of
the ladder's 250. The Domination lane's Conquest therefore starts at turn 126,
and in the proxy the seat's first war comes at a median turn 145 — very often a
rival's declaration on us. By then a King rival leads it by a wide margin
(seed 38210000: power 329 against 479–642), and rivals finish Science around
turn 190–210.

## The gene: `domination-specializes-earlier`

An assigned Domination lane's development half ends at
`DOMINATION_SPECIALIZATION_PERCENT` = 40% of the clock (turn 100 of 250),
at King and below. No
other lane's clock moves; off, byte-identical. Opt-in; forced on the live seat.

## Measured

Specialization point swept on 32 paired games (38210000+ and 38210016+)
against the current live configuration:

| Domination development half ends | foreign major cities held | foreign capitals | alive | Δ share |
|---|---:|---:|---:|---:|
| turn 125 (shared clock) | 7 | 1 | 26 | — |
| turn 75 (30%) | 4 | 0 | 26 | −0.62 pp |
| turn 88 (35%) | 16 | 0 | 29 | −0.64 pp |
| **turn 100 (40%)** | **14** | 1 | 27 | +0.11 pp |
| turn 150 (60%) | 9 | 0 | 27 | +0.76 pp |

Turn 100 replicated on 32 fresh seeds (38210032+): cities held 1 → 8,
capitals 0 → 1, alive 23 → 27, declarations on majors 20 → 30,
**+1.69 pp (z +1.84)**. Over all 64 paired games:

| King Domination, 64 paired | turn 125 | turn 100 |
|---|---:|---:|
| declarations on majors | 49 | 60 |
| foreign major cities ever held | 8 | **22** |
| foreign original capitals held | 1 | 2 |
| seat alive at the end | 49 | 54 |
| Domination wins | 0 | 0 |
| score share | 13.82% | **+0.90 pp (z +1.27)** |

**Emperor says no.** The same clock at Emperor (32 paired, 51210000+):
−1.06 pp (z −2.33), seat alive 27 → 24, cities held 2 → 1. The stronger
rivals punish the earlier war, so the gene acts only up to King
(`DOMINATION_SPECIALIZATION_MAX_ORDER` = 4); above it the halfway clock
stands and the gene is inert.

Still no Domination win: the seat now takes and holds outlying cities, but a
second or third original capital is beyond it. Capital sieges are the next
bottleneck (the one offensive siege traced broke the walls and never brought
the city below 170 of 200 health).

## What was decided

- **Shipped and forced on the live seat**: `domination-specializes-earlier`,
  King and below only.
- Earlier than 40% loses the economy the war needs; later (60%) scores better
  but conquers less, which is the wrong trade for a Domination seat.
