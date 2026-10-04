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

## Probe: culture-defense-theater and district-buildings-first-2

Both use the same probe, 16 paired seeds from 37150000.

| | culture-defense-theater | district-buildings-first-2 |
|---|---|---|
| t100 culture Δ | +3.1 (z 2.7) | −1.4 (z −1.6) |
| t150 culture Δ | +21.2, 44→65 (z 4.5) | — |
| t150 civics Δ | +1.5 (z 5.1) | — |
| t100 / t150 gold Δ | −4.3 (z −3.8) / −19.6 (z −1.9) | −3.0 (z −1.4) / — |
| foreign cities held Δ | −0.25 (z −2.2) | +0.06 |
| games with any bankruptcy, off / on | 4 / 8 | 4 / 8 |
| final score Δ | +35 (z 1.4) | +19 (z 0.45) |

`culture-defense-theater` buys culture and civics, and pays for it in Gold
and a small war cost. With domestic Tourists equal to lifetime Culture over
100, +21 Culture a turn from about t110 delays a culture victory like game
24's Sweden by four to six turns. That is a delay, not a defense.
`district-buildings-first-2` is null to slightly negative.

## The Builder drought (`builder-before-the-army`)

Games 29 and 30 (2026-10-03T090618Z and 093332Z) were the weakest economies
of the series. Game 30 held 29 population and 18.5 Science at t100 against
Persia's 61 and 88.6. Its capital sat at population 2 for 70 turns working
two 1-Food mines. Both games held no Builder for long stretches: 29 turns
from t42 in game 29, and 40 turns from t54 in game 30.

Across 25 live King games (3+ cities, to t150):

| zero-Builder share of turns | games | t100 population | t100 Science |
|---|---:|---:|---:|
| under 10% | 9 | 46 | 52 |
| 20% or more | 7 | 34 | 32 |

Weak empires both lack Builders and lag, so the table runs both ways, but the
chain is direct. At t120 the strong games had improved 54-64 of their 80-89
land plots, 15-20 of them Farms, with one bare flat plot left. Games 28-30
had improved 15-23, with 0-4 Farms and 17-22 bare grassland and plains plots.
Game 29 had all 9 cities at their housing cap.

The stock Builder step in `BasicAi::pick_item` comes after the Monument, the
capital Settler, the military floor, recon and navy. On the live board the
lent war floor (15-20 units for 5-8 cities in game 30), Walls, Settlers and
Monuments took every queue. The purchase fallback ("Buying a builder ... the
empire having none") fired once, at t46, then the Builder's rising price
passed the 120 working reserve. `builder-workforce-recovery` cannot help: its
reservation returns while a war plan is set or the strategy is Recovery,
which is when the droughts happen. A game 30 replay with it armed claimed
nothing.

`builder-before-the-army` puts the genome's own quota (`builder_per_city`,
at most one per two cities, at least one from two cities) ahead of those
steps. It stays behind the siege, barbarian and economic-recovery steps. It
fires only while there is land to improve and the city finishes a Builder
within 16 standard turns. Replaying game 30 from t55 to t80 gives Builders
where stock trained a Settler, Archers, Walls and a Heavy Chariot.

## Game 28: the late bankruptcy (`policy-deck-hysteresis-2`)

Game 28 (081800Z) went bankrupt from t211 to t233. The host disbanded its
army from 34 units to 7, and it lost a Technology victory at t236.

Net income is city Gold, minus unit upkeep, minus building and district
maintenance (`building_maintenance_total`, `district_maintenance_total`),
plus deal Gold per turn. Income fell in three steps:

1. **About −21 a turn.** Favor sold to Rome for Gold per turn stopped when
   Rome went to war with us. Rome was never our campaign or denial target.
2. **−29 a turn.** Conscription was evicted at t205. A mid-turn windfall
   (250 → 485 Gold) lifted the treasury over the emergency reserve, and no
   base conquest list names Conscription. From t206 to t208 it could not
   return, because the slotting loop protects every wanted card, so Logistics
   (rank 21) kept the slot. `policy-deck-hysteresis-2` keeps a held relief
   while income is below its discount, and lets the emergency's relief evict
   a lower-ranked wanted military card.
3. **−64 a turn from t222.** Every player's
   `public_stats.thermonuclear_devices` became 4 at an Arms Control
   congress. This is exogenous and unexplained.

The 24-game fires screen gives version 2 against version 1 −13.9 pp
(±13.5, z −1.03). That is a fires check, not a verdict. Version 1 stays
armed.

## Lead: recovery spends a city's first district slot

In `BasicAi::economic_recovery_item`, after a Trader and a gold-positive
building, the fallback is the best-gold Commercial Hub or Harbor site. It has
no build-time limit and no regard for the city's Campus. Across 13 games from
2026-10-02 and 10-03, 17 of the 19 Hubs and Harbors that became a city's
first district were placed in recovery. Most games had one or two; game 31
(100536Z) had six, including Popayán (34 turns at 2.7 production) and Cuenca
(59 turns at 1.8). Game 31's cities without a Campus held 5 Hubs at t140, and
its Science per citizen was 0.91 against the top rival's 1.73.

Simply swapping in the Campus is not clearly better in a deficit: it costs 1
Gold upkeep, while the Hub and Harbor cost none and earn adjacency Gold. The
defensible narrow rule is that a recovery district which cannot finish within
`FIRST_CAMPUS_MAX_TURNS` (15) is not recovery. It should fall through to
`upkeep_free_recovery_item` and leave the slot for later. Not yet built.

