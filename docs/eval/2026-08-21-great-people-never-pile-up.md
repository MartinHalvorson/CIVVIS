# Great People never pile up

_2026-08-21 · `agent/mbp-m5-pro-64/claude-fable-greatpeople` · PR #2248_

## What was asked

Operator, 2026-08-21: add a heuristic gene for better management of our
Great People — never let Great People pile up in a city unused; for Artists
and Musicians produce Great Works, and if there is no space either build
space or sell a few existing works to make space; the other classes should
not pile up either. Test it and merge it into main.

## What piling up is, in this engine

Great People are recruited and expended the moment they are claimed
(`Game::claim_great_person`), so the native form of a person parked unused
is **points sitting at or over the price while the claim is refused**. The
refusals are concrete (`Game::validate_great_person_activation`): a Writer
needs two open Writing slots, an Artist three Art slots, a Musician two
Music slots (a Broadcast Center holds one); a Scientist needs a Campus; every
second Engineer — Imhotep, Eiffel, Tesla, Roebling, von Braun — needs a
wonder under construction to land 2 × 175 … 1,400 production on; a General
needs a promotable land unit, a formation Admiral a ship. Meanwhile the
named person is a global race: the first seat to afford and satisfy the
claim retires them, and the next one costs more.

A probe over six native 6p games with the deployment genome (36 seats,
~7,000 seat-turns; `zz_probe_great_person_pile_up`, seeds 70M) counted the
seat-turns a class sat affordable and refused:

| class | blocked seat-turns | longest streak | seats ending the game still blocked |
|---|---|---|---|
| Engineer — needs a wonder under construction | 198 | 51 | 5 of 36, at **14.6 ×** the price |
| Writer — 2 open Writing slots | 113 | 18 | 2, at 2.2 × |
| Musician — 2 open Music slots | 82 | 32 | 1 |
| Admiral — a military sea unit | 75 | 28 | 1 |
| Artist — 3 open Art slots | 3 | 3 | 0 |

Nothing in the deployed agent answered those blockers on a native board:
`production_value` vetoes every Great Work slot building at −10,000 for any
lane but Culture, so the Writer points a Theater Square or a wonder yields
to a Science seat can never be spent; and the physical-person helper
(`BasicAi::prioritize_live_great_person_activation`) reads only the host's
`live_great_person_activation_needs`, which a native game never fills.

## What was built

`great-person-housing` (`PRODUCTION_OPT_INS`, `src/ai/advanced/great_person_housing.rs`),
a ladder run once a turn before strategic production fills the queues:

1. **Build space ahead of the person.** A class whose points are at, or
   within fifteen turns of, the price while the claim is blocked reserves
   one city for what lifts the block, read from the offered person's own
   effects: the typed slot building for a Writer, Artist or Musician (or the
   nearest missing step of its chain — museum before Broadcast Center,
   Amphitheater before museum); the district for a Scientist, trade
   Merchant, trade Admiral or Prophet; the cheapest available wonder for a
   wonder Engineer; a soldier or ship for a General or formation Admiral.
   Districts and wonders are reserved only once the person is due. The
   reservation takes an idle city or one on a repeatable project; a due
   person may also pause an ordinary building (progress is kept per item),
   never a unit, district, wonder, or the plan's threatened city. One
   reservation per class at a time.
2. **Sell to make room.** A due cultural person that no city can build for
   sells duplicate works of the kind it makes through Quick Deals — which
   quotes only genuine duplicates, never a last copy — and recruits the same
   turn: two new works for one sold, plus the Gold, and the race won rather
   than forfeited. Never to the plan's target or a rival past 60% of a
   victory, never on a mirrored seat.

Nine behaviour tests pin the ladder (`a_writer_at_the_price_…`,
`the_slot_building_starts_within_the_lead_…`, `a_due_person_pauses_a_building_…`,
`a_due_writer_no_city_can_house_sells_…`, `a_buildable_slot_outranks_a_sale`,
`districts_are_reserved_only_for_a_due_person_…`,
`a_due_wonder_engineer_starts_the_cheapest_wonder_…`,
`a_due_formation_admiral_is_answered_with_a_ship_…`,
`great_person_housing_is_a_native_opt_in`).

## How it was measured

| what | instrument | result |
|---|---|---|
| the pile-up itself, gene off v on | the probe above on the same seeds, every major carrying the gene | PROBE_RESULT |
| the gene against the best genome | `gene_screen --players 6 --all-seats --baseline best --genes great-person-housing`, 1,000 pairs (6,000 seat-pairs), seeds 75M | SCREEN_RESULT |

## What it means

MEANING
