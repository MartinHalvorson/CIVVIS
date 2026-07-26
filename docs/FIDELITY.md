# Fidelity: making CIVVIS an exact match for Civilization VI

CIVVIS started as a Civ-6-*like* engine: rules transcribed by hand from the
Civilopedia and the wiki, tuned until games felt right. That is enough to train
an agent that is superhuman *at CIVVIS*. It is not enough for the goal this
project is actually aimed at — an agent whose policy transfers back to the real
game. A policy trained against wrong unit costs learns wrong build orders.

This document defines what "exact" means here, how it is measured, and what is
built next.

## The prose is not the ruleset

`tools/civ6_fidelity.py` needs the game's *install* directory to replay the XML
load order, so it cannot run without Civilization VI installed. It is not the
only route to the shipped numbers: a machine that has ever run the game leaves
the compiled gameplay database behind at

    ~/Library/Application Support/Sid Meier's Civilization VI/Cache/DebugGameplay.sqlite

(`%LOCALAPPDATA%\Firaxis Games\...\Cache\` on Windows), and that file is a
plain read-only SQLite database with the whole ruleset in it — `LoyaltyLevels`,
`Happinesses`, `GlobalParameters`, `Units`, and 400-odd more tables. Query it
directly before changing any number.

## Running the audit without an install

`tools/civ6_fidelity.py --cache` reads the compiled gameplay database directly
instead of replaying the install's XML load order, so the ratchet runs on a
machine where Civilization VI is no longer installed. It finds the file at the
usual Cache path on macOS and Windows, or takes one: `--cache <path>`.

The two routes are not guaranteed identical. The XML route reconstructs a
specific content set in a specific order; the cache is whatever the game last
compiled for itself. **Where they disagree, that disagreement is itself a
finding** — see the Cartography note below.

### Aliases decide what is audited at all

`ALIASES` maps the game's spelling onto CIVVIS'. An entry the map does not cover
is not reported as wrong — it is reported as *absent from the other side*, and
compared against nothing. The nine unique units the game prefixes with their
civilization (`UNIT_ROMAN_LEGION`, `UNIT_GREEK_HOPLITE`,
`UNIT_EGYPTIAN_CHARIOT_ARCHER`, …) sat in that blind spot, so the `Units` table
audited 73 of 82 rows and looked clean. Aliasing them surfaced four real
divergences immediately, all now fixed:

| Unit | Field | was | shipped |
|---|---|---|---|
| Maryannu Chariot Archer | cost | 120 | **90** |
| Maryannu Chariot Archer | maintenance | 2 | **1** |
| Roman Legion | build charges | 0 | **1** (its Roman Fort) |
| Anti-Air Gun | range | 0 | **1** |

Egypt's unique was a third overpriced and cost twice the upkeep, which is a
direct distortion of that civilization's strength. **When a table reports
"Only in CIVVIS", check the alias map before concluding anything** — a nonzero
count there means rows are going unaudited.

### First cache run: 15 divergences, 5 fixed

The install-based audit last reported zero unwaived divergences. The cache run
reported fifteen. Five are fixed here; the rest are triaged below.

- **`Adjacency` / `industrial_zone` mine — FIXED here.** CIVVIS paid 1.5
  Production per adjacent Mine. The shipped `Minel_HalfProduction` row is
  `YieldChange` 1 with `TilesRequired` 2, i.e. **0.5 per Mine** — three times
  too generous on a core production adjacency, in a district built beside hills
  by every civilization that industrializes. Every other Industrial Zone source
  matched exactly (quarry 1, lumber mill 0.5, district 0.5, aqueduct/canal/dam
  2, government plaza 1, strategic 1), which is what makes the one outlier
  convincing rather than a projection artifact.

- **`Buildings` — three uniques carrying their base building's yield.** The
  Prasat replaces the Temple (Faith 4, one Relic slot) and ships Faith **6** with
  the same single slot; CIVVIS had the Temple's 4 and an invented second slot.
  The Sukiennice replaces the Market (Gold 2) and does not raise it; CIVVIS had
  3. The Tlachtli replaces the Arena (Culture 1) and doubles it to **2**; CIVVIS
  had the Arena's 1. One error repeated three times, which is what identified it.
- **`Improvements` / `sphinx` terrain.** CIVVIS allowed Snow;
  `Improvement_ValidTerrains` lists Desert, Grassland, Plains and Tundra with
  their Hills variants, and no Snow.

- **`Improvements` placement rows.** The Nubian Pyramid ships `TERRAIN_DESERT`
  *and* `TERRAIN_DESERT_HILLS`, so it is not flat-only, and `FEATURE_FLOODPLAINS`
  is a valid host — CIVVIS had neither. Gathering Storm's Volcanic Soil is a
  valid Beach Resort host; CIVVIS had no feature list at all.
- **`Resources` / `niter`.** CIVVIS listed the generic `floodplains` alongside
  the two typed ones. `Feature_ValidTerrains` settles what that generic feature
  is: `FEATURE_FLOODPLAINS` is Desert-only, and `Resource_ValidFeatures` for
  Niter names only the Grassland and Plains variants. Removed, with
  `every_strategic_resource_reaches_a_playable_supply` standing guard — plain
  Desert is still a valid Niter terrain, so only Desert *floodplains* are
  excluded and supply is unaffected.
- **`Boosts` / `near_future_governance` — waived, not a divergence.** The shipped
  trigger is `BOOST_TRIGGER_HAVE_GOVERNMENT_TIER` at `Tier4` with no `NumItems`,
  so the projection reads 0 against CIVVIS' ten Government slots. Those are
  exactly equivalent: `Government_SlotCounts` totals 2/4/6/8/10 for
  Chiefdom/Tier1/Tier2/Tier3/Tier4, so ten slots is reached by the Tier 4
  governments and by nothing else. The 90% boost matches.

Still to triage, listed so they are not lost:

| Table | Entry | CIVVIS | cache DB |
|---|---|---|---|
| Technologies | `cartography` requires | buttress, shipbuilding | buttress |
| Technologies | `mass_production` requires | …, shipbuilding | (no shipbuilding) |
| Boosts | `near_future_governance` count | 10 | 0 |
| Resources | `niter` feature | + generic floodplains | only the two typed floodplains |
| Resources | `pearls`/`turtles`/`whales` improvement | fishing_boats | industry |
| Improvements | `corporation`/`industry` resources | 3 luxuries | 28 luxuries |
| Wonders | `biosphere` yields | science 8 | none |

`niter` and `biosphere` are deliberately left alone. CIVVIS' generic
`floodplains` is the Desert variant the game calls `FEATURE_FLOODPLAINS`, so
dropping it from Niter is a map-placement change dressed as a data fix, and it
needs checking against where Niter actually spawns rather than against the
feature list alone. The Biosphere is settled, and my earlier reason for
leaving it was wrong: its shipped effect is not Culture but +1 Appeal to Marsh
and Rainforest, +100% renewable-power Tourism and +200 free Power — and CIVVIS
already models all three, as `rainforest_marsh_adjacent_appeal`,
`renewable_power_tourism` and `renewable_power_pct`, all live in the engine. The
`science: 8` was invention layered on top of a correct wonder, so removing it
costs the Biosphere nothing.

**Three of the fifteen were the tool's fault, not CIVVIS'.** The
resource-to-improvement projection prefers a land improvement over a sea one, so
that Oil keys on Oil Wells rather than Oil Rigs — and the Industry and
Corporation of Monopolies & Corporations are *land* improvements that sit on top
of an already-improved luxury. That preference reported Pearls, Turtles and
Whales as improved by `industry` instead of Fishing Boats. `Improvements_MODE`
names the game-mode improvements, so the projection skips them and the rule stays
data-driven; their own luxury lists are waived, because CPL's lobby disables all
game modes. **A divergence is a claim about two sides, and the projection is one
of them** — check what the tool did before changing what CIVVIS says.

**The Cartography pair contradicts this document's own history.** The first-wave
install audit lists "Cartography and Mass Production both require Shipbuilding"
as a *fix it applied*, and the cache says the opposite. One of the two reads is
wrong, and settling it needs an install — do not "correct" CIVVIS from the cache
alone. The `pearls`/`turtles`/`whales` rows look like a projection artifact
(harvest improvement versus Corporation improvement) rather than a CIVVIS defect,
and should be checked before being treated as one.

## Closed: civilization start bias

Civilization VI ships four `StartBias*` tables — 132 rows across
`StartBiasTerrains`, `StartBiasFeatures`, `StartBiasRivers` and
`StartBiasResources` — that decide *which* start a civilization is given. A
lower `Tier` is a stronger pull. **CIVVIS applies none of it**: seat `i` simply
takes `spawns[i]`, so which civilization lands on which start is an accident of
seat order.

Five of the eight shipped civilizations have a bias, and it is most of what
makes two of them what they are:

| Civilization | Shipped bias |
|---|---|
| Egypt | Floodplains (Tier 2, all three variants), River (Tier 5) |
| Greece | Hills — Grass, Plains, Desert, Tundra (Tier 3) |
| Sumeria | River (Tier 3) |
| Nubia | Desert and Desert Hills (Tier 2), plus ten strategic and luxury resources (Tier 5) |
| Scythia | Horses (Tier 2), Grassland and Plains (Tier 5) |

Rome, China and the Aztecs have no bias, which is correct — they have none in the
shipped tables either.

An Egypt that does not start on a river or floodplains is not the Egypt the
tournament drafts, and a Scythia away from Horses is a different civilization.
CPL allows duplicate civilizations, so drafting depends on each performing the
way its bias implies.

**Fixed.** The bias rows are carried in `data/civs.json`, mapped onto CIVVIS'
own spelling — Hills are a tile flag rather than a terrain here, so Greece's four
Hills rows become one `terrain_hills` requirement, and the floodplain variants
take the names `features.json` uses. `start_bias_score` weighs a site by the
biases it satisfies, each worth `6 - Tier`, across the tiles a city works rather
than the centre alone, and `assign_starts_by_bias` permutes the major sites
before any seat is handed one. Generation itself is untouched.

Measured over ten seeds and the five biased civilizations, asking whether each
civilization's own site beats the average of the other seven *for its bias*:

| | better | worse |
|---|---|---|
| before | 34 | 16 |
| after | **50** | **0** |

## Closed: major start spacing

`START_DISTANCE_MAJOR_CIVILIZATION` is **12** with `START_DISTANCE_RANGE_MAJOR`
**2**, so Civilization VI aims major civilizations 10-14 tiles apart (minors use
`START_DISTANCE_MINOR_MAJOR_CIVILIZATION` 6 and
`START_DISTANCE_MINOR_CIVILIZATION_START` 5, and there is a
`START_DISTANCE_FERTILITY_EXCLUSION_ZONE` of 6).

CIVVIS does not target a distance at all. `balanced_major_spawns` maximizes
spread — farthest-point layouts scored on separation, coverage, territory
balance and site quality — which on the tournament lobby's Standard 84x54 map
with 8 majors puts every civilization far outside the shipped band:

    major-major nearest-neighbour separation, 20 maps, 160 measurements
    min 17  max 23  mean 18.3  median 18
    within the shipped 10..14 band:  0 / 160  (0%)

Every single measurement is above the band; CIVVIS spreads civilizations about
50% farther apart than the game does. That changes the whole early game —
settling races, border friction, early aggression, and the Loyalty and religious
pressure that depend on how close neighbours sit. It is measured with
`mapdump --width 84 --height 54 --players 8 --city-states 0`, whose
nearest-neighbour separations are major-only when no minors are requested
(minors are appended after majors are placed, so major placement is identical
either way).

**Fixed.** `targeted_layout` now picks each start by how well its
nearest-neighbour distance fits the shipped band rather than by how far away it
can get, scoring the miss with `start_distance_miss` — zero inside 10..=14,
growing outside it, and counting crowding double, because two civilizations on
top of each other is a worse start than two a little too far apart. Measured the
same way afterwards:

    n = 160  min 10  max 15  mean 11.83  median 12
    within the shipped 10..14 band:  153 / 160  (96%)
    below 10: 0     above 14: 7

Zero crowded starts, a median of exactly the shipped target, and the seven
strays one tile over. `stock_map_profiles_produce_spread_and_complete_spawn_sets`
now asserts the band per start rather than a floor of six, and was confirmed to
bite by restoring the old maximizing rule (`Duel places a start 27 from its
neighbour`).

This changed every generated map, so layouts and league snapshots from before it
are not comparable.

**Where the Civilopedia and the database disagree, the database wins.** The
prose goes stale across rebalances and has been observed to be flatly wrong:
the Loyalty entry describes two penalty steps (below 75, below 25) where
`LoyaltyLevels` ships four, one per display band, and the Happiness entry
gives Ecstatic a +10% yield bonus where `Happinesses.NonFoodYieldModifier`
ships +20. Both readings were used to "correct" already-correct engine code.
Cite a table and a column, not a sentence.

## Resolved: buying a tile with Gold

`BuyPlot` is a complete legal action. A city may annex an explored, unowned
plot only when it touches that city's own territory and lies through ring 3;
`CITY_MAX_BUY_PLOT_RANGE` remains the shipped **3**. Applying the action
revalidates the live state and price, deducts Gold, assigns both tile and city
ownership, and immediately exposes the tile to yields, resources, Builders,
districts and Wonders. The browser lists every affordable plot with its exact
price and yields. Both AIs value resources, Natural Wonders and raw yields;
the strategic AI also values the district/Wonder sites ownership unlocks.

The executable-only curve was settled from real-game measurements rather than
inferred from the unrelated Culture-border curve. A
[measured vanilla sequence](https://www.realmsbeyond.net/forums/showthread.php?page=2&tid=8994)
established the 1x–5x farther-tree progression, while a
[current Gathering Storm Marathon sequence](https://www.reddit.com/r/CivVI/comments/1polncr/district_discount_and_tile_price_demo/)
identifies the present 77-technology and 61-civic denominators and the
rounding/discount order. CIVVIS executes:

    progress = floor(100 × max(completed techs / 77, completed civics / 61)) / 100
    price = floor_to_5(speed × ring base × (1 + 4 × progress)) × (1 - discount)

The [observed ring bases and legal range](https://civilization.fandom.com/wiki/Borders_%28Civ6%29)
are **50** Gold through ring 2 and **75** for ring 3 (ring 1 is normally
granted at foundation, but can exceptionally become neutral). Rounding to 5
comes after game-speed scaling but before Land Surveyors or Expropriation;
that otherwise easy-to-miss order is why a 20% discount produces multiples of
4 on Marathon. Regression tests reproduce the measured 8-tech/8-civic prices
of **180/272** and 8-tech/11-civic prices of **204/308**. The forged-price test
also posts a zero quote and proves the engine still charges its recomputed live
price, so the browser's quote is never trusted as authority.

## Open, and needing a judgement call: resource placement frequency

`Resources.Frequency` and `SeaFrequency` weight the shipped placement lottery.
CIVVIS picks **uniformly** among the resources valid for a tile:

```rust
let pick = valid[rng.below(valid.len())].clone();
```

The shipped weights are not close to uniform. On land, Stone and Wheat are 10
against Copper, Deer, Sheep and Bananas at 4 — two and a half times as likely.
At sea it is starker: Fish 23, Crabs 17, Turtles 5, and Amber, Pearls and Whales
1 apiece, so a Whale should be a twenty-third as common as a Fish and CIVVIS
makes them equally likely. Land luxuries are the one group that really is
uniform, all at 2, which CIVVIS gets right by accident.

**Why this is not simply fixed.** The lottery runs before start selection and
feeds it: resources are part of what scores a start, through
`StartBias::weight(resource_tier)`. Weighting the lottery therefore moves
starts. Implementing it made two of roughly a hundred sampled (size, seed)
combinations miss the start-spacing tolerance in
`stock_map_profiles_produce_spread_and_complete_spawn_sets` and
`islands_flat_poles_spread_starts_across_the_whole_archipelago` — both
marginally, 207% against a 200% cap and 64.7% against a 65% floor.

One or two marginal misses in a hundred is consistent with ordinary variance
around a tight threshold, and it is also consistent with a small real
degradation. n=1 does not separate them, and even start spacing is a
tournament-fairness property this project cares about. Loosening the tolerance
or reseeding until the sample passes would settle the argument in the change's
favour without evidence, so the code is not in the tree.

What would settle it: run the spacing property over a few hundred seeds with and
without the weighting and compare failure rates. If they match, the weighting is
correct and the thresholds simply sit close to the natural spread.

## Swept clean: the systematic comparisons already run

These are whole-axis comparisons, not spot checks. Each was run against every
shipped row whose owner CIVVIS models, and each came back with the divergences
listed — so re-running them is unlikely to pay unless the shipped database or
CIVVIS' content changes. Recorded here so the next pass starts somewhere new.

| Sweep | Scope | Found |
|---|---|---|
| Effect arguments | every `Modifiers` row on a modelled owner | the modifier census, see [MODIFIERS.md](MODIFIERS.md) |
| `Inverse` on effect modifiers | 809 rows, 76 inverted | Monasticism only |
| `Inverse` on `ATTACH_MODIFIER` wrappers | 2 on modelled owners | Just War and Defender of the Faith |
| `RequirementSetType` (`TEST_ANY` vs `TEST_ALL`) | 22 multi-requirement sets | none |
| `OwnerRequirementSetId` | 0 on modelled owners | not a live axis here |
| Requirements on `ATTACH` inner modifiers | 19 | none |
| `GlobalParameters` | 115 tracked of 500 shipped | see the ratchet |
| Ruleset fields with no consumer | 262 fields | `barb_force_scale` |
| `effects` keys with no consumer | 640 keys, 44 flagged | none — all read via prefix match, `format!` lookup or struct field |

Two lessons worth carrying forward.

**Read `Inverse` first.** It has produced three separate defects — Monasticism's
Culture penalty, and both army combat beliefs reaching Apostles. Every time,
CIVVIS had the elaborate part of the condition right and dropped the negation.

**A requirement can sit on either level of an `ATTACH_MODIFIER` pair.** Just War
carries its unit-class exclusions on the wrapper and its yield on the inner
modifier; a sweep that reads only the effect-bearing row sees neither.

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
civics, buildings and districts. All are now resolved — 45 by correcting
CIVVIS' data, the rest by recording them as deliberate:

| Fixed | Examples |
|---|---|
| Gathering Storm rebalances CIVVIS had at vanilla values | Knight 180→220 production and 3→4 maintenance, Pikeman 200→180 and 3→2, Military Engineer maintenance 4→2, Shipyard maintenance 2→1 |
| Modern-era building costs, uniformly overstated | Factory/Stock Exchange/Military Academy 390→330, Research Lab/Seaport/Broadcast Center/Film Studio/Shopping Mall 580→440, Stadium 660→480, Airport 600→480, Zoo/Aquarium 445→360, Hangar/Food Market 465→380 |
| Sight radii left at the default | Spy, Naturalist, Helicopter, Rocket Artillery, Giant Death Robot all see 3 tiles, not 2 |
| Missing prerequisites | Cartography and Mass Production both require Shipbuilding |
| Future-era research frozen into one representative layout | Technology and civic nodes now carry the shipped `RandomPrereqs` flags and 2200/2300 or 3200/3300 column costs. Each match draws one connected two-column graph from its seed, shares it across every player, and stores the concrete graph in saves. |

`tools/fidelity_waivers.json` holds the accepted divergences, each with a
reason — purchase-only units store a Faith price where the database stores an
unpayable production cost, and the City Center is placed rather than produced.
Future-era randomization is no longer waived: the audit reads
`Technologies_XP2`, `Civics_XP2`, `TechnologyRandomCosts`, and
`CivicRandomCosts` directly. **That file is the fidelity roadmap: shrinking it
is the work.** Anything not listed there counts against the ratchet, which now
stands at zero for these five tables.

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

**Eighth wave (the meteor):** the Apocalypse pack's meteor shower lands in
the engine, **behind its game-mode checkbox** (`--game-modes apocalypse`;
Secret Societies is the other one CIVVIS models). New Frontier modes are
off in a stock lobby and in every tournament lobby, so the baseline
ruleset sees none of this — about six strikes per game (the shipped Moderate frequency)
on the shipped strike terrains outside anyone's territory, each leaving
the `METEOR_GOODIES` site whose one-entry table grants the most advanced
Heavy Cavalry the finder can field, in their nearest city, exempt from
resource upkeep (the shipped refund modifier). The tribal-village roll
keeps its seven categories — the meteor's table is its own goody type,
popped only by its own site.

**Ninth wave (the weather):** Gathering Storm's random disasters now
happen rather than merely being resolvable. Every class — volcanic
eruptions, river floods, droughts, and the four terrain-bound storm
systems — rolls each turn against a per-game budget, scaled by the lobby's
disaster-intensity setting and again by the warming already banked, and
resolves through the protection rules that were already modelled (Dams,
the Great Bath, Egypt's Iteru, Aqueducts, Flood Barriers, and Governors'
Reinforced Materials). Volcanoes now have the shipped active/dormant
split, eruptions bury the ring they reach and leave Volcanic Soil, storms
drift for three turns and pay for their damage with fertility, and
droughts hold their tiles for a severity-scaled span before lifting.

Two honest boundaries around it:

- **The rates are calibrated, not shipped.** The tuning that lives in
  `Expansion2_RandomEvents.xml` — occurrences per game, severity weights,
  per-severity damage — is not published outside an installation, and the
  only figures that are public are the band a volcano's activity sits in
  (45%–95% across the five intensities) and the fact that intensities 3
  and 4 widen an eruption to two rings. Both of those are exact here. The
  rest lives in `data/disasters.json` precisely so it is visible, tunable
  and moddable rather than buried in Rust: the per-class `per_game`
  budgets, the intensity ladder, and the climate scaling are CIVVIS
  numbers chosen to land in the documented range, and a pinned tournament
  ruleset can replace the file wholesale. `validate` checks the file's
  shape, and a test asserts a full game lands near the rates it asks for.
- **Flood fertilisation stays off.** Gathering Storm gives a flooded
  Floodplains tile a chance at permanent extra Food and Production, and
  that probability is one of the numbers only an installation carries.
  The mechanism is implemented — `disaster_food`/`disaster_production` are
  real tile yields, and storms use them — but `river_flood`'s
  `fertility_chance` is zero until the shipped table can be read, because
  a guessed fertility rate changes what Floodplains are worth for the
  whole game.

**Both halves of every Dedication.** A Dedication in Civ VI is two rules,
not one: the Normal-Age half that turns the behaviour it names into Era
Score, and the Golden-Age half that only a Golden or Heroic Age switches
on. CIVVIS had the Golden halves and none of the Normal ones, which meant
choosing a Dedication in a Normal or Dark Age did nothing at all — and
since that Era Score is exactly what a Dark Age civilization climbs out on,
the ages below Golden had no engine behind them. All twelve Dedications
now carry both halves in `data/dedications.json`, including the two that
were missing entirely (Wish You Were Here, Bodyguard of Lies), with their
triggers wired to the seventeen moments that pay them. The per-Dedication
era spans are exact where the Civilopedia states them (Exodus of the
Evangelists through the Renaissance, Automaton Warfare and Wish You Were
Here in the last two eras) and era-appropriate where it does not.

**Dark Age policy cards.** A Dark Age also opens a Wildcard slot to cards
no civic unlocks: strong effects bought with a real drawback. The seven
with published effects are implemented and execute both halves — Twilight
Valor (+5 melee attack Strength, no healing outside your own territory),
Isolationism (+1 Trade Route capacity and +2 Food/+2 Production at home,
but no Settlers trained, bought or settled), Monasticism (Science doubled
in Holy Site cities, -25% Culture), Inquisition (+15 Religious Combat
Strength at home, -25% Science), Letters of Marque (+100% naval-raider
Production, +2 Movement, doubled plunder, -2 Trade Route capacity), Elite
Forces (+100% unit experience, +2 Gold per military unit) and Robber
Barons (+50% Gold with a Stock Exchange, +25% Production with a Factory,
-2 Amenities everywhere). They are offered only while the civilization is
actually in a Dark Age and inside the card's own eras, and an age change
takes them back out of their slot.

The ten cards Gathering Storm added (Collectivism, Cyber Warfare,
Decentralization, Despotic Paternalism, Disinformation Campaign, Flower
Power, Rogue State, Samoderzhaviye, Soft Targets, Automated Workforce) are
not modelled: their effects are not published in a form worth copying, and
guessing at a card's numbers is worse than not shipping it.

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
and the per-ring unit damage is the one number the database does not carry).

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
