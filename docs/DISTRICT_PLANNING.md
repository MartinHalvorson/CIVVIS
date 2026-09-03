# `district-planning`: the city plans its districts, sites and tile buys together

One gene (`Kind::OptIn`, off until the screen prices it) that replaces three
independent greedy answers with one plan per city:

- **Which district next.** The shipped picker asks `district_yields` ×60 for
  whatever site each district's greedy top-2 found, so the district that
  happens to have a good free plot outbids the district the lane actually
  wants. With this on, the city consults its plan: the lane's wishlist
  (`new_city_district_wishlist`, the same table the settler scores a site by)
  joined with a standing production foundation and the district combinations
  the city's terrain can actually support.
- **Where it goes.** The shipped placement ranks fresh sites per district by
  raw adjacency, independently — two districts can both be priced on the same
  river-mountain hex, and the Commercial Hub that orders first takes the
  Campus nest. The plan assigns plots **once**, best marginal value first
  (the assignment the settle look-ahead already does for a city that does not
  exist, done here for the city that does), and charges every site the worked
  tile it destroys.
- **Which tile to buy.** A very valuable site — high adjacency for a heavily
  weighted family — that the city does not own is worth Gold. The plan names
  the plot; the buy clears only when the site beats the best owned
  alternative by a margin after the purchase price, at the lane's own price
  for Gold.

## Vocabulary

The **gene pool** is the collection of all genes, on or off — the registry,
`src/ai/advanced/genes.rs`. A **genome** is one player's set of on genes.
`district-planning` enters the pool off; the screen prices it before any
default question is asked (`docs/GENE_SCREEN.md`).

## What the plan is

For one city: every plannable district family the city still lacks (wishlist
families first, at their weights) plus an Industrial Zone foundation. The
planner then looks at the civilization's *actual* Zone variant: Aqueduct,
Dam, and Canal enter only when that variant receives adjacency from them, and
a German Hansa treats its Commercial Hub as the same kind of adjacency
partner. A support district is dropped unless placing it raises the projected
output of a legal or standing Zone; there are no generic zero-yield Aqueducts
in the queue.

Every legal site (`district_sites` — the engine's own placement legality,
rings 1–3 of owned ground) plus every *purchasable* core candidate (unowned,
adjacent to the border, physically legal for the family) is priced as

    adjacency value at the lane's weights
    − the worked-tile value the district would destroy
    − (for unowned plots) an amortized charge on the purchase price

The assignment is sequential rather than static. Each selected district is
laid as a foundation on a private planning board before the next candidate is
evaluated, so an Aqueduct or Dam placed first is visible to the Industrial
Zone it enables. Each plot and family is still used at most once. On a
Science lane (or an explicit Science target), a Campus claims its premium
ground and specialty slot before the cluster pass.

The production menu receives the resulting sites and a modest construction
order floor: Campus, then Industrial support, then the Zone, then other core
districts. If the immediate plan already needs all remaining specialty slots,
an off-plan economic specialty district cannot take one first. Threatened
cities, Encampment/Aerodrome/Spaceport choices, and a severe amenity collapse
remain exceptions to that reservation.

Early cities fall out of the arithmetic rather than a special case: a young
city owns little beyond rings 1–2, so its legal sites are rings 1–2, and a
ring-3 mountain nest enters the plan only when the buy actually clears.

## What it does not do

- It does not touch settle scoring (`district-lookahead-settle` owns that).
- It does not replace the general plot-purchase pass
  (`priced-tile-purchase` owns working-tile/resource/border economics); it
  adds only the district-site buy its plan names, through the same `BuyPlot`
  action, and stands down where that gene already bought the plot.
- Off, every touched path is byte-identical to before the gene existed.
- It does not fabricate a support district merely because the map has a
  river: the support must have a positive marginal Industrial adjacency on
  the city's current planning board.

## Version 2: `district-planning-2` — buys that actually fire

Version 1's buy never fired on the live seat: no recorded live game holds a
single `buy_plot` order, because the spender demanded 200 Gold of surplus
above the whole strategy reserve before it would even price a plot, and the
live treasury (typically 200–450 Gold against a 300–400 reserve) never got
there. Replaying Emperor game `civvis-20260901T132005Z` at t40/t44: the plan
named the adjacency-4 Campus plot at score ~905 against the 120 floor, every
inner gate passed, and the headroom rule alone refused the buy — the game
then placed three campuses at adjacency ≤ 1 beside that ground.

Version 2 changes only the plan's own buy:

- **Affordability**: a plot the plan prices may spend into the reserve, but
  never below half of it (`bank ≥ reserve/2 + cost` instead of
  `bank ≥ reserve + 200 + cost`). Unplanned surplus plots keep the old rule.
- **Bars**: raw adjacency 2 (was 3) with an edge of 1 (was 2) over the best
  owned site. The 120-Gold-scale score floor still arbitrates, so cheap
  ground with a real edge clears and expensive marginal ground does not.

One version plays per seat; the screen prices the `v2 − v1` contrast
directly.
