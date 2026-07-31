# Fidelity: making CIVVIS an exact match for Civilization VI

CIVVIS started as a Civ-6-*like* engine: rules transcribed by hand from the
Civilopedia and the wiki, tuned until games felt right. That is enough to train
an agent that is superhuman *at CIVVIS*. It is not enough for the goal this
project is actually aimed at — an agent whose policy transfers back to the real
game. A policy trained against wrong unit costs learns wrong build orders.

This document defines what "exact" means here, how it is measured, and what is
built next.

## What exactness can mean

Civilization VI's rules live in a closed DLL. Bit-identical random streams are
unattainable and unnecessary. The achievable contract has three clauses:

1. **Identical legal-action sets.** In any state both engines admit the same
   moves. This is the clause a trained policy is most sensitive to: an action
   CIVVIS allows and the real game forbids is a habit that transfers as a
   blunder.
2. **Bit-identical deterministic transitions.** Yields, costs, adjacency,
   movement, growth, research — every transition with no randomness in it
   agrees exactly, over a canonical state schema.
3. **Distribution-identical stochastic transitions**, plus forced-outcome
   replay: given a logged real game's random outcomes, CIVVIS reproduces it
   step by step.

Everything below is machinery for enforcing those three clauses. The order
matters — clause 2 is cheap to test statically and catches the largest bug
class, so it comes first.

## Phase 1 (shipped): rules data measured against the game database

The game ships nearly every rules constant as readable XML under
`Base/Assets/Gameplay/Data`, with expansion overlays in `DLC/Expansion1` and
`DLC/Expansion2`. Those files are the authority — the wiki is a secondary
source that lags balance patches, and CIVVIS' data was demonstrably carrying
pre-Gathering-Storm numbers because of it.

`tools/civ6_fidelity.py` loads that database in the game's own load order,
projects it onto CIVVIS' schema, and reports every divergence:

```sh
python tools/civ6_fidelity.py                      # markdown report
python tools/civ6_fidelity.py --max-divergences 0  # CI ratchet
```

It needs a local installation (auto-detected, or `--civ6` / `$CIV6_DIR`). It
reads the game files and never copies them into the repository; only the
divergence report is an artifact, which keeps the audit reproducible without
redistributing Firaxis data.

Two loader details are worth knowing, because getting either wrong silently
produces a clean-looking but false report:

- The XML uses attributes (`<Row Cost="80"/>`) and child elements
  (`<Set><Cost>730</Cost></Set>`) interchangeably. Handling only the first
  spelling drops every expansion rebalance and makes the base game look like
  the current ruleset.
- Each expansion ships a plain overlay and a `_Major` overlay per table; the
  `_Major` pass applies afterwards.

**Result of the first run:** 55 real divergences across units, technologies,
civics, buildings and districts. All are now resolved — 31 by correcting
CIVVIS' data, the rest by recording them as deliberate:

| Fixed | Examples |
|---|---|
| Gathering Storm rebalances CIVVIS had at vanilla values | Knight 180→220 production and 3→4 maintenance, Pikeman 200→180 and 3→2, Military Engineer maintenance 4→2, Shipyard maintenance 2→1 |
| Modern-era building costs, uniformly overstated | Factory/Stock Exchange/Military Academy 390→330, Research Lab/Seaport/Broadcast Center/Film Studio/Shopping Mall 580→440, Stadium 660→480, Airport 600→480, Zoo/Aquarium 445→360, Hangar/Food Market 465→380 |
| Sight radii left at the default | Spy, Naturalist, Helicopter, Rocket Artillery, Giant Death Robot all see 3 tiles, not 2 |
| Missing prerequisites | Cartography and Mass Production both require Shipbuilding |

`tools/fidelity_waivers.json` holds the accepted divergences, each with a
reason — Future-era techs and civics draw randomized prerequisites in
Gathering Storm, purchase-only units store a Faith price where the database
stores an unpayable production cost, the City Center is placed rather than
produced. **That file is the fidelity roadmap: shrinking it is the work.**
Anything not listed there counts against the ratchet, which now stands at
zero for these five tables.

**Second wave (terrain layer):** the audit now also projects `Terrains`,
`Features`, `Resources` and `Improvements` — yields, movement, defense
modifiers, passability, housing, valid terrain/feature/resource placement,
reveal prerequisites, and the tech/civic-gated improvement upgrades of
`Improvement_BonusYieldChanges` against `tree_effects.json` — and loads the content packs a standard all-content
game enables (civilization, leader and landmark packs; scenario and
optional-mode data stays out). Where the game enumerates variant rows the
projection folds them onto CIVVIS' spelling: hills rows become the engine's
single hills rule, `*_MOUNTAIN` rows the one impassable mountain terrain,
lake plots the coast rows they really are, and an all-land enumeration means
"no terrain restriction". The wave surfaced 88 more real divergences, all
resolved. The largest:

| Fixed | Detail |
|---|---|
| Movement was max-based, the game's is additive | `move_cost = terrain + hills + feature`: Woods on Hills is 3 MP, not 2. Feature data now stores the database's `MovementChange` |
| Floodplains carried vanilla values | Desert floodplains 3→2 Food; grassland/plains floodplains add no yields at all; all three impose −2 defense like Marsh |
| Reef defense was on the wrong feature | The bonus Reef grants +3, Great Barrier Reef grants nothing |
| Pamukkale modeled as tile culture | Its real effect: +1 Amenity to the owning city, +1 more while its plot is adjacent to an Entertainment Complex |
| 32 resources missing | Every Gathering Storm luxury and bonus resource now exists with exact class, yields, placement and improvement — including the manufactured four (Toys, Jeans, Perfume, Cosmetics) no tile improvement works |
| Wrong valid-placement lists | Wheat is plains-only (plus floodplains), Stone grassland-only, Sheep spawn on grassland too, Uranium anywhere including snow; camps/mines/quarries/plantations/fishing boats accept their full resource sets |
| Oil Wells unlocked two eras early | Steel → Refining, as shipped |

The "Only in Civ VI" column measures scope rather than error — the units and
buildings CIVVIS does not model are almost all civilization uniques from DLC
packs, and the missing features are natural wonders plus the volcano system.
That column is the content backlog.

**District adjacency (parallel session):** `District_Adjacencies` joined to
`Adjacency_YieldChanges` against each district's per-source `adjacency` map,
dividing every rule's yield by its `TilesRequired`. It surfaced one wrong
Industrial Zone Mine rule, now fixed. This projection is the case that
justifies the whole approach: reading the XML by hand said Wonder adjacency
was +1 Culture, because the base row says `YieldChange="1"` and Rise and
Fall raises it to 2 through a separate `<Update>` element. The loader
applies overlays; eyes skimming a dump do not.

**Third wave (content layer):** `Wonders` (cost, prereqs, yields, housing,
amenities, regional ranges, great-work slots, great-person points, and the
whole placement predicate — terrain, required features, coast/lake/river,
adjacency), richer `Buildings` (housing, amenities, citizen slots, yields,
districts, great-work slots), `Policies` (slot + civic), `Governments`
(slots, influence, prereqs), `Beliefs` (classes), `UnitPromotions`
(class/tier/prerequisite trees), and `Projects` (costs, districts, GPP,
yield conversions, cost progressions). Policies, governments and beliefs
were already exact; the rest surfaced 62 divergences, all resolved:

