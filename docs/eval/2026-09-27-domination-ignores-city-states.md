# A Domination seat that goes to war with city-states

_2026-09-27 · `claude-opus55-0927-5d` on `martins-m4-air` · simulator rounds, no native games_

The native King Domination record (four majors, Gran Colombia, Tiny 60×38
Pangaea, Online) holds no Domination win: the seat's first war on a major
comes at turn 100–185, it rarely holds a foreign major city, and a rival
finishes Science, Culture or Religion around turn 170–220
(`docs/eval/2026-09-27-native-conquest-accounting.md`,
`docs/eval/2026-09-27-king-counter-response-trial.md`). This round looks for
actuation defects in how that seat chooses and opens its wars.

## Instrument

The private ladder proxy (`examples/ladder_proxy.rs`, not committed) in the
native profile: seat zero Gran Colombia with `--target domination`, the live
controller and the live forced genes, `handicap_exempt`; three rival seats
`AdvancedAi::new()` + `enable_live_bridge()` with King's AI bonuses; six
city-states; full 250-turn clock; paired by seed. It records the seat's first
war turn, declarations on majors and on minors, cities it ever held that a
major founded, and foreign original capitals.

## What the baseline does

Sixteen games, seeds 38210000+: **0 wins**; first war (any side) at a median
turn 145; the seat declared on a major in 8 games and **on a city-state in
7**; one foreign major city held in the whole block; the seat eliminated in
four games. The native trial of 2026-09-27 saw the same minor declaration.

A declaration-gate trace (seed 38210014) shows the city-state war's cost.
With no major capital it could reach, the campaign's fallback ranking in
`assess` — which adds city-states to the candidates whenever the plan is
Conquest — named Rapa Nui at turn 59. War followed at turn 60. From then on
the city-state was the war's front (`wartime_rivals`), so the plan's target
stayed Rapa Nui through turn 128 and beyond, and the army never turned to a
major. A captured city-state counts nothing toward a Domination victory,
which is foreign original capitals.

## The gene: `domination-ignores-city-states`

`AdvancedAi::conquest_campaign_considers_city_states`: an assigned Domination
seat leaves city-states out of that fallback ranking. Every other lane, and
the gene off, is unchanged. Opt-in.

## Measured

Two sixteen-game blocks, paired by seed, gene on the focal seat only:

| King Domination, Gran Colombia, 250 turns | baseline | with the gene |
|---|---:|---:|
| city-state declarations (32 games) | 16 | **0** |
| declarations on majors | 24 | 29 |
| foreign major cities ever held | 4 | 7 |
| foreign original capitals held | 0 | 1 |
| seat alive at the end | 25 | 26 |
| Domination wins | 0 | 0 |
| score share | 13.68% | **+0.72 pp (z +1.15)** |

Block A (38210000+) +0.53 pp, block B (38210016+) +0.91 pp. No Domination
win in either arm: this removes a distraction, it does not make the seat
strong enough to take three capitals.

## Found and not shipped

- **The capital the lane aims at is the one it has seen, however far.**
  `assess` reads the decision view, so `domination_capital_target` ranks only
  capitals the seat has explored; on seed 38210002 that was Cusco, 29 tiles
  away, while the declaration requires the objective within 18 tiles of one
  of our cities. The lane sat in Conquest for 46 turns without declaring. A
  reach filter on that ranking and on the denial target (prototype) was
  neutral: −0.12 pp alone (16 games), and +0.39 pp with this gene against
  the gene's own +0.72 (32 games). In the default configuration the lane
  rarely reaches Conquest before turn 125, so the lock seldom binds.
- **The seat does not find its neighbours.** By turn 45 it had explored 200
  tiles against the rivals' 323 and seen **no** rival capital in 8 of 8
  games; the live contact sweep sends its one Scout across the continent for
  city-state first contact. Turning off `explore-dead-targets` or
  `explore-commit` for the seat read +0.63 / +0.70 pp over 16 games, but
  both carry native-host move-refusal handling and were not changed.
- **A bigger army and an earlier hand-over.** Two units per city through
  the development half read +0.51 pp (16 games), three −0.82 pp. With
  `domination-lane-hands-over` at parity with the weakest rival instead of
  1.8× and two units per city, Conquest moved into the first half (≈25
  turns of it against 3) and the seat held more cities (6 against 1 with
  the reach filters), but share fell −0.67 to −0.83 pp: the army costs the
  economy the second half needs.

## What was decided

- **Shipped and forced on the live seat**: `domination-ignores-city-states`
  (`deploy/live-force-on.txt`). It acts only on an assigned Domination seat,
  whose victory city-states cannot advance.
- **Recorded, not shipped**: the reach, exploration and army findings above.