## Lead: idle trade capacity

The strong game 26 (072557Z) filled every trade slot: capacity 7 at t140,
with 6 routes running. The weak games left slots empty for long stretches:

- game 30: capacity 1 and no route from t60 to t100;
- game 34: capacity 1-2 and no Trader until t120;
- game 31: capacity 4-5 with 1-2 routes at t120-t140.

`BasicAi::should_add_trader` only needs a free slot and an open destination,
but its step comes after the floor, the Settler, the housing reserve and the
Builder. So this is the same ordering starvation as the Builders and
Campuses. Not yet acted on: the `campus-before-the-army-2`,
`builder-before-the-army-2` and `settler-before-the-navy` set goes live from
game 37, and should be read first.

Read on game 37 (131343Z), the first game under that set. Its second slot
sat idle from t40 to t100. A prototype `trader-before-the-army` (the Trader
step ahead of the floor, yielding to a due Settler) was replayed on the ten
idle-slot turns with an idle city. Nine kept their live pick: a due Settler,
Walls, an Archer, a Builder or a Campus. The only change was a Trader in
place of Bogotá's Campus at t74. Under the new order the idle slot is mostly
the cost of expanding, which is the right priority, so the gene was not
shipped.

## Result: version 2 of the build-order genes

`campus-before-the-army` version 1 starved expansion. Game 34 (113755Z) held
3 cities from t42 to t100 against a target of 10, and its first Settler came
at t88. The paired probe agreed: +1.44 techs at t100, but 4.8 fewer citizens
(z −3.19). `BasicAi::settler_gates` / `settler_due` now give every step that
must yield to a walker the Settler step's own gates. Version 2 against
version 1 (16 pairs): t100 population +4.06 (z +4.16), cities +0.88
(z +2.91), civics +0.94 (z +3.76). Chained on the same seeds, version 2
against no Campus gene comes to about +5 Science and +0.9 techs at t100, with
population, cities, civics and culture flat. `builder-before-the-army-2`
(drought only) against version 1: games last 8.1 turns longer (z +2.27).

## Housing: `granary-before-the-army` (not armed)

At t100 our cities held 4-5 citizens against the rivals' 7-9, and many sat at
their housing cap. Game 41 (155014Z) had 5 of 6 cities at housing and no
Granary. `first-granary-reserve-3` arms two Granary steps. The stock
`housing_reserve_item` comes after the floor and the Settler. The advanced
reserve runs ahead of the strategic scorer, which a delegated live seat never
reaches. `granary-before-the-army` moves the stock step ahead of the floor,
behind the Monument and the capital Settler, yielding to a due Settler.

The 16-pair domination probe against the live bundle bought +2.1 citizens at
t100 (z +2.48). It paid 23 Science (z −2.38), 0.87 Libraries (z −2.04) and
1.9 districts at t150, because the Granary step displaces
`campus-before-the-army-2`. Replays bind rarely: 0 and 3 of 10 housing-bound
idle turns, and in game 41 the siege reservation claimed the idle capital
first. The gene is not armed.

Dry sites (base housing 2) numbered 2-3 per weak game against none in game
26. The site scorer already prices water three ways: the growth forecast,
`(housing − 2) × 4`, and an early dry-site penalty. This reads as map
scarcity, not a missing term.

## `campus-before-the-army-3` (not armed) and version 2 confirmed

Version 3 carries the chain on to the University and the Research Lab.
Against version 2 it scored −51 (z −1.87), with Science −12, Culture −10 and
Gold −8.6 at t150. A 72-game confirmation screen of the armed version 2
(seeds 31004000) gave +1.1 pp (z +0.26). The earlier 24-game −14.7 pp
(z −2.90) was noise.

## Where the economy stands (2026-10-04 early UTC)

Armed from game 36/37 on: `builder-before-the-army-2`, `campus-before-the-army-2`,
`settler-before-the-navy`, `policy-deck-hysteresis-2`, plus `culture-defense-theater`
(with the Campus and Settler gates) and `upkeep-reserve`.

| game | t100 cities / pop | t100 Science vs best | later |
|---|---|---|---|
| 37 (131343Z) | 9 / 40 | 72 vs 95 (0.76) | 138 vs 174 at t150; Technology loss t238 to a 15-city, 200-pop runaway |
| 44 (025448Z) | 8 / 42 | 59 vs 69 (0.85) | **191 vs 133 at t175, Science lead**; Culture loss t194 to Nubia |
| 46 (033533Z) | 9 / 52 | 71 vs 106 (0.67) | lost 2 cities by t150, 69 vs 179 |

The t100 economy is no longer what loses these games. Game 44 led Science, ran
+67 Gold a turn and had no bankrupt turns. Its own domestic Tourists (75-77)
sat at the culture bar. It still lost the culture race to Nubia, whom the
campaign had targeted from t130. Nubia grew from 5 to 9 cities while our
sieges stood in Stage with nothing staged. Across today's runs, 26 Siege
streaks lasted 8 or more turns with "0 of N units staged". The force shrinks
on about one staging turn in three everywhere (the drain), but the empty
streaks carry larger forces than average. So they are a convergence problem
for the siege lane, and the next lever is there, not in the economy.