| Fixed | Detail |
|---|---|
| Naval Raider and Carrier promotion trees rearranged | Loot is tier 1 with no prerequisite, Homing Torpedoes tier 2, Silent Running tier 3, Wolfpack tier 4 — plus five wrong prerequisite lists (Armor Piercing, Hangar Deck, Folding Wings, Observation, Swift Keel) |
| Reactor-era project costs | Coal/Oil/Uranium conversions 300/360/480 → 200/300/400, Recommission Reactor 200 → 400, Operation Ivy 1200 → 1000 |
| Gathering Storm building buffs missed | Palace grants 2 Amenities (not 1), Biosphère +8 Science, Prasat 2 Relic slots and 4 Faith, Sukiennice 3 Gold, Tlachtli 1 Culture |
| Jebel Barkal double-counted | Its +4 Faith reaches every city within 6 tiles including its host; CIVVIS carried a local copy on top of the regional effect |
| Estádio do Maracanã was local | The game gives its 6 Culture and 2 Amenities to every city in the empire (regional range 100000) |
| Improvement siting was intersection-based | Civ 6 sites improvements through any of three routes — valid terrain OR valid feature OR valid resource. Farms on desert Floodplains and flat resource mines now place exactly as shipped |

**Fourth wave (triggers, villages, eras, constants):** `Boosts`, `GoodyHuts`,
`Eras`, `GreatPersonIndividuals` and a curated `GlobalParameters` mirror are
audited — 22 tables at zero unwaived divergences. The finds this wave:

| Fixed | Detail |
|---|---|
| Nearly every Eureka/Inspiration was wrong | 109 of ~110 boost entries corrected to the shipped trigger, count and target; the engine trigger vocabulary grew from 16 to 45 forms (meet a civilization, improve a specific resource, government slot counts, continents, alliances, promotions-class kills…), several data triggers that silently never fired now do, and the boost grant is per-row data (40%, Near Future Governance 90%, +10 points for China) |
| Tribal villages used a lean 4-outcome table | The shipped seven-category, 22-reward table with exact weights, turn gates, city gates and amounts now drives rewards (`data/goody_huts.json`); Gilgamesh's Epic Quest rolls the same table |
| Embarked strength was flat 10 | It climbs the shipped era ladder (10/15/15/30/35/50/55/55) via `data/eras.json` |
| Border growth used a homebrew curve | Borders now grow on the city's Culture against the shipped cost curve, 10 + 6 × plots^1.3 (was 15 + 8 × plots fed by 1 + Culture/2) |
| Great Person costs | Recruit costs follow the shipped per-era ladder (30…1320); two prophets carried invented prices |
| Constants verified | Growth curve/thresholds, housing bands, fresh-water housing, amenity demand (GS zeroes the free Amenity), city spacing, corps/army bonuses, amphibious/river combat modifiers, barbarian XP caps — and the damage roll: CIVVIS' 30·e^(Δ/25)·U(0.8, 1.2) is the same distribution as the shipped 24 base with its 1.0–1.5 spread |

**Fifth wave (the unit lifecycle):** `UnitUpgrades` and the `Units` column
`MandatoryObsoleteTech` are now projected alongside every other unit field,
which closed the largest remaining behavioural hole in the audit: CIVVIS
carried neither, so no unit ever retired and no unit ever upgraded. An
Information-era empire still fielded — and still trained — Slingers.

| Fixed | Detail |
|---|---|
| No unit ever became obsolete | 33 units carry the shipped `MandatoryObsoleteTech`; researching it removes the unit from every production and purchase menu and from every queue |
| No unit could ever be upgraded | 52 units carry their shipped `UnitUpgrades` successor, reachable through the new `upgrade_unit` action |

The Gold price is the one number this wave could not read from the database:
`UPGRADE_BASE_COST` (10) and `UPGRADE_MINIMUM_COST` (15) are shipped
GlobalParameters, but the per-Production factor lives in the executable. The
engine charges the community-documented `10 + 2 × Production difference`,
which reproduces the in-game prices those two parameters bracket.

**Sixth wave (bands, maps, routes, spawns):** `Happinesses`, `Maps`, `WMDs`
and more of `GlobalParameters` join the audit — 25 tables at zero unwaived
divergences. The finds: the amenity bands were wrong twice over (thresholds
from an invented ≥5 tier, values from the base game the expansions
rebalance — the shipped ladder is +20/+10/0/−15/−30/−100 growth and
+20/+10/0/−10/−20/−30/−40 non-food); roads now follow the shipped route
ladder (1 MP until Industrial-era routes at 0.75, Modern at 0.5) and bridge
rivers with Classical-era (medieval) routes instead of a technology;
resource map placement follows the union rule with hills-only (Sheep,
Copper, Iron, Coal, Diamonds) and flat-only (grains) forms honored — a
regression where feature-listed resources became feature-*only* is fixed;
barbarian camps keep the shipped placement floors (4 from cities, 7 from
camps); nuclear device stats live in `data/wmds.json` with maintenance
charged from data; the Cultural Heritage Inspiration now fires via a
full-museum theming proxy; and Gilgamesh aside, the six map-size profiles
and the alliance-leveling timeline verified exact.

**Seventh wave (routes per tile):** `Routes` joins the audit — 27 tables
at zero unwaived divergences. Roads are leveled per tile on the shipped
PlacementValue ladder: traders lay the best route their civilization's
era allows (Medieval from Classical, Industrial and Modern from their
namesake eras), each step is priced by the destination tile's route
(1 / 0.75 / 0.5 / 0.25 MP), river bridging is a route property (Medieval
and later, `SupportsBridges`) instead of a world-era check, and Military
Engineers lay Railroads for 1 Iron and 1 Coal once Steam Power is in —
no build charge, exactly as `Routes_XP2` and `Route_ResourceCosts`
price them.

### Data the engine never reads

`tools/civvis_inert.py` joins the other direction: every effect key in
`data/*.json` against the engine that should consume it. Nothing enforces that
join, so a key can sit in the data doing nothing -- mistyped, refactored away,
or dropped by a rebase in a shared checkout. The last is not hypothetical: the
Sphinx's Floodplains Culture and Wonder-adjacency Faith survived in data for
fourteen iterations after their engine arm was lost, and no test noticed
because none covered them.

It reports zero unwaived keys across 629, with five waived in
`tools/inert_waivers.json` for consumption the string join cannot see. Run it
after any refactor that moves yield code.

### Next inside phase 1

