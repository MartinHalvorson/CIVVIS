# Idle Faith buys Great People

_2026-08-21 · `agent/mbp-m5-pro-64/claude-fable-frames` · PR #2212_

## What was asked

Ship measured improvement to the CIVVIS algorithm; pick the next target from
the screens' own rows rather than from intuition.

## What the rows said

On the 6p 60k screen (13,446 seat-pairs) half the seats never found a
religion — three Great Prophets exist in `data/great_people.json` — and
those seats won 6.7% of games against the founders' 26.7%. They also ended
their games with **~1,000 Faith banked**, more than the founders. Natively a
seat with no religion has almost no legal Faith sink (`Game::unit_purchase_cost`):
no religious units (the buying city's majority must be a religion it can buy
for), Builders and Settlers only under Monumentality, soldiers only under
Theocracy or a Grand Master's Chapel, buildings only the worship kind; and
`advanced_great_people` patronizes only a person already 85% earned (60% on
its own lane). The bank expires with the game.

## What was built

`idle_faith_patronage` (`PRODUCTION_OPT_INS`): a seat with no religion and
600+ Faith may patronize any offered Great Person with Faith whatever the
shortfall — the price is `150 + 10 × missing points`, so a cold purchase runs
400–700 — while gold purchases keep their gate, a founder keeps its Faith,
and a bank under 600 is not idle. One clause in the patronage loop.

## How it was measured

| what | instrument | result |
|---|---|---|
| the gene against the best genome | `gene_screen --players 6 --all-seats --baseline best --genes idle-faith-patronage`, 1,000 pairs (6,000 seat-pairs), seeds 55M; resolves ±0.6 pp win, ±0.04 pp share | **+0.5 pp [+0.1, +0.9]** win (z +2.28) · **+0.06 pp share (z +3.96)** — helps past the family-wise bar on both axes (one gene, bar 1.96) |

## What it means

The twelfth proven helper, and the cheapest so far: no new mechanism,
only a gate that assumed Faith had a better use. The ledger turns it on in
the deployment genome. The same rows point at the remaining Faith levers for
a religion-less seat — Theocracy for Faith-bought soldiers, Monumentality for
Builders — which are government and dedication choices, not patronage, and
are the next things to price.
