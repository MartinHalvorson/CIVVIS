# Live economy census: what the King cities build

Session -ff, 2026-10-03. This is a census of the live seat's economy against
the Firaxis rivals, plus the three opt-in `BasicAi::pick_item` genes it
produced: `monument-first`, `district-buildings-first` (with version 2) and
`culture-defense-theater`.

## Data

The data is 14 live King Gran Colombia Domination games (4 majors, Tiny
Pangaea, Online) from 2026-10-01 and 10-02, plus games 21 and 22 of 10-03.
Each turn is read from its first `state` row (frame 0) and the `tiles`
exports. Rival numbers come from `rivals[].public_stats`, the host's own
figures. Our numbers come from the same `public_stats` block, so both sides
are measured the same way.

## Where the gap is not

The table compares us with the rivals at each checkpoint. The rival
production column includes King's +20% production bonus
(`data/difficulties.json`: King AI +20% production and gold, +8% science,
culture and faith).

| per citizen | t60 ours | t60 rivals | t100 ours | t100 rivals |
|---|---:|---:|---:|---:|
| production | 2.16 | 2.91 | 2.06 | 2.66 |
| food | 2.74 | 2.79 | 2.70 | 2.68 |

With the 20% removed, production per citizen is at parity (2.66 / 1.2 =
2.2). Food per citizen is also at parity. The citizens already work the best
plots they own: across every city at turns 30-100, the number of worked tiles
that an unworked owned tile beats outright (at least as much food, at least as
much production, and more in total) is 0 or 1 at every checkpoint. The seat
never issues a citizen order, so this is Civilization VI's own governor, and
it does fine. About 22% of our owned land is improved, against 24% of the
rival land we can see. Luxuries are improved.

The frame-0 "idle" production (about 20% of production) is an observation
artifact. Queues refill inside the turn, and in game 21 only 0.6% of
production was idle at each turn's last frame.

## Where it is

| t100 (median of 13) | ours | best rival |
|---|---:|---:|
| cities | 9 | 6 |
| population | 41 | 49 |
| science | 41.9 | 98.1 |
| culture | 24.8 | 93.0 |
| gold / turn | 9.8 | 47.4 |

- **What the cities build.** `pick_item` tries every district a city still
  lacks before it tries any building. At t100, 15 of 45 Campuses had no
  Library, 13 of 19 Commercial Hubs had no Market, and 4 of 4 Industrial Zones
  had no Workshop. At t120, 23 of 41 Commercial Hubs had no Market and 8 of 13
  Entertainment Complexes had no Arena. Game 21, the best of the series, had
  7 Campuses and 2 Libraries at t100.
- **Monument.** From t60 to t100 only 54-57% of cities older than 20 turns
  held a Monument (27% at t30). In a new city's first 40 turns, 3.5% of the
  build went to the Monument, 6.9% to Walls (`barbarian_defense_item`) and
  about a quarter to soldiers and ships.
- **Culture.** Our culture ran a third to a half of the strongest rival's.
  The 14 games built one Theater Square between them, because
  `theater_square` is the last `DISTRICT_PRIORITY` family in every bred
  genome.
- **Gold.** Gross gold at t100 was about 38 a turn. Unit upkeep took about
  21 of it.
- **Housing.** `first-granary-reserve-3` works. Population at t100 was 48
  with it against 29 without (median of 6 and 7 games), and the share of
  housing-capped cities fell from 89% to 50%. Granary coverage was still only
  27% at t60.

The culture-victory rule matters for reading these numbers.
`Game::check_culture_victory` gives the win to a seat whose foreign Tourists
exceed the highest domestic count among the other majors. Domestic Tourists
are lifetime Culture divided by 100. So our Culture is the binding defense only
when ours is that highest count. That held in 1 of the 4 culture losses
checked (2026-09-30T221624Z: ours 53, and Byzantium won at about 51).

## Probe

The probe is `victory_eval --domination-pair <gene> --games 16 --start-seed
37150000`. The focal seat is Gran Colombia Domination with the compiled live
force-on bundle, at King, on 4p 60×38 Pangaea, with only seat 0 toggled. The
economy snapshots come from c7ab10121. Each cell is the paired mean Δ (on −
off) over 16 seeds, with z in brackets. The rivals are adaptive CIVVIS AIs,
not the Firaxis AI, and they win Science around t190 in both arms.

| | monument-first | district-buildings-first |
|---|---|---|
| t60 culture | +3.1 (z 3.3) | 0 |
| t60 Monuments | +1.25 (z 2.6) | 0 |
| t60 techs | −0.5 (z −1.6) | 0 |
| t100 civics | +0.8 (z 2.5) | −0.06 |
| t100 gold | +5.4 (z 2.7) | +0.9 |
| t100 science | +4.1 (z 1.4) | +0.8 |
| t100 Libraries | +0.3 | +0.38 (z 2.4) |
| final score | +26 (z 0.8) | −29 (z −1.0) |
| wins on/off | 0 / 0 | 0 / 0 |

The t150 snapshots are dominated by war outcomes, with pairs gaining or losing
3-5 cities in both directions, so they are not read as an economy effect.