Known simplifications not yet expressed in data: civic-gated valid terrain
(farms on Hills at Civil Engineering, already era-exact through
`tree_effects`' hill_farms), wonders' widening `Building_ValidFeatures`
rows (CIVVIS is the permissive side there), theming (complete: works are
era-and-creator pieces, museums theme on the shipped rules — three
artists for art, three origin civilizations for artifacts — with the
+100% bonus), barbarian camp spawn cadence (the odds/boldness
model is DLL-side; placement floors and distances are exact), WMD delivery detail
(the `WmdStrike` action launches on the shipped ranges, radii and fallout;
the per-ring unit damage is the one number the database does not carry),
and the meteor-strike goody site.

### Open gaps with a known shape and unknown constants

Two Gathering Storm rules are identified but deliberately unimplemented,
because their shape is documented and their numbers are not:

- **Flood fertilisation.** A flood does not only damage: each flooded
  Floodplains tile has a chance to gain +1 Food and/or +1 Production
  permanently, which is what makes floodplains the most fertile land in a long
  game. A Dam prevents the damage and halves the fertilisation. CIVVIS models
  the damage and the Dam's protection but not the fertilisation, because the
  per-floodplain-type probabilities live in a table only an installation can
  settle. Sources also disagree on the Dam itself — one has it halving the
  fertilisation rate, another has it preventing floods and therefore the
  fertilisation entirely — so even the qualitative interaction is unsettled.
  Climate Phase IV stops floods fertilising at all and Phase V begins stripping
  those yields back off, so the whole late-game desertification arc hangs off
  this one missing mechanic.
That one is recorded here rather than approximated: a wrong constant in a rule
that compounds over a game is worse than a missing one, and it would not show
up in any test.

- **Leader AI bias traits.** Each civ carries the leader's Civ VI preference
  traits (`aggressive_military`, `expansionist`, `mediterranean`, and so on) in
  `data/civs.json`, and the loader validates them, but nothing in the engine or
  the bundled AI reads them — `Game::leader_trait` has no caller. In Civ VI
  these seed a leader's hidden agendas and colour its diplomatic and military
  disposition. That is AI *behaviour* rather than a deterministic rule (it does
  not change any legal-action set or transition), so it sits outside the
  FIDELITY contract's three clauses; it is recorded here as a known-unmodelled
  Civ VI system rather than fabricated, since inventing disposition weights
  would not match the shipped AI and no source publishes them.

The Canal gap next to it turned out to be a wrong *shape*, not a missing
constant, and is now fixed — see the row below.

### Rules the database does not carry

The audits above reach every number the shipped XML holds. A second class of
rule lives only in the executable — how pressures are combined, when a rate
applies, what a state machine does between turns — and no projection can see
it. Those are audited subsystem by subsystem against the Civilopedia and the
community measurements that reproduce the DLL's behaviour, one system per
pass, with a regression test per correction. Findings so far:

| System | Divergence | Fix |
|---|---|---|
| Loyalty pressure | A Capital's extra pressure was a flat +1 per Citizen instead of a second copy of its own pressure at the same distance — an order of magnitude short next door to the Capital | `w += pop × (10 − distance)`, at an Age factor of 1.0 as shipped |
| Free Cities | The single largest structural divergence in the Loyalty system: a city out of Loyalty changed hands on the spot, going straight to whichever civilization happened to be pressing hardest that turn. Civ VI has it revolt into an independent Free City that keeps everything it has built, stands alone, and only later joins whoever courted it hardest **over the whole of its independence** | a dormant "Free Cities" seat is created with the world and wakes when a city revolts; `declare_free_city` evacuates the losing empire's units, hands the city over intact and raises two defenders; the Free City runs its own Loyalty each turn with the +10 an independent city holds against its neighbours, banking every civilization's pressure in `City::independence_pressure`; at 0 Loyalty it joins the argmax of that ledger, and the seat sleeps again once it holds nothing. A Free City is at war with everyone from the moment it revolts |
| Culture Bomb | A Culture Bomb claimed only tiles nobody owned, which is the one thing it is not for. Civ VI takes adjacent tiles **from other civilizations**, and several rules hang on that theft — Poland's whole leader ability is written about the city that loses the tile | `culture_bomb` now takes any adjacent tile whose owner is another civilization, skipping only City Centers and districts, and removes the tile from the loser's worked list |
| Dead effect reads | Five engine clauses read an effect key no ruleset row provides, so each read 0.0 forever while looking like an implemented rule: a Governor Spy-defence bonus no promotion carries (Victor's six are Redoubt, Garrison Commander, Defense Logistics, Embrasure, Air Defense Initiative and Arms Race Proponent), a building starting-experience grant no building carries (Barracks, Stable, Armory and Military Academy all grant a percentage instead), and three aliases for rules already delivered under another name — `empire_tile_appeal` beside the Eiffel Tower's `empire_appeal`, `gold_per_citizen` beside Reyna's `gold_per_pop`, and `empire_amenity` beside the regional-range route the Estádio do Maracanã uses | all five removed, and `tools/civvis_dangling.py` now fails the build if another one appears. It is the mirror of `civvis_inert.py`: that tool finds data with no consumer, this one finds a consumer with no data |
| Air interception | The formula the tests measured was not the formula the game fought with. `air_interception_strength` omitted the mutual-support term (+5 for every same-kind interceptor covering the same tile) that `resolve_air_interceptions` applied, so eight assertions were pinning an obsolete calculation and the live path could have drifted unnoticed | both now read one `air_interceptors` helper, and a regression test pins the support term through the query the tests use |
| Hungary's Huszár | Hungary carries **two** unique units in Civ VI — the Huszár is the civilization's, the Black Army is Matthias Corvinus' leader unit — and only the Black Army had shipped | the Huszár joins: a 335-Production Cavalry of 65 Strength needing 10 Horses, with +3 Combat Strength for every Alliance Hungary currently holds (lapsed alliances stop counting) |
| Sweden's Open-Air Museum | Sweden is one of the few civilizations with **two** pieces of unique infrastructure — the Queen's Bibliothèque building and the Open-Air Museum improvement — and only the building had shipped. The improvement was absent from the ruleset entirely | added as a Builder improvement unlocked by Nationalism, buildable on Snow, Tundra, Desert, Plains or Grassland, one to a city: +2 Loyalty per turn, and +2 Culture and +2 Tourism for each distinct terrain type Sweden has founded a city on |
| Unique units shipped as bare stat-lines | An audit of all 40 unique units against the engine found twelve with no code site at all — correct for the ones whose Civ VI text is only "stronger/cheaper", wrong for the ones carrying real clauses. Four are now implemented: the **Khevsur** (+7 Combat Strength on Hills and no Hills Movement cost, +5 against anti-cavalry), the **Berserker** (+10 attacking, −5 defending, and 4 Movement when it starts a turn on hostile ground), the **Ngao Mbeba** (+10 defending against ranged fire, and it moves and sees through Woods and Rainforest) and the **Janissary** (a free promotion, a two-Citizen minimum to train, and one Citizen spent when the training city is one the Ottomans founded). The Berserker needed two new posture helpers, applied at all four attack paths and the melee defence, because "when attacking" must also count against a city. A second round added the **Hul'che** (+5 against a wounded opponent), the **Redcoat** (+10 off its capital's continent, and no disembark cost), the **Mandekalu Cavalry** (Gold equal to the whole Combat Strength of whatever it kills) and the **Winged Hussar**, which drives a beaten defender off its hex — directly away where it can retreat, and for 10 extra damage where it cannot. Four units remain deliberately stat-only: the Minas Geraes and Domrey really are just better numbers, the Bireme's clause is its stat-line, and the Sea Dog's naval-raider stealth needs a per-unit visibility class CIVVIS does not model |
| Great General aura, and the Hetairoi | CIVVIS models Great People as instant-effect recruits — claiming a Great General applies its named effect (a promotion level, a formation) to existing units and consumes it — rather than as persistent map units with a Combat Strength aura. So the +5 CS / +1 Movement Great General aura, and any "when adjacent to a Great General" clause (the Hetairoi's +5), have nothing to attach to. That is an architectural choice, documented here rather than reworked. What IS implementable of the Hetairoi — its "starts with 1 free Promotion" — was missing and is now wired: a trained Hetairoi gets exactly enough XP to expose its first promotion, alongside the Embrasure governor path. (The Basilikoi Paides was verified to grant +25% XP, not a free promotion, so it stays as-is.) |
| Feudalism farm food | Civ VI's Feudalism gives a Farm a **flat +1 Food** when it has two or more adjacent Farms — a threshold. CIVVIS paid `floor(adjacent / 2)`, so a Farm ringed by four other Farms earned +2 and one ringed by six earned +3, over-rewarding dense farm blocks. (Replaceable Parts, which pays +1 per adjacent Farm, was already right and stays.) | the Feudalism term is now a single +1 gated on `adjacent_farms >= 2`; a regression test pins one-adjacent = 0, two = +1, four = +1, and Replaceable Parts adding +1 per neighbour on top |
| Disband unit | Civ VI lets a player delete any of their own units on their turn — a legal action under FIDELITY clause 1 — but CIVVIS only ever disbanded units automatically under bankruptcy. An agent could never voluntarily disband | new `Action::DeleteUnit`, offered for every own unit in `legal_actions` and handled by `do_delete_unit`, which removes the unit with no refund and counts the loss for elimination and domination exactly as a battlefield death does. Registered in the action-space encoding (KINDS 76 → 77) so the observation tensor exposes it |
| Bibliophile's agenda | Sweden's Bibliophile agenda named the `great_works` measure, but no branch of the agenda scorer read it, so it fell through to the 0.0 default and the agenda never fired. It had shipped dead in pass 68 | the `great_works` measure now counts a civilization's housed Great Works, and `tools/civvis_dangling.py` grew a fourth report — every agenda's `measure` must appear as a literal the scorer reads, so a dead agenda cannot ship again |
| World Congress | Two of the eighteen Gathering Storm resolutions never reached the floor. Sovereignty was missing outright, and Arms Control was fully implemented in `resolve_congress` but absent from `regular_congress_candidates`, so no session could ever propose it — live code that could not be reached | Sovereignty joins the slate (outcome A doubles the yield a city-state type is known for on Trade Routes into its cities, outcome B silences that whole type's Suzerain bonus), and Arms Control is added to the roster, which activates the handler that was already there |
| Natural disasters | Recorded, not fixed: `resolve_flood`, `resolve_drought` and `resolve_coastal_flooding` all exist and are correct, but nothing random ever calls them. The only live trigger is a Spy breaching a Dam, and sea level rise through the climate phases. Volcanoes are placed on the map by `mapgen` and never erupt; `burning_forest` and `burnt_forest` ship as features with no fire mechanic | not attempted: Gathering Storm ties disaster frequency to the map's Disaster Intensity setting and those per-turn probabilities are not published anywhere fetchable. Implementing the roll would mean inventing the rate, so the gap is documented instead |
| Happiness yields | The yield column of the Happinesses table was a copy of the growth column. CIVVIS paid Ecstatic +20% and Happy +10% on yields, and taxed Displeased -10% and Unhappy -20%. The Civilopedia's Happiness page states both columns separately: "a Happy city will have a 10% growth increase and a 5% yield increase... an Ecstatic city will have a 20% growth increase and 10% yield increase", and the penalty bands cost growth alone until a city reaches Unrest | `amenity_yield_mult_for` now pays +10%/+5% and leaves Displeased and Unhappy yields untouched, dropping to 70% only at Unrest. The growth column was already right and is unchanged. Twenty-six assertions across five modules had the old multiplier baked into their expected values and were rebased |
| Elite Forces | An eleventh Dark Age card, missed in the sweep that added the other ten | +100% combat experience for every unit, and a Gold on the upkeep of each combat and Spy unit, Classical through Renaissance |
| Dark Age policy cards | The whole card family was absent. Rise & Fall gives a civilization in a Dark Age access to ten Wildcard-slot cards that trade a real drawback for a strong bonus — the mechanism that makes a Dark Age playable rather than merely survivable | `PolicySpec` gains `dark_age`, `min_era` and `max_era`; a Dark Age card is offered, slotted and paid out only while the civilization's age is `dark` and the world era is inside the card's window, enforced in `available_policies`, `has_policy` and `policy_effect` together. All ten shipped with their exact text: Isolationism, Twilight Valor, Monasticism, Inquisition, Letters of Marque, Robber Barons, Collectivism, Rogue State, Flower Power and Automated Workforce |
| Dedications | Two of the twelve were missing outright (Bodyguard of Lies, Wish You Were Here), and — larger — every Dedication's **Dark Age half** was absent. Civ VI gives each Dedication two effects: an Era Score trigger paid in a Normal or Dark Age, and a bonus paid only in a Golden or Heroic one. CIVVIS implemented only the bonus half, so the Era Score ladder a civilization climbs out of a Dark Age on did not exist. The era gates were also loose: Heartbeat of Steam was offered from the Renaissance and Monumentality, Pen Brush and Voice, Free Inquiry and Exodus of the Evangelists vanished from the Modern era onward | the roster is now all twelve, gated only where the Civilopedia gates them (Heartbeat of Steam Industrial onward, Sky and Stars Atomic onward, Automaton Warfare Information onward); `dark_dedication`/`score_dedication` pay the Era Score half in Normal and Dark Ages, hooked at every trigger the Civilopedia names — specialty districts, Eurekas, Inspirations, Great Work and Science and Industrial and Aerodrome buildings, Great People, completed Trade Routes, first conversions, Corps and Army kills, naval kills, natural wonders, Giant Death Robot kills, successful offensive Spy operations and extracted Artifacts |
| Cultural dominance | The Civilopedia's Cultural Dominance page defines it exactly — attracting more visiting tourists out of a civilization than it has domestic tourists of its own — and hangs three effects on it. CIVVIS computed both tourist counts already and used neither for this: none of the three effects existed | `culturally_dominant_over(source, target)`, consumed in three places: Citizens exert 25% more Loyalty pressure on the dominated civilization's cities, a Trade Route into one of its cities pays +4 Gold, and a Spy mission there finishes in half the time |
| Free Cities, continued | A revolted city raised two defenders and then stood still forever, while Civ VI has a Free City "continue training them non-stop" | a Free City with an empty queue gives itself the best land defender it can raise, so its own Production sets the cadence and no interval is invented. Liberating a captured Free City back to its founder already worked through the existing conquest path and is now pinned by a test |
| Loyalty per turn | The Happiness band paid no Loyalty at all; the shipped Happinesses Loyalty column pays Ecstatic +6, Happy +3, Displeased −3, Unhappy and below −6 | `amenity_loyalty_for` joins `process_loyalty` |
| Healing | Any city or district tile healed at the city rate, including a hostile one — a unit standing on an enemy Campus healed 20 instead of 5 | the city rate needs the district to belong to the healing civilization (or a city-state it is Suzerain of) |
| Governors | Five of the seven carried a first-tier promotion as a free effect of the appointment itself — Magnus' Groundbreaker, Liang's Guildmaster, Moksha's Bishop, Victor's Redoubt and Amani's Messenger all arrived with the Governor rather than with a Title. Pingala and Reyna were already right, which is what made it visible | each is now a tier-1 promotion in `data/governors.json`; establishing a Governor grants the +8 Loyalty and nothing else, as the shipped appointment does |
| Policy cards | The three Wonder-Production cards ignored their era windows, exactly as the unit cards once did: Corvee boosted Big Ben as readily as the Pyramids, and Gothic Architecture reached past the Renaissance | `wonder_eras` gates them the way `unit_eras` gates Agoge — Corvee to Ancient and Classical, Gothic Architecture to everything before Industrial, Skyscrapers ungated |
| Beliefs | Sixteen shipped pantheons were missing entirely, the three most-played Holy Site adjacency beliefs among them | Stone Circles, Goddess of the Hunt, Initiation Rites (Faith *and* the clearing unit healed whole), Earth Goddess, Lady of the Reeds and Marshes, Monument to the Gods, Desert Folklore, Dance of the Aurora, Sacred Path, River Goddess, Religious Idols, God of Craftsmen, God of Healing, God of War, Goddess of Festivals and City Patron Goddess now execute — twenty-two of the shipped roster against the six CIVVIS started with. The terrain beliefs land before the adjacency cards, so Scripture doubles them as it should |
| Beliefs | Two shipped founder beliefs were missing | Cross-Cultural Dialogue (+1 Science per 4 followers, beside World Church's Culture) and Papal Primacy (city-state type bonuses 50% stronger where the city-state follows the religion) execute; the founder class is now seven of the shipped eight |
| City-states | Six of the eighteen city-states carried a type — so they paid Envoy bonuses — but no Suzerain ability at all: Jerusalem, Brussels, Preslav, Antananarivo, Seoul and Amsterdam | Five now execute: Brussels (+15% wonder Production), Antananarivo (+2% Culture per Great Person earned, capped at 30%), Jerusalem (every Holy Site city presses like a Holy City), Seoul (a Eureka on entering each era) and Amsterdam (+1 Gold per Luxury at an international destination). Preslav (+5 Combat Strength for Light and Heavy Cavalry fighting on Hills) closed the set: every city-state in the roster now has its Suzerain ability |
| Beliefs | Warrior Monks could not be trained at all: the unit exists but is not buildable, and the follower belief that unlocks its Faith purchase was missing, so the only Warrior Monk in the game came from a wonder grant | the belief ships, and `do_buy` admits the unit on Faith in a city with a Holy Site whose majority religion holds it |
| Beliefs | Two shipped follower beliefs were missing | Divine Inspiration (every World Wonder pays +4 Faith) and Reliquaries (Relics pay triple Faith and Tourism) execute; the follower class is now eight of the shipped ten |
| Trade | Trade Route range was a flat 15 tiles with no way to extend it; in Civ 6 every city where a civilization holds a Trading Post carries its routes 15 tiles further, which is how a mature trade network keeps widening | `trade_route_in_range` chains through the owner's own posts; a rival's post carries nothing |
| Civilization abilities | Rome's All Roads Lead to Rome was the last unmodelled second ability, and it could not be written before Trading Posts existed | every Roman city, founded or conquered, arrives with a Trading Post, and a founded one is roaded to the Capital when the Capital is in Trade Route range. Its third clause — Gold for routes *passing through* posts — waits on a route path model |
| Trade | Trading Posts did not exist. In Civ 6 a Trade Route that runs its course leaves one in both City Centers, and every Trading Post foreign to a city pays +1 Gold to each route reaching it | `City.trading_posts` records the holders, completion (not cancellation) creates them, and the Gold is paid. The other half of the mechanic — a Trading Post extending Trade Route range by 15 tiles — waits on CIVVIS modelling a route range at all, which it does not |
| Civilizations | CIVVIS modelled eight, and the ruleset already carried unique districts and buildings for twenty-eight more — the Hansa, the Seowon and the Lavra among them, with nobody to build them | Germany, Korea, Russia, the Zulu, the Maya, Gaul, Mali, Phoenicia, England, Byzantium, Vietnam, the Kongo, Brazil, Norway, Macedon, Poland, Japan, America, Arabia, Mongolia, Portugal, the Ottomans, Hungary, Sweden, Babylon, the Khmer, Persia and France join the roster — with Brazil every unique district and every unique unit in the ruleset finally has a civilization that can build it. England: British Museum (Archaeological Museums hold six Artifacts, which is also what lets a second Archaeologist work, since CIVVIS gates them on free Artifact capacity), Victoria's Pax Britannica (the first city on each new continent brings a free melee unit and a Trade Route, and every Royal Navy Dockyard launches the best warship England can build) and both of her unique units, the Sea Dog and the Redcoat. Byzantium: Taxis (+3 Combat and Religious Strength for every Holy City in the world that follows Byzantium's religion, 250 pressure of that religion into every city within ten tiles whenever a civilization's or city-state's unit is defeated, and a Great Prophet point from every city with a Holy Site, which doubles the district's own), Basil II's Porphyrogénnētos (heavy and light cavalry ignore the 85% wall-damage penalty against cities that already follow Byzantium's religion) and both his unique units, the Tagma, which arms every land unit beside it by 4, and the Dromon, a Quadrireme that shoots two tiles and 10 harder against units. Vietnam: Nine Dragon River Delta (land specialty districts only on Woods, Rainforest or Marsh — the Harbor and the Water Park are not land districts and stay on the water — the feature survives the district, every building standing on one pays a Culture in Woods, a Science in Rainforest or a Production in Marsh, and Woods may be planted from Medieval Faires rather than Conservation), Ba Trieu's Drive Out The Aggressors (+5 Combat Strength fighting on those features and +1 Movement starting on them, both doubled inside Vietnam) and the Voi Chien, a 200-Production Crossbowman with 35 Strength, 3 Movement, 3 sight, that moves after attacking. Her agenda Defender of the Homeland needed a measure that never decays: whether a civilization has ever declared war on her, concluded wars included. Kongo: Nkisi (every Relic and Artifact pays +2 Food, +2 Production, +1 Faith and +4 Gold on top of its Culture or Faith, Great Artist, Musician and Merchant points arrive half again as fast, and the Palace holds five Great Works rather than one), Mvemba a Nzinga's Religious Convert (he may build no Holy Site, gains no Great Prophet and founds no religion, takes every Belief of whichever faith holds a majority of his cities, and an Apostle of that city's faith walks out of each finished Mbanza or Theater Square) and the Ngao Mbeba, a 110-Production Swordsman of 38 Strength needing only 5 Iron. His agenda Enthusiastic Disciple judges a civilization solely on whether the religion it founded has reached a Kongolese city. Brazil: Amazon (each adjacent Rainforest is a full adjacency to a Campus, Commercial Hub, Holy Site or Theater Square, and Rainforest raises the Appeal of neighbouring tiles by 1 rather than lowering it by 1), Pedro II's Magnanimous (a fifth of a Great Person's point cost is returned the moment they are recruited or patronized) and the Minas Geraes, a Battleship unlocked by the Nationalism civic with 70 Strength, 80 Ranged Strength and 95 anti-air. His agenda Patron of the Arts scores rivals on how many Great People they have taken. Norway: Knarr (Ocean opens with Shipbuilding rather than Cartography, naval melee units repair themselves in neutral water, and embarking or disembarking costs nothing extra), Harald Hardrada's Thunderbolt of the North (every naval melee unit can raid a coast and is built half again as fast, a raided Mine pays Science beside its Gold and a raided Quarry, Pasture, Plantation or Camp pays Culture beside its Faith) and both his unique units, the Viking Longship and the Berserker. His agenda Last Viking King counts hulls. Adding Norway also retired a dead engine clause: `unit_ignores_zoc` had tested for a `viking_longship` that did not exist in the ruleset. Macedon: Hellenistic Fusion (taking a city pays a Eureka for each Encampment or Campus in it and an Inspiration for each Holy Site or Theater Square — counted from the city **as it fell**, since districts the captor cannot yet operate are dropped moments later and Civ VI still counts them), Alexander's To World's End (Macedonian cities never suffer war weariness, every military unit is made whole when a city holding a wonder falls, and grievances against him decay twice as fast) and both his unique units, the Hetairoi and the Hypaspist, whose +5 against districts sits on the city-attack path rather than in `matchup_bonus`, which never sees a district. Poland: Golden Liberty (an Encampment finished at home Culture Bombs the ground around it, and one Military policy slot runs as a Wildcard), Jadwiga's Lithuanian Union (a city that loses a tile to that bomb takes Poland's religion, a Holy Site takes a whole point of Faith from each adjacent district rather than the usual half, and every Relic pays +2 Faith, +2 Culture and +4 Gold) and the Winged Hussar. Japan: Meiji Restoration (every district takes a whole standard adjacency from each district beside it, on top of the half everyone gets), Hojo Tokimune's Divine Wind (land units fight 5 harder on land within sight of Coast and ships 5 harder in shallow water, and Encampments, Holy Sites and Theater Squares are raised in half the time) and the Samurai, which needed a unit-aware `unit_effective_strength` because the wounded penalty was a free function that only ever saw a number and a hit-point count. America: Founding Fathers (every Diplomatic policy slot runs as a Wildcard, and each Wildcard slot pays a Diplomatic Favor a turn), Teddy Roosevelt's Roosevelt Corollary (+5 Combat Strength on America's own continent, and a city holding a National Park lifts the Appeal of every tile it owns) and the P-51 Mustang. His agenda Big Stick Policy reads the observer's own continent: a civilization with a city there is liked while it keeps the peace and disliked once it starts a war on it. Arabia: The Last Prophet (the final Great Prophet comes unasked once the next-to-last religion is founded, and every foreign city following Arabia's religion pays a Science), Saladin's Righteousness of the Faith (Arabia's Worship building costs a tenth of the usual Faith **for every civilization, not only Arabia**, and lends Arabian cities a tenth more Science, Faith and Culture) and the Mamluk, which mends itself at the end of every turn whatever it spent the turn doing. Mongolia: Örtöö (a Trade Route lays its Trading Post the moment it starts rather than when it expires, a Trading Post in any of a civilization's cities is worth a level of Diplomatic Visibility over it, and Mongolian units take 6 Combat Strength per level of advantage rather than 3), Genghis Khan's Mongol Horde (+3 Combat Strength on every cavalry unit) and the Keshig, which carries civilians at its own pace. Portugal: Casa da India (an international Trade Route must leave a coastal city and reach a coastal one or one with a Harbour, and pays half again as much of everything), Joao III’s Porta do Cerco (every unit sees a tile further, and every living major civilization widens Portugal’s trade capacity — CIVVIS has no first-contact state, so "when a civilization is met" counts the living majors) and the Nau, with the Feitoria improvement it builds. Ottomans: Great Turkish Bombard (siege units are built half again as fast and take 5 more Combat Strength against district defences, a conquered city loses no Population at all, and a city the Ottomans did not found gains +1 Amenity and +4 Loyalty a turn), Suleiman's Grand Vizier (Gunpowder brings a Governor Title along with the Janissary) and the Janissary itself, at 60 Strength for half the Musketman's Production. Hungary: Pearl of the Danube (a district or building raised across a river from its City Center is built half again as fast), Matthias Corvinus' Raven King (a levied unit moves 2 further, fights 5 harder and costs three quarters less to upgrade, and levying from a city-state sends two Envoys with the Gold) and the Black Army, which fights 3 harder for every levied unit beside it. Sweden: Nobel Prize (a Great Person earns 50 Diplomatic Favor, a Factory pays a Great Engineer point and a University a Great Scientist point), Kristina's Minerva of the North (a building with three or more Great Work slots, or a wonder with two or more, themes itself the moment its slots are full, with no matching-set requirement) and the Carolean, which fights 3 harder for every Movement point it has not spent. Babylon: Enuma Anu Enlil (a Eureka finishes its technology outright rather than granting a fraction of it, but Babylon makes half the Science of anyone else), Hammurabi's Ninu Ilu Sirum (the first specialty district of each family but the Government Plaza brings the cheapest building it can hold, and the first of any other district family brings an Envoy) and the Sabum Kibittum, a starting melee unit with +17 against cavalry. Khmer: Grand Barays (a city with an Aqueduct gains an Amenity and a Faith per Citizen, and a Farm gains +2 Food beside an Aqueduct or +1 Faith beside a Holy Site), Jayavarman VII's Monasteries of the King (a Holy Site takes a major river adjacency, culture-bombs the ground around it, pays Food equal to its adjacency, and houses two more on a River) and the Domrey, a Trebuchet that also holds a zone of control. Persia: Satrapies (Political Philosophy opens a Trade Route, a domestic route pays +2 Gold and +1 Culture, and Persian roads are laid a grade above everyone else's), Cyrus' Fall of Babylon (a Surprise War is judged as a Formal one for Grievances, the army moves 2 further for ten turns after declaring one, and a garrison holds an occupied city 5 Loyalty steadier), the Immortal — a melee unit that also shoots — and the Pairidaeza, which draws Culture from adjacent Holy Sites and Theater Squares and Gold from Commercial Hubs and City Centers. France: Grand Tour (+20% Production on Medieval, Renaissance and Industrial wonders, and double Tourism from a wonder of any era), Catherine de Medici's Flying Squadron (a level of Diplomatic Visibility over every civilization she has met, and a free Spy with the room to keep it at Castles), the Garde Impériale (+10 Combat Strength on its own capital's continent) and the Château, which takes Culture from every adjacent wonder — twice as much after Flight — and Gold from the river it stands on. The Maori: Mana (the ocean is open from turn one and embarked units move 2 further, unimproved Woods and Rainforest pay Production — more with Mercantilism and Conservation, Fishing Boats give +1 Food, resources cannot be harvested and no Great Writer is ever earned), Kupe's Voyage (the Palace houses three more and calms one) and the Toa, which takes 5 Combat Strength off any adjacent enemy. Georgia: Strength in Unity (a Dedication scores its Era Score even inside a Golden Age, and defensive buildings are built half again as fast), Tamar's Glory of the World (a combat victory pays Faith worth half the fallen unit's strength, and an Envoy to a city-state of Georgia's religion counts double) and the Khevsur. **With Georgia every shipped unique unit, district and building in the ruleset has an owner** — 36 civilizations, and the inert-content audit that drove this run for a dozen passes is closed. Phoenicia: Mediterranean Colonies (a coastal city on the Capital's own continent never loses Loyalty, and Settlers move 4), Dido's Founder of Carthage (a Trade Route from every Government Plaza building and +50% district Production where the Plaza stands) and the Bireme. Mali: Songs of the Jeli (Desert beside the City Center pays Food and Faith, Mines trade a Production for four Gold, and everything built or trained costs 30% more), Mansa Musa's Sahel Merchants (a Gold on international routes for every flat Desert tile at home) and the Mandekalu Cavalry. Gaul: Hallstatt Culture (every Mine pays a Culture, and the district rules CIVVIS already carried for Gaul finally have a Gaul to apply to), Ambiorix's King of the Eburones (a fifth of every unit's cost in Culture, and +2 Combat Strength per adjacent friendly unit for melee, anti-cavalry and ranged) and the Gaesatae, which fights 10 stronger against anything with a higher base Strength. Maya: Mayab (no Housing from fresh water or coast — Farms carry it, pay a Gold, and take a Production from an adjacent Observatory, while a Luxury beside the City Center is an Amenity), Lady Six Sky's Ix Mutal Ajaw (a tenth more yield inside six tiles of the Capital, 15% less outside, and +5 Combat Strength in that ring) and the Hul'che. Zulu: Isibongo (a garrison adds 3 Loyalty a turn, a Corps or Army 5, and whoever takes a city is promoted into one), Shaka's Amabutho (+5 Combat Strength on every Corps and Army, and both formations arrive a civic early — Corps at Mercenaries, Armies at Nationalism) and the Impi, a 125-Production Pikeman that flanks harder and learns faster. Russia: Mother Russia (five extra tiles on founding, +1 Faith and +1 Production from Tundra), Peter's Grand Embassy (a Science or Culture per three nodes a trade partner is ahead) and the Cossack (67 Strength, +5 on and beside home soil, and it may move after attacking). Korea: Three Kingdoms (Farms +1 Food and Mines +1 Science per adjacent Seowon), Seondeok's Hwarang (+3% Culture and Science per promotion an established Governor holds, the title included) and the Hwacha, a 250-Production Field Cannon that cannot move and attack in the same turn. Germany: Free Imperial Cities (one specialty district past the population limit), Frederick Barbarossa's Holy Roman Emperor (an extra Military policy slot and +7 Combat Strength attacking city-states), the U-Boat (a Submarine at 430 Production needing no Oil, +10 Combat Strength in Ocean) and the Iron Crown agenda, which needed a new comparative measure — how many city-states a civilization has made itself Suzerain of |
| Tourism | The Tourism a government turns away from civilizations running a different one was hard-coded to 20 for the six late-game forms and zero for everything else. Communism against Democracy lands on the documented −40%, so the model and those values are right; the earlier tiers are the open question | the value moved into `data/governments.json` as `tourism_intolerance`, so the tiers CIVVIS has not measured can be filled in without touching the engine |
| Trade | The Gold a Trade Route takes from its destination's districts was multiplied only by two wonders — the Golden Gate Bridge and the Panama Canal — so an ordinary Canal, a Railroad and a Mountain Tunnel all counted for nothing. Gathering Storm instead scores the path the Trader walks | water, Canal and Railroad tiles are worth 2 efficiency apiece and a Mountain Tunnel 15, and the district Gold doubles as the score reaches about 1.6x the route's length. The per-tile values are the shipped ones; the ratio is the community-measured anchor, in the same spirit as the unit-upgrade Gold formula. The two wonders keep their flat multiplier as a floor |
| Game modes | Secret Societies ran unconditionally too: every civilization could join the Hermetic Order, the Owls of Minerva or the Voidsingers at Code of Laws, and their replacement buildings existed in a default game | `Game.secret_societies` gates the choice and the action, defaulting to off; the two tests that exercise the mode turn it on |
| Game modes | Monopolies and Corporations ran unconditionally, but it is an optional New Frontier game mode that a standard game leaves **off** — the baseline this project declares. Industries, Corporations, Products and Monopoly Gold/Tourism were all live in a default game, which is a legal-action-set divergence: clause 1 of the exactness contract | `Game.monopolies` gates the improvements, `can_found_corporation` and `monopoly_bonuses`, defaulting to off; the three tests that exercise the mode turn it on explicitly |
| Natural wonders | Eight of roughly thirty were modelled | Twenty now are, all placed: Kilimanjaro, the Galapagos Islands, Cliffs of Dover, Torres del Paine (doubles the terrain yields of every adjacent tile), the Giant's Causeway and the Matterhorn (both mark passing land units with a permanent promotion), the Eye of the Sahara (deepens in the Atomic era), Ubsunur Hollow (−2 defence, +1 Movement cost), Piopiotahi, Zhangye Danxia (pays Great General and Great Merchant points to whoever holds a tile), Tsingy de Bemaraha and the Delicate Arch. Placement predicates are CIVVIS' own reading; the shipped `Feature_ValidTerrains` rows are only checkable against an installation |
| Appeal | Every natural wonder lifted its neighbours by a hard-coded +2; Uluru ships +4 | the bonus reads `adjacent_appeal` from the feature, defaulting to 2 |
| Unique improvements | Sumeria's Ziggurat did not exist (+2 Science, +1 Culture beside a River, another with Natural History, never on Hills), and the Nubian Pyramid paid only its base yields — none of the adjacency that is the point of it, and it refused Desert Hills and Floodplains, two of its three legal sites | the Ziggurat ships as an improvement with no technology gate; the Pyramid pays +1 Food beside the City Center and +1 of each adjacent district's own yield, and sites on Desert, Desert Hills or Floodplains. Rome's Bath was already exact |
| Unique units | Three defects in the unique roster: the Heavy Chariot never received its open-terrain Movement at all (the bonus was hard-coded to the two uniques that inherit it), the Crouching Tiger was marked as replacing the Crossbowman when it replaces nothing — locking China out of a unit it can build — and the Legion had neither its build charge nor the Roman Fort | the chariot bonus is +1 for the Heavy Chariot and War-Cart and +2 for the Maryannu, applied over each unit's own base; `crouching_tiger` no longer replaces; `roman_fort` is an improvement `unique_to` Rome with no technology gate, and the Legion carries the one charge that builds it |
| Civilization abilities | Gilgamesh's Adventures with Enkidu charged him full Grievances for joining a war against someone already fighting his ally | the Grievance is waived; the 5-tile shared experience and pillage clause is still open |
| Civilization abilities | Three more second abilities existed nowhere: the Aztec Legend of the Five Suns (a Builder charge completes 20% of a district), Qin Shi Huang's First Emperor wonder clause (a charge pays 15% of an Ancient or Classical wonder — only its +1 Builder charge was modelled) and Cleopatra's Mediterranean's Bride (+4 Gold on Egypt's international routes; a foreign route into Egypt pays its owner +2 Food and Egypt +2 Gold) | the two Builder clauses are `hurry_district` / `hurry_wonder` operations charged against the city's current item; the route clauses pay at both ends |
| Civilization abilities | Every civilization in Civ 6 carries two — its own and its leader's — where `data/civs.json` held one. Egypt's Iteru paid its 15% to wonders beside a River but not to districts, and neither its flood immunity nor Amanitore's Kandake of Meroë (+20% district Production, +40% beside a Nubian Pyramid) or Scythia's People of the Steppe (Light Cavalry and the Saka Horse Archer train in pairs) existed at all | `CivSpec` takes an `abilities` list beside the headline one, and the four clauses execute. Tomyris' +5 against wounded units was already there and now has a test |
| Governors | Three promotions were missing the second clause Gathering Storm gives them: Pingala's Researcher pays +20% Production toward Campus buildings as well as its Science per Citizen, Magnus' Industrialist +1 Production per worked Strategic resource as well as its power-plant bonus, and Black Marketeer waives the resource *requirement* — a city with an empty stockpile still trains the unit — not only its 80% discount | all three clauses execute, the last two through new engine arms (`strategic_resource_production`, `strategic_resource_optional`) |
| Governors | Victor carried seven promotions where every Governor ships six: Security Expert is a Rise and Fall row that Gathering Storm replaced with Arms Race Proponent, moving its Spy defence onto Amani's Local Informants — which CIVVIS already had | Security Expert removed; the count is six everywhere |
| War Weariness | Not modelled at all. `war_weariness_multiplier` computed the policy and government reduction and nothing consumed it, so no battle ever cost an Amenity and a permanent war was free | War Weariness Points accrue per battle at the shipped Era Base — 16 rising 6 an era to 40, plus 3 an era to 12 for a Surprise War aggressor — doubled away from home and doubled again for a city battle, with three more Era Bases when the unit dies. They decay 50 a turn at war and 200 at peace, 2,000 on signing peace, and every complete 400 costs one Amenity in every city |

Both mode flags are exposed in the observation, because which modes are on changes the legal action set an agent must reason over.

Verified exact in the same pass, and left alone: the pressure-to-Loyalty
curve `10 × (domestic − foreign) / (min + 0.5)` clamped to ±20, the Age
factors, the 9-tile range and 10%-per-tile falloff; the four healing rates
(20/15/10/5) and the naval restriction to friendly territory; unit combat
experience (relative Combat Strength, doubled on a kill, +2 melee / +1
ranged / +1 attacking, capped at 8) and city combat experience (3 attacking,
2 defending, 10 for the capture or the killing blow, uncapped); Science and
Culture per Citizen (0.5 / 0.3); the standard start of one Settler and one
Warrior; the Diplomatic Victory threshold of 20 points with the Statue of
Liberty at 4, Mahabodhi Temple 2 and Potala Palace 1; the Exoplanet
Expedition's 50 light years; the World Congress vote refunds (full Favor when
your outcome loses, half when the outcome wins but your target does not) and
the +1 Diplomatic Victory point for backing the winning pair; and the
district cost progression (1x to 10x on the tree ratio, truncated to whole
percents, with the underbuilt discount applied after).

## Phase 2 (measured): the modifier engine

The size of this phase is no longer a guess. `tools/civ6_modifiers.py`
censuses the shipped `Modifiers` tables and reports 3,405 rows across 698
distinct effects, of which CIVVIS covers 825 rows. Crucially the tail is long:
32 effects reach half the rows, and the other half needs 666 more. See
[MODIFIERS.md](MODIFIERS.md) for the ranked backlog and the order of work.

### Why an interpreter

Nearly all Civ 6 *content* — leader and civilization abilities, wonders,
beliefs, policies, governors — is not code. It is rows in the `Modifiers`
table: an `EffectType`, arguments, and a `RequirementSet`, attached to a
collection of game objects. CIVVIS hardcodes these effects one at a time in
Rust, which is why every new civilization is an engineering task.

Implementing the interpreter instead of the content inverts that:

- Content correctness collapses into engine correctness. A few hundred effect
  and requirement types cover thousands of rows.
- Remaining scope becomes measurable. Log every unimplemented `EffectType` with
  the number of rows that reference it, and implement in frequency × impact
  order instead of guessing.
- Balance mods become a database swap. The competitive-multiplayer ruleset
  (BBG) is mostly SQL edits, so a modifier-driven CIVVIS gets it nearly free.

## Phase 3: the ground-truth bridge

The real game cannot run headless or fast, so it can never be a training
environment — but it is an excellent *oracle*, and the same bridge doubles as
the evaluation and exhibition path that makes a CIVVIS-trained policy a Civ 6
policy.

Three components, all reusable:

1. **Logger mod** (Lua, gameplay context): serialize full state each turn plus
   the ordered `GameEvents` stream into the canonical schema. Two modes —
   omniscient for golden tests, `PlayerVisibility`-filtered for fair play.
2. **Action injector**: FireTuner speaks a local TCP console protocol, so Lua
   can be driven remotely and unit/city/player operations issued through the
   same request path a human client uses.
3. **Turn-0 import**: fix the map and game seeds, export via WorldBuilder
   (`.Civ6Map` is SQLite) and load the same start directly in CIVVIS.

The property that makes this work: with fixed seeds and an identical action
stream, a real game is reproducible. Every combat roll and goody hut replays,
so real games become perfect oracles without touching the DLL. The event stream
is also the empirical specification of the between-turns phase machine —
healing versus growth versus border expansion, barbarian and city-state
ordering — and ordering bugs are where near-clones die.

## Phase 4: the differential test stack

Escalating, each stage cheaper per bug than the next:

1. **Derived-value differ (static).** From any dumped real state, recompute
   every derived quantity — yields, adjacencies, combat previews, movement
   costs, housing, amenities, loyalty — and diff. Requires no dynamics.
2. **Action-replay differ (dynamic).** Replay logged action streams from turn
   0 with stochastic outcomes forced from the log; require per-phase state-hash
   equality; report first divergence by subsystem.
3. **Distribution tests.** KS-test each stochastic node — combat rolls,
   barbarian spawns, goody huts, climate events — against thousands of real
   samples.
4. **Fuzzing.** Random-but-legal action sequences injected into both engines,
   with delta-debugging minimization down to a small repro.
5. **CI freeze.** The golden corpus reruns on every rules change, so fidelity
   only ratchets up.

Oracle throughput is the bottleneck: real games run near real time, roughly one
game per hour per instance. Plan for a small fleet or weeks of soak, and lean
on distribution tests and fuzzing where oracle samples are scarce.

## Determinism rules for engine code

These already hold in CIVVIS and must keep holding, because clause 2 depends on
them: integer/fixed-point arithmetic in rules code (the game's own values are
integers, often ×100); no unordered iteration in game logic; per-subsystem RNG
streams with seeded, forced and recording modes; serialization to the same
canonical schema the logger emits, so diffing is trivial; a state hash per
phase; cheap snapshots so search stays affordable.

## Baseline configuration

Gathering Storm ruleset, standard DLC civilizations, NFP game modes off,
sequential turns. Free-for-all and official pre-game team relations and victory
rules are both supported. Simultaneous-turn multiplayer changes action
interleaving rather than transition rules, so the rest of the competitive
ruleset layers on after the sequential clone is exact. See
[COMPETITIVE.md](COMPETITIVE.md) for the tournament-specific boundary.
