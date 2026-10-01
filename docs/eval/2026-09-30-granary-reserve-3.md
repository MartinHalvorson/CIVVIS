# Granary reserve, version 3: housing in the delegated governor

`first-granary-reserve-3` applies version one's rule in both production
governors. A city whose population is within one of its housing, with no
Granary, starts one. Version three also follows the Granary with an Aqueduct
while the city stays bound. In `BasicAi::pick_item` (the delegated governor)
both come ahead of another Builder, the Monument, the Harbor, the specialty
districts and the cheapest-first building list. They come after the
military floor and the Settler step. In `advanced_production` it
uses version one's reservation unchanged.

## Why the delegated governor

With `lane-delegates-production-2` live, a lane seat's cities go through the
delegated governor (`delegated_cities` → `BasicAi::pick_item`) for most of the
game. `first-granary-reserve` and `-2` sit only in `advanced_production`, which
that seat runs only under Recovery, an appointed war or the adaptive expansion
dispatch. The delegated governor reaches a building only after every district
a city still lacks, and a growing city always lacks one, so housing never comes
up.

Live King Gran Colombia, Domination lane, frame-0 states for turns 40–150:

| run | city-turns | at housing (×0.25 growth) | one short (×0.5) | bound with no Granary | median food surplus there |
|---|---:|---:|---:|---:|---:|
| civvis-20261001T003717Z | 529 | 55% | 34% | 88% | 2 |
| civvis-20261001T000033Z | 604 | 36% | 36% | 68% | 3 |

In 003717Z, Pottery came at turn 12, yet no city had a Granary at turn 127 and
none had an Aqueduct, although Engineering came at turn 82. Seven of nine
cities sat at 2–7 population, while the rivals' cores reached 13 (Ostia,
Patna). Science trailed 46 against 93–143. Every housing-bound city could
build the Granary.

## Screen

Standard fieldless shape (6 majors, 74×46 continents, Emperor, 250 turns),
gene on at p = 0.25, from a dirty build tree on top of 1fd8b1ca8.

| batch | seeds | games | seats on/off | win Δ | share Δ |
|---|---|---:|---:|---:|---:|
| 1 | 930610000–930610047 | 48 | 71/217 | +0.3 pp (z +0.06) | **+2.01 pp (z +2.40)** |
| 2 | 930620000–930620047 | 48 | 75/213 | −6.3 pp (z −1.42) | +1.43 pp (z +1.88) |
| pooled | | 96 | 146/430 | −3.1 pp (z −0.89) | **+1.71 pp (z +3.05)** |

| 3 (ahead of Builders) | 930630000–930630047 | 48 | 66/222 | −2.0 pp (z −0.38) | +1.03 pp (z +1.01) |

Batches 1 and 2 placed the step right after the Monument, behind the Builder
step. A replay of 003717Z turns 60–120 with that placement queued no
Granary: under the war floor every idle city took a Builder, a ship or a
unit first. The step now sits ahead of the Builder step, and the same replay
queues 32 Granaries. Batch 3 screens the current placement.

All three batches move score share up and leave the win rate unresolved
(144 seats on in total; the win interval spans about ±5 pp per batch). The
gene is armed on the live seat as a labeled arm (`deploy/live-force-on.txt`),
because the live failure it repairs is measured directly. Its live check is
housing-bound city-turns and population at turn 100.

Compute cost was +3.1 ± 2.3% wall time per turn. The fires artifact is
`docs/gene_screens/fires/first-granary-reserve-3.json`.

## The lent war floor

That replay also showed the Domination war's lent army target (3.0 a city,
`delegated_cities`) sending every idle city into military. A 4-production Cali
started a Crossbowman due in 23 turns, and a 3-production Popayán a
Man-at-Arms due in 27. `BasicAi::lent_military_floor_base` now keeps the
genome's own floor for every city. Above it, only a city that trains the unit
within 16 standard turns builds toward the lent margin. In the same replay,
military produce orders went from about 120 to 82 of 138, with Builders and
Traders in their place.