`district-buildings-first` barely binds. Its step comes after the military
floor, the Settler, the Builders and the Monument, so few cities reach it.
Version 2 gives the capital's Library the slot `campus-before-harbor-2` gives
its Campus, ahead of the next Settler. That is the slot where game 21 lost its
Libraries.

## Game 21: the late bankruptcy spiral (`upkeep-reserve`)

Game 21 (2026-10-03T040354Z) was the best game of the series. Kongo won a
Technology victory at t216, with a score of 1363 against our 1304. Tech was
level at 64 each at t190. Then our science fell from 301 to 178 by t210, while
Kongo flew from 2 to 25 science-victory points.

What caused the drop was bankruptcy and amenities, not research:

| turn | gold | gold/turn | unit upkeep | bankruptcy amenity points | science lost to amenities |
|---|---:|---:|---:|---:|---:|
| 179 | 155 → 30 (an upgrade) | +22 | 137 | 0 | — |
| 180 | 0 | −108 | 204 | 56 | — |
| 184 | 0 | −132 | 204 | 120 | about 100 |
| 207 | 0 | −128 | 272 | 68 | — |
| 208 | 0 | −66 | 201 | 102 | about 140 |

The chain runs like this:

1. Both upgrade passes spend down to a flat 30 Gold at war. These were
   `modernize_before_the_purchase_pass` (`WARTIME_UPGRADE_FLOOR`) and
   `upgrade_units_preserving_air_wing`. The seat made no Gold purchases at all
   in t170-215; every treasury drop was an upgrade (299→69, 459→104, 408→88).
2. The army's bill is 130-220 Gold a turn. When the deck loses Levée en Masse
   for a turn (−2 Gold per unit; a government change at t181, a re-ranked
   military slot at t189 and t207), the bill jumps 60-70 at once.
3. With 30 Gold in hand, that swing bankrupts the empire. Bankruptcy's Amenity
   penalty then cuts every yield, Gold included, by 20-40%. So the deficit
   outlives the swing.

`upkeep-reserve` makes both upgrade passes keep 1.5 turns of the unit bill
(`Game::unit_gold_maintenance`, the host's own figure when it is exported)
instead of 30 Gold. On the game-21 numbers, the t179 upgrade (125 Gold against
a 205 floor) would have waited, and about 177 Gold would have covered the t180
swing.

The same game also lost the race on target selection. That belongs to the war
lane and was handed to session -9c. We made a stall-peace with Kongo at t136,
at 2.7× its power. Our army never fought Kongo again while Kongo launched
every space project from t178 to t198. At t186 the seat also proposed a
Research Alliance to Kongo.

## The policy deck's churn (`policy-deck-hysteresis`)

Game 21's request and readback log (`policy_deck_request` and
`policy_deck_readback` events) shows the host applying every deck it is sent.
The churn is therefore the seat's own, and it has two forms.

1. **Levée en Masse in and out.** From turn 160 to 216 the deck alternated
   between Levée en Masse and Lightning Warfare, Total War, Strategic Air Force
   or Propaganda, every turn or two. `strategic_policies` keeps a maintenance
   relief card only on a maintenance emergency. That emergency holds while
   income is positive only if the deck already `retained` the card. But the
   base governor's `revise_policy_deck` runs first in the turn, and on its
   review turns it can unslot the card. At t206 the relief therefore read as
   gone, at 33 Gold and +2.9 a turn, and the wants list dropped it. The next
   frame's bill was 272 against 206, and the treasury hit zero. The journal's
   "Slotted X over Scripture / Colonial Taxes / Finest Hour" lines name cards
   the base pass had just slotted. None of them was in the host's deck.
2. **Aesthetics and Liberalism alternating** every turn from 110 to 132. The
   Liberalism repair rule fires while two developed cities show an Amenity
   deficit. Liberalism fixes that deficit, so the rule stops firing, Aesthetics
   comes back, and the deficit returns.

`policy-deck-hysteresis` makes `AdvancedAi` remember the deck the turn began
with (the host's deck on the live board) and counts a relief card held there
as retained. Once Liberalism is slotted, a two-district city at 0 Amenities
still calls for it, since Liberalism is exactly +1 in those cities.

## Probe: the bankruptcy chain

The probe is `victory_eval --domination-pair <gene> --games 16 --start-seed
37150000` with the bankrupt-turn counter (8fc914742, schema 4).

| | upkeep-reserve | policy-deck-hysteresis |
|---|---|---|
| pairs whose actions changed | 7 / 16 | 8 / 16 |
| bankrupt turns, off / on | 61 / 56 | 61 / 56 |
| games with any bankruptcy, off / on | 4 / 3 | 4 / 3 |
| final score Δ | −6.6 (z −1.1) | −0.75 (z −0.1) |
| t150 treasury Δ | +1.6 | +9.5 (z 1.6) |

Both genes are neutral in the simulator, with a small reduction in
bankruptcy. The simulated seat rarely meets the live trigger. On the live
board three things combine:

- a host-exported unit bill of about 200 Gold,
- upgrade drains down to 30 Gold,
- the base pass unslotting the relief card.

The case for arming both rests on the live mechanism measured in game 21.
Both genes do nothing until they bind.
