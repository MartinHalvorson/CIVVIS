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

## Where the install is

`python3 tools/civ6_fidelity.py` takes no arguments: `tools/civ6_env.py` is the
one place that knows where Civilization VI lives, and every Civ VI tool asks
it. Override with `--civ6`, `$CIV6_INSTALL` or `$CIV6_DIR`, naming either the
install root or the assets directory inside the macOS app bundle.

⚠ Until 2026-08-19 this audit carried its own list of four candidate installs
and every one of them began `C:\` or `D:\`. This fleet runs on macOS, where
the gameplay database is not at the install root at all — it is inside the
signed bundle at `Civ6.app/Contents/Assets`. So the audit that checks we are
modelling Gathering Storm rather than some other ruleset answered "install not
found" on every machine that could have run it, and `tools/civ6_modifiers.py`,
which imports the same resolver, answered the same. Neither was broken; neither
had ever been asked a question it could answer. `civ6_env.ASSETS_SUBPATH` was
already documented as "the path the fidelity audit wants" and nothing had wired
it up.

That gap has a cost on the record: #2049 read the compiled cache directly to
get around this, bypassing the ruleset refusal, and shipped Vanilla belief
values as Gathering Storm. #2050 retracted them the next day.

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

- **`Buildings` — three uniques must follow the installed Gathering Storm rows,
  not stale expansion snippets.** The Prasat ships Faith **4** with two Relic
  slots; CIVVIS had 6 and one. The Sukiennice ships Gold **3**; CIVVIS had 2.
  The Tlachtli ships Culture **1**; CIVVIS had 2. The XML load order, not an
  isolated historical row, is the authority for all three.
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

## Closed: only a full civilization annexes tiles with its own yields

`CivilizationLevels` is the table that decides who may take ground, and only
one of its four rows may take it unaided:

| level | `CanAnnexTilesWithCulture` | `CanAnnexTilesWithGold` | `CanAnnexTilesWithReceivedInfluence` | `StartingTilesForCity` |
|---|---|---|---|---|
| `FULL_CIV` | 1 | 1 | 0 | 6 |
| `CITY_STATE` | 0 | 0 | 1 | 5 |
| `FREE_CITIES` | 0 | 0 | 0 | 0 |
| `TRIBE` | 0 | 0 | 0 | 0 |

A city-state's territory therefore grows **only** by one tile per Envoy it
receives from its Suzerain.
CIVVIS ran the ordinary Culture border curve for every city whatever its
owner, and `plot_purchase_cost` had no owner gate either, so a minor accreted
ground for the whole game on top of the Envoy tiles it was already paid.

Measured over four 250-turn games on the tournament lobby before the fix, a
city-state finished **larger than the average major city** — 23.8 tiles and
10.4 Population against 22.2 and 9.9 — on a mean of 8.6 Envoys from all
majors combined. Territory is the mechanism and Population is the symptom:
more owned tiles is more worked tiles is more Food.

`annexes_tiles_with_own_yields` now gates both paths.

### Deliberate divergence: a founding city-state takes its whole first ring

`StartingTilesForCity` is the one number in that table CIVVIS does not follow.
The column counts tiles *besides* the centre — Russia's Mother Russia is
`MODIFIER_PLAYER_ADJUST_CITY_TILES` with `Amount` **5**, and Russian cities
visibly open with the whole first ring plus five more, which only adds up if
the base 6 *is* the ring. So the shipped minor really does found one plot
short.

CIVVIS grants the full ring anyway. The count is shipped but the **choice**
is not: nothing in the database says which of the six a city-state gives up,
so it fell to the plot-influence picker, and the result was a neutral hole
inside a city's own first ring on every single city-state — measured at 5 of 6
owned across 96 city-states over 8 map seeds, always exactly one, never taken
by anybody else. On the map that reads as a rendering fault rather than a
rule. The gate above is what keeps a city-state small; this tile is worth
about 2 Food and costs the map its legibility.

The grant is still free ground only: a ring plot a neighbouring city already
holds stays with that neighbour, exactly as for a full civilization.

## Resolved: a city-state weighed nothing for itself

Every city-state in a 406-turn spectator game sat at exactly 100 Loyalty while
the observation reported it losing **-25 a turn**. Both halves were wrong, and
they were hiding each other.

`loyalty_change_for_city` skipped every minor-owned city when it summed
population pressure. That was meant to model the shipped rule that a minor
projects no pressure onto its neighbours — the population tooltip really does
drop the "also applies to other cities within 9 tiles" clause for minors:

> Ranges from +20 to -20, based on the comparison of pressure coming from
> nearby Citizens belonging to the city's owner and nearby Citizens belonging
> to a different civilization. 1 Pressure per Citizen normally. Increased in
> Capital.
>
> — `LOC_CULTURAL_IDENTITY_POPULATION_PRESSURE_TOOLTIP_MINOR_CIVS`

But the skip also removed a city-state's own Citizens from its *own* domestic
side. A pop-14 Kabul therefore compared **0** against its neighbours and pinned
the ratio at the -20 floor forever. `process_loyalty` then returned early for
minors, so the number was computed, published to the HUD, and never applied.
The city survived because the second bug cancelled the first.

Two shipped constants were missing beside it. `IDENTITY_PER_TURN_FROM_CITY_STATES`
is **20** and `IDENTITY_PER_TURN_FROM_FREE_CITIES` is **10** — "Base strength as
a City-State" and "Desire for independence" in the pressure breakdown. Only the
Free City half was paid.

CIVVIS now counts a minor's own Citizens as its domestic pressure while still
projecting nothing outward, pays the +20, and runs city-states through the same
`process_loyalty` as anybody else, so an overwhelmed one revolts into a Free
City. Replaying that same 406-turn checkpoint takes the count of cities that
claim to be bleeding Loyalty while pinned at 100 from **18 to 0**, and no
city-state flips: the most pressured of the eighteen still clears +15 a turn,
which is the balance the +20 base exists to produce.

The occupation term went with it. Rise & Fall charged a flat penalty between
`IDENTITY_PER_TURN_FROM_OCCUPATION_MIN` -1 and `_MAX` -5; Gathering Storm
rescaled the range to 0..10 and added `_MULTIPLIER` **25**, which the
Civilopedia reads as "Loyalty penalties based on the conqueror's Grievances
caused against the city's original owner". CIVVIS charged R&F's flat -5 and
cancelled it outright when a unit was garrisoned. It now charges 25% of those
Grievances clamped to [0, 10] whether or not anyone is garrisoned, and a
garrison instead pays the separate `IDENTITY_PER_TURN_FROM_MARTIAL_LAW` **+8** —
so holding a fresh conquest down with troops raises its Loyalty rather than
merely stopping the bleed.

Verified unchanged in the same pass: the ±20 clamp is exactly
`LOYALTY_PER_TURN_FROM_NEARBY_CITIZEN_PRESSURE_MAX_RATIO` 3.0 against
`_NEUTRAL_RATIO` 1.0; the 10%-per-tile falloff; `CITIZEN_IDENTITY_PRESSURE_BASE`
1 with `_CAPITAL` +1 and the Golden/Dark ±0.5 per-Citizen age terms; every
`Governors.IdentityPressure` is **8**; the Statue of Liberty's
`STATUELIBERTY_CITIES_ALWAYS_LOYAL` really does pin all cities within 6 tiles;
and the `LoyaltyLevels` yield/growth bands.

## Resolved: resource placement follows the shipped weights

`Resources.Frequency` and `SeaFrequency` weight Civilization VI's placement
lottery. CIVVIS now exports both columns from the shipped database and draws
each valid land or sea resource by its matching weight. That restores important
ratios such as Stone and Wheat at 10 versus Copper, Deer, Sheep and Bananas at
4, and Fish at 23 versus Whales and Pearls at 1.

The normal lottery excludes resources with a zero applicable weight. That is
intentional: artifacts remain assigned only by their later, dedicated quota
pass instead of becoming accidental ordinary resources. The fidelity audit
compares every exported land and sea weight against the installed game data; a
mapgen test pins the full shipped table and checks the weighted draws, including
the zero-weight case. Existing stock-map start and spawn properties continue to
exercise the changed generator without relaxed spacing thresholds.

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

## Measured: the live economy against the host, city by city (2026-08-16)

The rules-data audit above compares CIVVIS' *tables* with the game's. This
compares CIVVIS' *derived economy* with the game's, on a live mirrored seat —
the first stage of the differential stack described under Phase 4, shipped as
`tools/civ6_yield_drift.py`.

The mirror already corrects the board to the host: every own city carries a
host-to-model delta on its yields (and now on its Housing ceiling and Amenity
count, and on every worked plot where the export names the plot's yields), so
the numbers a viewer reads beside the Civilization VI window agree by
construction, and `tools/civ6_mirror_check.py` guards that per city. What those
corrections *hide* is how far CIVVIS' own model is from the game — the currency
every "what is a Library worth here?" is priced in. The instrument rebuilds the
decider's own board for every exported state record (`civvis_orders
--dump-mirror --turn N`), reads `city_yields_model` — the figure BEFORE the
correction — and diffs it against `city:GetYield` per city and per yield,
splitting the result into episodes that persist (a rule the model has wrong)
and transients (the host publishing a change one turn before or after the
model — a policy swap, a tech, a repair — which is timing, not a rule).

    python3 tools/civ6_yield_drift.py [run-dir] [--turns LO:HI] [--city NAME]

The first run over three completed Settler games named its causes, and the
fixes below shipped with it. Numbers are `|model − host| × turns` summed over
persistent episodes on run `civvis-20260816T011314Z` (turns 1–200, 8 cities),
the decider before this change against the decider after it, on the same
recorded exports; the whole-empire figures at turn 200 moved from
Production 183.8 / Gold 124.5 / Food 227.0 modelled against 178.8 / 131.2 /
220.0 reported, to 178.8 / 130.8 / 222.0:

| Yield | before | after | What was wrong |
|---|---:|---:|---|
| Production | 334 | 149 | a District plot in the host's worked list is a **specialist**, and the mirror imported it as a worked tile too — every specialist paid its slot yield AND the ground under the district (Cumae, two Campus + one Industrial Zone specialists: +2 Food, +4 Production for twenty turns) |
| Gold | 828 | 490 | Rome had only Trajan's leader ability; All Roads Lead to Rome (`TRAIT_FREE_TRADING_POSTS`, `TRAIT_GOLD_FROM_DOMESTIC_TRADING_POSTS`) is now `free_trading_posts` / `own_trading_post_route_gold` in `civs.json`, read by `Game::trading_post_route_gold` — Antium → Rome read +1 Gold in the host and 0 in the model for the route's whole life; the base rule (`TRADING_POST_GOLD_IN_FOREIGN_CITY` 1) pays foreign posts the same way |
| Food | 517 | 338 | the specialist double-pay above; the residual is tile-level and mostly the class only the host can know (below) |
| Science | 169 | 169* | a **pillaged building** was invisible: `HasBuilding` stays true for a raided Library and the export carried no pillage bit for buildings, so Antium paid +6 Science on a Campus the host had at +0 (t147–t170); `pillaged_buildings` now crosses (`CityBuildings:IsPillaged`) and the engine's existing pillage rule finally has its input. *Unchanged in this replay by construction: the export that carries the bit is new, so the episode closes on the next game |

Two findings could not be closed from totals alone, and are why the export
grew:

- **Fertility the ground remembers.** Rome on run `civvis-20260816T003229Z`
  read +12 Food and +5 Production over the model for forty turns, all of it on
  volcanic soil an eruption had left. Gathering Storm pays for a disaster with
  permanent tile fertility, cumulative and random; no rule reproduces it and no
  tile catalogue carries it. The mod now exports every worked plot's own
  `Plot:GetYield` (`worked[].yields`, `center_yields`), the mirror pays each
  plot what the host pays it (`observed_tile_yield_adjustments`, a delta like
  every other correction), and the instrument's TILES block names the exact
  plot — terrain, feature, resource, improvement — where CIVVIS' tile model and
  the host disagree.
- **A +4 Gold step in two cities on one turn** (Antium and Ostia, t102, both
  Library cities, nothing else in the export moved) was chased through
  policies, religion, envoys, city-states and trade routes without an answer.
  The mod now exports the host's own per-yield ledger, `City:GetYieldToolTip`
  (`yield_sources`, icon markup stripped) — the "+N from Buildings / Citizens /
  Districts / Trade Routes" a player reads in the city panel — so the next such
  step names its source instead of inviting a guess.

The export also gained the rest of the host's Housing ledger
(`housing_from_water/buildings/districts/civics/great_people/starting_era/great_works`),
the rest of its Amenity ledger (`amenities_great_people/religion/national_parks/
starting_era/improvements/districts/natural_wonders`) and its growth arithmetic
(`food_surplus`, `growth_threshold`, `growth_turns`, the three growth
multipliers), for the same reason: a modelled total that disagrees must be able
to name the term. Two model-side observations already fell out of the ledgers
that were there: Firaxis does not hand a luxury to the neediest city first the
way CIVVIS' allocator does (Rome sat at 3 luxuries → 0 with a deficit of three
while smaller cities held theirs), and the host's `amenities_needed` is
`ceil(pop/2)` exactly as `Game::city_amenities_required` has it.

What to expect from the instrument going forward: the two `after` columns are
this replay's floor, not the model's — the pillage and per-plot fields only
exist in exports written after this shipped. Run it on the first game the new
mod plays; the TILES and SOURCES blocks are where the remaining Food and Gold
episodes will name themselves.

### Round 2: what the first per-plot game named (2026-08-16, run `civvis-20260816T040537Z`)

The first game exported with plot yields and the host's ledgers made the
remaining causes legible in one pass. Grouping the 849 worked-tile
disagreements by (terrain, feature, resource, improvement, delta) put four
signatures at the top; two were rules, two were state only the host holds:

| Signature | Count | Verdict |
|---|---:|---|
| Tundra under **Ubsunur Hollow**: host 1 Food / 1 Production / 2 Faith, model 2 Food | 389 | rule — a natural wonder's tile pays the wonder's `Feature_YieldChanges` and nothing from the terrain or hills under it (`Rules::tile_yields`) |
| Plains + Horses + Pasture: host 2/2, model 2/3 | 90 | state — the pasture was pillaged; `IsImprovementPillaged` now rides in the tiles export as `p` and the mirror sets the tile |
| Floodplains (grassland and plains, with and without farms): host +1..+3 Food, +1..+2 Production over the model, varying by tile and turn | 143 | state — Gathering Storm river floods leave permanent fertility on the tile; no rule reproduces it and the per-plot correction carries it |
| Bare Plains Hills / Grassland Hills + Iron Mine: host +1 Food, mine paying nothing | 140 | state — a pillaged mine plus storm/flood fertility on the hill; same two mechanisms |

And the host's per-yield ledgers (`yield_sources`) settled three questions
that totals never could:

- **Loyalty does not touch Food.** "from Disloyal" / "from Wavering Loyalty"
  appears on culture, faith, gold, production and science in every banded
  city-turn (59 of them) and on food in none; a city in Unrest read "+7 from
  Worked Tiles", total 7, while `city_yields_inner` multiplied its Food by the
  band's **0**. `LoyaltyLevels.YieldChange` is the non-Food factor;
  `GrowthChange` is what a disloyal city pays, and `loyalty_growth_mult`
  already applies it. Fixed.
- **The Palace sits where the host's capital is.** Rome fell at t79; the host
  moved the Palace to Aquileia (`capital: true`) and the mirror paid it in
  Antium (the first city the export listed) to the end of the game — 5 Gold,
  2 Production, 2 Science, 1 Culture wrong in two cities, the largest
  persistent gap of the run. Every mirrored city now takes `IsCapital` from
  the export, rivals and city-states included, and `city_has_palace` follows.
- **"+2 from Incoming Trade Routes"** on Aquileia from t131 was Zhang Qian,
  activated there at t130 (`GREATPERSON_GOLD_FROM_INCOMING_FOREIGN_ROUTES`) —
  a rule the engine already has (`great_person_foreign_route_gold`) but the
  mirror could not count the routes it applied to: the export carried only
  routes this seat sends. Foreign and domestic incoming counts now cross per
  city (`incoming_routes`, gathered the way the shipped Trade Overview does,
  from every other player's outgoing routes); wiring the Great Person's
  permanent city effect from the `gp` event stream is the follow-up.

The ledger also confirmed two rates the model already had: 0.5 Science and 0.3
Culture per citizen (the tooltip truncates 0.29688 to "+0.2" — read totals, not
the printed decimals), and the Monument's "+1 from Modifiers" is its
full-loyalty Culture, which CIVVIS pays as `full_loyalty_culture`.

### Round 3: the audit's blind row, and the other civilizations (2026-08-16, run `civvis-20260816T115139Z`)

The next per-plot game put a new signature on top — every worked Fishing Boat
one Production under the host from the turn Colonialism landed — and chasing it
found two defects in **this audit**, not in the engine:

- **A duplicate Id in the shipped table.** `Improvement_BonusYieldChanges` keys
  its rows by `Id`, and Firaxis ships Id 225 twice (Camp/Gold/Synthetic
  Materials and Fishing Boats/Production/Colonialism). Keyed by Id, the loader
  kept one and dropped the other, so Colonialism's grant was never compared,
  never listed as "only in Civ VI", and never modelled. The table is now keyed
  by (improvement, yield, tech, civic) and `compare` treats a grant we pay that
  the game does not as a divergence rather than an unmeasured field.
- **The XML route never applied `<Expansion>_RemoveData.xml`.** The core
  directories are filtered by `FILE_PATTERN`, which names the rules files and
  not the retirement file each expansion ships at modinfo `Priority="1"` (394
  and 572 `<Delete>` rows). Every retired base row therefore stayed in the XML
  reference: Robotics granting Pasture Production (moved to Replaceable Parts by
  Gathering Storm) "confirmed" a CIVVIS grant the game no longer makes;
  Cartography and Mass Production kept a Shipbuilding prerequisite the game
  removed; Niter kept a Floodplains feature; the Sphinx kept Snow; the
  Industrial Zone kept a 1.5 mine adjacency; every base Boost was compared
  against a row the expansion re-declares. Applied first, as the modinfo does,
  the XML route now agrees with the compiled cache on all of them, and the
  four "divergences" the earlier fingerprint notes had recorded as CIVVIS
  corrections were CIVVIS being right against a stale reference. What remains
  between the two routes (12 fields, all matching the compiled cache: Pike and
  Shot upkeep, the Tagma, three unique buildings, Eyjafjallajökull's adjacent
  Food) is content-pack rebalancing the XML route still misses; the cache
  route — `--cache`, which now also finds the Aspyr build's nested path — is
  the reference for the running game.

Both engine-side corrections that fell out are in `tree_effects.json`:
Colonialism `fishing_boats_production: 1` (and the engine's Fishing Boats
branch reads it), Robotics loses `pasture_production`. Fingerprint re-pinned.

**The other civilizations.** The standings' rival Science and Culture were
CIVVIS's own derivation from whichever rival cities happened to be visible —
usually none — the one column of the mirror a viewer could never trust. The
host reads them for every player (its World Rankings and Deal screens call
`GetTechs():GetScienceYield`, `GetCulture():GetCultureYield`,
`GetStats():GetTourism`, `GetTreasury():GetGoldBalance` on other players), so
the rival record carries `science`, `culture`, `tourism`, `gold`,
`gold_per_turn` (net), `faith`, `faith_per_turn`, and its technology and civic
counts. The mirror puts the balances directly on the rival seat and uses the
five host yield rates as host-to-model deltas, just as it does for seat 0.

The rest of the player HUD is now an explicit fog-safe `public_stats` aggregate
for the active civilization and every met major: city count, total population,
Food and Production per turn, World Wonder count, suzerainty count, and both
nuclear-device counts. The exporter totals a player's cities but does **not**
send the hidden city identities, positions, improvements, or units that made
those totals. CIVVIS holds that aggregate apart from its reconstructed city
records, so an unseen empire can correctly read seven cities and 49 population
without inventing seven map locations. Technology/civic counts and tourism are
also surfaced in the public observation. `civ6_mirror_check.py` PUBLIC compares
every current HUD total, economy rate, research count, and tourism figure per
rival seat.

Still open on the other civilizations, in order of value: rival cities'
districts and buildings are not exported (a rival city record is name, size,
health, walls, capital), so a rival's economy and defence are modelled from
population alone — `Plot:GetDistrictType` on every revealed plot would carry the
districts at least; a rival's techs and civics cross as counts, not names;
city-state envoy totals from all players (only ours cross today).

### Round 4: the age that never crossed (2026-08-16, run `civvis-20260816T132247Z`)

The largest gaps of the next game were all Production in one Golden Age:
Cumae −10 and −13, Arretium −9.9, Rome −7, for thirty turns from t182. The
host's own production ledger named the source in one line — "+14 from
Districts: +4 Industrial Zone, **+10 from Campus**" — and the database named
the rule: `COMMEMORATION_INUDSTRIAL_GA_CAMPUS_MODIFIER`, Heartbeat of Steam's
Golden Age half, every Campus granting Production equal to its Science
adjacency. Three things were wrong at once:

- **The age itself never crossed.** `golden_age`, `dark_age` and
  `heroic_golden_age` were exported and read by nothing; `Player::age` sat at
  its "normal" default on every mirrored board, so `dedication_active` was
  false for the whole of every live Golden Age and no Dedication ever paid.
  Now set from the flags (Heroic outranks Golden), every rebuild and sync.
- **Which Dedications were active never crossed either.** The mod now exports
  `dedications` — `Game.GetEras():GetPlayerActiveCommemorations`, the accessor
  the shipped EraProgressPanel lists from — as `COMMEMORATION_*` type names,
  and the mirror maps them onto its own ids (`civvis_dedication_name`).
- **The engine had Heartbeat of Steam wrong anyway** — +1 Science per
  Industrial Zone building, which no row grants. It now pays each active
  Campus's Science adjacency as Production; Reform the Coinage pays +3 Gold per
  specialty district in the destination of an international route (it was a
  flat 3 on every route); Sky and Stars loses an invented ×1.10 (its rows grant
  tech boosts, air-unit XP and Aluminum, no yield).

And one rule the engine had that Civilization VI does not: **an age multiplies
nothing**. `city_yields_inner` scaled every non-Food yield by ×1.10 in a Golden
or Heroic Age and ×0.95 in a Dark Age; the shipped `Modifiers` carry no yield
term keyed on PLAYER_HAS_GOLDEN_AGE or a Dark Age beyond the named
commemorations (and Suleiman's trait, Tsikhe). Removed. This binds natively —
CIVVIS-vs-CIVVIS Golden Ages are now worth their Dedication and their Loyalty,
which is what they are worth in the game.

The same run's TILES block put another host-only class on top: volcanic soil
paying +2 Food, +1..3 Culture, +1..3 Science over the model — the
`RandomEvent_Yields` table (eruption fertility, by severity and volcano, with
Science and Culture terms CIVVIS's own `disaster_food/production` never
carried). Only the host knows how many times a tile has been buried; the
per-plot correction carries it. Modelling Science and Culture fertility
natively is a follow-up for the eruption code.

### Round 5: the first game with everything exported (2026-08-16, run `civvis-20260816T155856Z`)

With plot yields, pillage bits, incoming routes, the age and its Dedications
all crossing, the first full game read persistent Production 9 and Science 21
(from 913 and 279 the game before). Two classes were left, both named by the
host's ledgers in one line each:

- **The capital's Gold: 44 modelled against 17 reported for thirty turns.**
  The host's capital ledger was "+1 Harbor, +5 Palace, +11 Worked Tiles" and
  nothing else; the model's 27 was Merchant Confederation's Gold per placed
  Envoy (7 + 6 + 13 + 1 Envoys at four city-states). The card is
  `MODIFIER_PLAYER_ADJUST_YIELD_CHANGE_PER_USED_INFLUENCE_TOKEN` — income paid
  to the PLAYER, on the top bar beside the city sum, never a line of any city's
  ledger — and Raj (`..._PER_TRIBUTARY`) and an Emergency's Gold per Envoy are
  the same shape. `city_yields_inner` had paid all three into the Palace city.
  They now sit in `Game::player_policy_yields`, banked at turn end with the
  founder-belief income and reported by every consumer of the per-turn figure
  through `player_yield_extras`. (God King stays a capital-city yield: its
  modifier is `PLAYER_CAPITAL_CITY_ADJUST_CITY_YIELD_CHANGE`.)
- **Two Culture short in every coastal city.** "+2 from City Center" in Rome
  and "+2 from Wonder" in Mediolanum, beside the +2 the model already paid on
  each specialty district: Nan Madol's
  `MODIFIER_PLAYER_DISTRICTS_ADJUST_YIELD_CHANGE` reaches every district plot on
  or beside Coast, and the City Center and each wonder's plot are district
  plots too. `city_yields_inner` now pays them.

Both bind natively. The remaining Culture residual of that run is the same
Nan Madol term through the game's other coastal cities and closes with it.

### Round 6: a correction is measured after everything it corrects for (2026-08-16, run `civvis-20260816T175306Z`)

`civ6_mirror_check.py` on the refreshed board read the rivals over by their own
growth — Nubia 174 Science against the host's 141, 329 Food against 229 — and
Ravenna 14.5 Science against 9.5. Two orderings, both mine: the rival seats'
corrections were derived before the loop that writes a rival city's Population
(rival cities are planted at one), so the delta was measured against a
size-one city and paid on the size-eleven one; and the seat's own Dedications
(round 4) were applied after its correction, so a Golden Age paid Free
Inquiry's Science on top of a delta measured without it. Rival seats are now
corrected last in `apply_observed_host_metrics`, and `apply_player_ages` runs
before it on both paths (and still after it for the era score). The regression
test fails on the previous ordering by exactly those margins.

**The other civilizations, continued.** The first item on the round-3 list is
closed: the tiles export now names the district (`d`) and wonder (`wo`) on
every revealed plot — any owner — and the mirror puts them on the rival or
city-state city that owns the ground (`apply_foreign_infrastructure`, rebuilt
from every export so a razed district does not linger; our own cities keep the
city record, which carries completion and pillage). A rival's Encampment,
Campus or wonder is on the board from the turn it is seen.

### Round 7: the World Congress, incoming routes, and three rules the ledgers taught (2026-08-16, run `civvis-20260816T200454Z`)

The instrument's largest remaining episode was Cumae's Gold, four under the
host for fifteen turns, against a host ledger line the model had never heard
of: "+4 from Incoming Trade Routes", opening the turn a Maori route arrived and
closing at t102 while the route still ran. The rules database names the
figure — `TRADE_ROUTE_GOLD_CULTURAL_DOMINANCE` is 4 too, but the modifier that
actually carries an incoming-route Gold of 4 is `INCREASES_TRADE_TO_GOLD`, an
`EFFECT_ADJUST_TRADE_ROUTE_YIELD_FROM_OTHERS` owned by no game table because
`Expansion2_Congress.xml` attaches it: World Congress resolution
`WC_RES_TRADE_TREATY` ("Trade Policy"), option A, alongside
`TARGET_ADD_TRADE_ROUTE`. Our own trade capacity had gone 1→2 at t82 and 2→1 at
t102 for the same reason. Four things follow, all shipped:

- **The mod exports what the Congress has in force**, every turn: `resolutions`
  (`GetResolutions(pid)`, the call the shipped CityPanel makes between
  sessions; only entries with a `ChosenOption` are in effect, the same call
  returns the unfinished ballot during a session) as `{type, option 1|2,
  target}` plus `congress_turns_left`. The mirror maps them onto the model's
  own `active_congress_effects` (`civvis_congress_effect`: player targets
  become seats, `RESOURCE_`/`DISTRICT_`/`BUILDING_`/`PROJECT_`/`FEATURE_`
  targets their CIVVIS node names, class-like targets the engine's own
  suffixes; Sovereignty, Arms Control and the victory resolution have no model
  rule and are reported as `congress:…` in the unmapped list) — and applies
  them BEFORE the host-to-model corrections, for the round-6 reason. An export
  without the field leaves the model's simulated Congress alone.
- **Trade Policy A pays the chosen player's destination city, not the sender.**
  The resolution's text says "+4 Gold to the sender"; what Firaxis ships is a
  FROM_OTHERS effect on the chosen player, the same shape as Zhang Qian's
  destination Gold and Cleopatra's, and the host's ledger agrees (a domestic
  incoming route paid nothing, the sender's origin nothing extra). The engine
  used to pay the origin +4 in `route_yields`; it now pays the destination +4
  per incoming international route.
- **Rival routes into our cities are on the board.** `game.routes` carried only
  our own, so every destination-side rule (Zhang Qian, alliances, this one)
  paid nothing on a mirrored seat. `incoming_routes.origins[]` now names each
  foreign route's origin city and owner; the mirror seats the route on the
  rival's city with the rival's seat as owner (`restore_incoming_foreign_routes`),
  reporting rather than guessing an origin that is not on the board.
- **Three more rules from the same run's ledgers, each measured on its opening
  and closing turn.** (1) A pillaged destination district still pays its
  `District_TradeRouteYields` row: Rome's pillaged Holy Site and Campus kept
  Cumae's domestic route at "+6 from Outgoing Trade Routes" (t81-95) while the
  model, skipping them, paid 4–5, and the gap closed district by district as
  each was repaired; and a route made to Aquileia while its Diplomatic Quarter
  lay pillaged paid the Quarter's Food and Production from its first turn
  (t144) — the row is for the district's existence, not a figure frozen at
  the route's creation. (2) A pillaged district is not adjacent to anything: Rome's
  Campus read "+8 from Campus" beside a Government Plaza and a Holy Site and
  "+6" from the turn after the Holy Site was pillaged (t82) until its repair
  (t96) — Natural Philosophy doubling a base that had lost its district pair;
  Aquileia's Industrial Zone and Campus lost the same point while the
  Diplomatic Quarter between them lay pillaged (t142-145). The count excludes
  pillaged neighbours in the live rule and the planner's cached count alike.
  (3) An unemployed citizen pays half a Gold: with every workable plot taken
  and no specialist slot, Rome's Gold ledger read "+0.5 from Population" for
  one idle citizen (t81-96), "+1" for two (t97-106) and nothing once new plots
  were worked (t107); no other ledger moved (Science and Culture per citizen
  are paid regardless). The model now pays `0.5 × (pop − employed)`.
  (4) Percentage modifiers SUM; they do not chain. Rome under Merchant
  Republic (10% Gold with a Governor) with Kilwa Kisiwani (15% for a Trade
  suzerainty) read "+25 (+4.5) from Modifiers" on a base of 18 → 22.5, where
  the model's ×1.10 × ×1.15 read 22.77 (t146-149); and "-10 (-2) from
  Amenities | +10 (+2) from Modifiers" on 21 read exactly 21.0 (t150), which
  ×0.9 × ×1.1 would have made 20.79. Every `ADJUST_CITY_YIELD_MODIFIER`-shaped
  term in `city_yields_inner` — government, policy, wonder, Governor, suzerain
  bonus, industry, the Amenity band, Loyalty, the difficulty handicap — now
  lands in one per-yield sum applied once (floored at −100%).

Housing in the instrument now compares the floor of the model's figure, since
the host never reports a fraction; the persistent half-Housing offset of a
Farm was noise, not a finding. `state_changes` names `resolutions` and
`dedications` changes on an episode's opening turn.

Replayed on the run with the round-7 binary: persistent Food 21→0,
Production 8→0, Science 20→0, Gold 99→70 — 60 of the 70 is Cumae's Trade Policy
Gold, which this run's export cannot carry (no `resolutions`, no route
origins) and the first game launched with the new mod will; the other 10 is Ostia's path.

### Round 8: the route's own path (2026-08-16, run `civvis-20260816T200454Z`)

Ostia → Aquileia read "+2 from Outgoing Trade Routes" (t144-154) against a
model that found one Trading Post: the host's trader follows roads, and its
road ran through Cumae; the model's `route_path_cities` walks a straight
line. Rather than emulate the pathfinder, the mod now asks it —
`Game.GetTradeManager():GetTradeRoutePath(...)`, the call the shipped
TradeRouteChooser draws — and files each city plot on the path (origin
excluded, destination included) that answers `HasActiveTradingPost(pid)` by
owner: `trade_routes[].posts_own` / `posts_foreign`. The mirror keeps them in
`Game::observed_route_posts` keyed by (origin, destination) and
`trading_post_route_gold` pays them (own posts at Rome's trait Gold, foreign
at `TRADING_POST_GOLD_IN_FOREIGN_CITY`) instead of walking; an export without
the fields, or a route the pathfinder could not answer for, walks as before.

**Open after round 8**: the first game launched with the new mod should be
read for the treaty, the seated routes and the path posts with
`civ6_yield_drift.py`.

### Round 9: the Dark Age cards were guesses (2026-08-16, run `civvis-20260816T223457Z`)

The first game with the round-7 mod opened its Dark Age at t57 with
Inquisition slotted, and every city's Science ledger read "-25% from
Modifiers" beside its Amenity band; the model docked 15. `policies.json`
had -15, with a note that read like a paraphrase, and so did every other
Dark Age card: they were CIVVIS's own approximations, not the shipped rows.
Audited against `PolicyModifiers` → `Modifiers`/`ModifierArguments`/
`RequirementSets` and `Policies_XP1` (era windows) in the live rules
database — the ordinary cards (adjacency, trade-route, colonial taxes) all
matched; only the Dark Age set did not:

- **Inquisition** −25% Science (was −15). **Monasticism** −25% Culture in
  EVERY city (`MONASTICISM_CULTURE_MODIFIER` has no requirement set; the
  code used to dock only cities without a Holy Site). **Robber Barons** +50%
  Gold with a Stock Exchange and +25% Production with a Factory
  (`BUILDING_IS_STOCK_EXCHANGE` / `BUILDING_IS_FACTORY`; the code had
  rewritten this to Bank-or-Shipyard on a note that names a requirement set
  which exists nowhere in the Expansion data). **Isolationism** +2 Food and
  +2 Production on domestic routes that stay on one continent
  (`Intercontinental=0`) — no route capacity, no Gold, which were both
  invented. **Letters of Marque** initially appeared to supply +100% plunder
  from every unit and −50% Trade Route yields at both ends, but the later
  resolved Byzantium & Gaul removal data retires that policy entirely in the
  target ruleset. **Elite Forces** +2 maintenance (was 1) and
  Industrial–Future (was Classical–Renaissance).
- **Six cards CIVVIS never carried**, now on the roster with the shipped
  amounts: Collectivism (Modern–Atomic: +2 Housing, +1 Food per worked
  Farm, +100% Industrial Zone adjacency, −50% Great Person points), Rogue
  State (Atomic+: +50% toward the Manhattan Project, Operation Ivy and the
  nuclear/thermonuclear devices; no Influence), Flower Power (Atomic+: −100%
  unit Production, +100% unit purchase cost, free Rock Bands, +50% concert
  Tourism), Cyber Warfare (Information+: +10 Combat against Information-era
  civilizations; Grievances against you never decay), Automated Workforce
  (Information+: +20% project Production, −1 Amenity, −5 Loyalty per turn),
  Disinformation Campaign (Information+: −10% Science, −10% Culture, +3
  Favor per Broadcast Center).

`Yields::scale`, `Game::same_continent` and the destination-side route
yields gathered into one figure (`iys`) are the engine's new seams; the
policy effect vocabulary grows by `domestic_same_continent_trade_*`,
`trade_route_yield_pct`, `stock_exchange_city_gold_pct`,
`factory_city_production_pct`, `city_housing`, `farm_food`,
`great_people_pct`, `nuclear_project_production_pct`,
`project_production_pct`, `no_influence`, `unit_production_pct`,
`unit_purchase_cost_pct`, `rock_band_purchase_discount_pct`,
`rock_band_concert_tourism_pct`, `combat_vs_information_era`,
`no_grievance_decay`, `city_loyalty`, `favor_per_broadcast_center`.
Fingerprint re-pinned. The lesson generalises: a `note` that paraphrases
instead of naming the row it came from is a guess until measured.

**A placed district is not adjacent until it is built.** The same run's
Ravenna read its Commercial Hub one adjacency point over the host for thirty
turns (model 3, host 2) beside a city-state Encampment the tiles export
named — `Plot:GetDistrictType` answers from the turn a district is placed.
Our own placements — Puteoli's Campus beside its Hub (t108-119), Arpinum's
Industrial Zone beside its Campus (t131-140), Ostia's Theater beside its
Campus (t196-198) — held the neighbour's adjacency flat until the turn the
district completed, then moved it. The tiles export now carries `dc`
(`CityManager.GetDistrictAt(x, y):IsComplete()`) beside `d`, and
`apply_foreign_infrastructure` plants only what is built; our own cities'
records already carried completion.

### Round 10: a district project's per-turn yield is a city yield (2026-08-16, run `civvis-20260816T223457Z`)

Rome read 32.17 Gold against a model 25.20 for the three turns it ran
Commercial Hub Investment (t112-114), Ravenna 14.17 against 11.70 — the
host's ledger line is "+7.7 from Commercial Hub Investment", 30% of the
city's Production rate (`Project_YieldConversions.PercentOfProductionRate`:
Commercial Hub 30% Gold; Campus/Theater/Holy Site 15%; Encampment and
Harbor 15% Gold), filed as a base line the Amenity band then scales. The
engine already paid the conversion, but only inside turn processing; the
city's yield never carried it, so every project turn read short. It is now
computed in `city_yields_inner` from the finished Production rate, before
the percentage sum, and turn processing adds nothing on top.

**A city-state at war with the seat suspends its Envoy bonuses.** Ostia's
"+2 from Consulate" Culture (Caguana, three Envoys) went to nothing the turn
Caguana's new Suzerain brought it into a war against us (t90) and came back
the turn peace was made (t98); Rome's capital point from the same city-state
went and returned with it. `envoy_yields` now skips a city-state the seat is
at war with — the mirror already carries `minors[].at_war` onto the board.

**Exodus of the Evangelists pays +4 Great Prophet points a turn.** The
player-level Faith block read the host 3.7–4.6 a turn over the model for
t65-91: "+8 from excess Great Person points" against a model 3.45. Rome's
Holy Site, Shrine and Temple make 3 a turn; the Golden Age dedication
(`COMMEMORATION_RELIGIOUS_GA_GREAT_PROPHET_POINTS`, Amount 4) makes it 7,
and Classical Republic's 15% makes it 8.05 — the host's 8.04. With the
Prophet class exhausted every one of those points is Faith. Now in
`great_person_points_per_turn`.

### Round 11: the host houses its own Great Works, and Rationalism reads raw adjacency (2026-08-17, run `civvis-20260816T233226Z`)

- **A Relic the host kept in Rome's Palace read "+6 from GreatWorks" there
  while the model paid Mediolanum** — the model houses works by its own
  best-slot heuristic (a Relic goes to St. Basil's over the Palace), and for
  twenty turns Rome read 6 Faith under and Mediolanum 6 over. The export
  already names each work's city, building and slot; the mirror now keeps
  that placement (`Game::observed_great_work_housing`) and `housed_great_works`
  returns it for the seat instead of distributing.
- **Rationalism, Free Market, Grand Opera and Simultaneum read the district's
  own adjacency.** Ostia's Campus showed "+6" (3 doubled by Natural
  Philosophy) and Antium's "+4" (2 doubled) with Rationalism slotted, and
  neither city's Library or University earned a point (t153-169); the model
  paid both +3. `REQUIREMENT_CITY_HAS_HIGH_ADJACENCY_DISTRICT Amount=4` is
  met by the district's adjacency before the percentage cards, so the clause
  now sums the adjacency sources without the `adjacency_bonus` line.

### The rules data is at parity and the gap is coverage (2026-08-18)

`tools/civ6_fidelity.py --civ6 <install>` against the real Gathering Storm
install: **0 divergent fields across 27 tables**, 1,367 fields compared. The
numbers are right. What the report shows instead is the *only in Civ VI*
column — content the game has and CIVVIS does not model at all:

| table | only in Civ VI |
|---|---:|
| GreatPeople | 184 |
| Units | 58 |
| Promotions | 26 (16 of them spy promotions) |
| Beliefs | 22 |
| Projects | 17 |
| Policies | 11 |

**Pantheons were the largest coherent piece of that**: the game has 23 and
`beliefs.json` had 6. The pantheon is the earliest religious choice every
civilization makes, and religion decides about three quarters of the games on
the evaluator's own board.

Five were added, chosen because the existing per-improvement pantheon socket
already expressed them — `pasture_culture` and `fishing_boats_production` are
the same shape — so the engine surface is one predicate rather than five:

| pantheon | Gathering Storm | note |
|---|---|---|
| Goddess of the Hunt | +1 Food, +1 Production from Camps | |
| Stone Circles | +2 Faith from Quarries | |
| Goddess of Festivals | +1 Culture from Plantations | ⚠ expansion **deletes** the base game's Food row |
| Religious Idols | +2 Faith from Mines over Bonus/Luxury | two modifiers, one per class |
| God of Craftsmen | +1 Production, +1 Faith from improved Strategic | ⚠ expansion **deletes** the base game's Mine-only row |

Two of the five are cases where a compiled cache from a base-game machine
states the opposite of the shipped rule. Every id was checked against
`Expansion2_RemoveData.xml` before being modelled, which is the discipline the
entry below exists to enforce.

**★★★ And the content would have been unreachable.** The AI's pantheon chooser
was a hand-written list of exactly the six that existed, tried in order,
stopping at the first that took. A pantheon is exclusive, so in an eight-player
game — the `audit` and `soak` default — the seventh and eighth empires found
every name taken and founded **nothing**, holding the faith for the rest of the
game. The list is now a preference prefix over a roster read from the rules,
which is what the follower and founder choosers twenty lines below it have
always done. `every_major_can_found_a_pantheon_when_there_are_more_majors_than_favourites`
fails against the old list with two empires reading `None`.

⚠ The frozen anchor does **not** move: its five profiles have at most six
majors, so the named six still answer every one of them and no rated game
changes. That also means these five are not yet reachable on the six-player
deployment profile — they are reachable in eight-player games, and they are
what the live mirror needs to represent a rival's pantheon at all.

**Twelve pantheons remain**, and they need engine surface the improvement
socket does not provide: Holy Site terrain and feature adjacency (Desert
Folklore, Dance of the Aurora, Sacred Path), post-combat yields (God of War),
healing (God of Healing), district amenities and housing on a river (River
Goddess), first-district production (City Patron Goddess), wonder-era
production (Monument to the Gods), barbarian-camp dispersal faith (Initiation
Rites), feature yields (Lady of the Reeds and Marshes, Goddess of Fire) and
appeal (Earth Goddess). Their authoritative definitions are in the install.

### The Founder beliefs were already right, and a compiled cache said otherwise (2026-08-18)

**This entry is a retraction, and the most useful thing in it is how the
mistake was made.**

`tools/civvis_inert.py` gained a mirror direction — which keys does the engine
*price* that no data supplies — and it reported four dead arms in
`founder_belief_yields`: `gold_per_followers`, `culture_per_foreign_followers`,
`faith_per_foreign_city`, `gold_per_foreign_city`. Reading the compiled
gameplay cache at `Cache/DebugGameplay.sqlite` showed modifier ids that named
the distinction outright — `TITHE_GOLD_FOLLOWER`,
`WORLD_CHURCH_CULTURE_FOREIGN_FOLLOWER`, `PILGRIMAGE_FAITH_FOREIGN_CITY`,
`CHURCH_PROPERTY_GOLD_CITY` — so #2049 rewrote `beliefs.json` to match, on the
argument that Civilization VI pays a founder for converting its rivals.

**The cache held the base game.** It is whatever ruleset Civilization VI last
ran, and `civ6_fidelity.py` has refused a non-Gathering-Storm reference since
#1946 for exactly this reason — but that refusal lived in `main`, and #2049
opened the same file with three lines of `sqlite3` and never met it.

`Expansion2_RemoveData.xml` deletes all four of those modifiers.
`Expansion2_Beliefs.xml` replaces them:

| belief | base game | **Gathering Storm** | `beliefs.json` |
|---|---|---|---|
| Tithe | +1 Gold per 4 followers | `TITHE_GOLD_CITY` — **+3 Gold per following city** | `gold_per_city: 3` ✓ |
| World Church | +1 Culture per 5 foreign followers | `WORLD_CHURCH_CULTURE_FOLLOWER` — **+1 Culture per 4 followers** | `culture_per_followers: 0.25` ✓ |
| Pilgrimage | +2 Faith per foreign city | `PILGRIMAGE_FAITH_CITY` — **+2 Faith per following city** | `faith_per_city: 2` ✓ |
| Church Property | +2 Gold per city | **deleted** | absent ✓ |

The data was correct on all four. #2050 reverted it, and the revert is verified
rather than asserted: the frozen anchor returned to 18,572 decisions and
`0x3bda_c2f2_b84d_30fc`, and the ruleset fingerprint to
`fnv1a64:585ff2655ffd3a6d`, all unchanged from before #2049.

**Two things were kept, because both are real.**

- The mirror direction of `civvis_inert.py` stays, with the three arms waived
  by name in `BELIEF_YIELD_WAIVERS` and the ruleset reason spelled out for
  each. They are the base game's forms of beliefs Gathering Storm replaces, so
  the engine implementing both and the data selecting one is correct — and now
  it is *written down*, which it was not when #2049 read the same evidence and
  drew the opposite conclusion. `gold_per_foreign_city` has no belief of its
  shape in **either** ruleset and stays removed.
- `load_cache_database` now refuses a foreign ruleset itself rather than
  leaving that to its caller, with `require_gathering_storm=False` as the one
  named way past it, and a test that opens an empty database and expects the
  refusal. A guard beside the door protects only the people who walk through
  that door.

⚠ **One genuine divergence surfaced and is deliberately not fixed here.**
Gathering Storm's Cross-Cultural Dialogue is `BELIEF_YIELD_PER_FOLLOWER`
(+1 Science per 4 followers); `beliefs.json` has
`science_per_foreign_followers: 0.25` — the right rate on the wrong scope. It
is a real correction, it needs an engine arm that does not exist yet, and it
is not going into a revert.

### Faith at the empire level: unused Great Person points and a religion's own beliefs (2026-08-16, run `civvis-20260816T123936Z`)

Rome's Faith per turn diverged from the host by more than half, and the
city-by-city instrument could not see most of it because most of it is not in
any city. From t231 the host banked 100–113 Faith a turn while every city
together made 49 and the model read 33; Rome itself read 35 in the host and 23
in the model. Three causes:

- **Civilization VI pays the Great Person points of a class the empire can no
  longer earn out again as Faith, one for one.** The game core's
  `GetFaithFromUnusedGreatPeoplePoints` (`Player::GreatPeoplePoints`, visible in
  the shipped `.map` symbols); the top-bar tooltip files it under "from Other".
  A class is exhausted once the last named individual anywhere is claimed —
  the mod now says so outright (`great_person_exhausted`, a list so that
  "nobody exhausted" and "everybody exhausted" both encode; before it, a class
  with points and no `great_person_costs` entry was the same answer except
  on the turn every class was gone) — and the Prophet the moment the empire holds a
  religion, has one pending, or the map's religions are all founded. Measured
  across seven live games as the balance's next-turn change minus the city
  sum: after the last Great Scientist was claimed the empire gained the Campus
  rate to the point (ratio 0.97–1.10, 19–32 turns per game), a Holy Site's
  Prophet points arrived as Faith from the turn we founded a religion (with
  other Prophets still on offer) or the map ran out of them, and by t239 the
  five exhausted classes paid 60 a turn. `Game::great_person_class_earnable`,
  `unused_great_person_faith` and `player_yield_extras` carry it; the mirror
  reads the host's roster into `live_great_person_exhausted`; the HUD, the
  `--dump-mirror` JSON and the player-level correction all add the extras.
- **A religion's follower beliefs belong to the city that follows it, whoever
  founded it, and the mirror could not say which religion held which belief.**
  Rome and Ostia followed a Catholicism founded elsewhere; Divine Inspiration
  (`MODIFIER_SINGLE_CITY_ADJUST_WONDER_YIELD_CHANGE`, +4 Faith per Wonder in a
  following city) was neither in `beliefs.json` nor attributable from the
  union `taken_religion_beliefs`. The mod now exports each religion with its
  founder and beliefs (`religions`), the mirror seats them on the founder's
  seat (rivals hold seats in host order), and Divine Inspiration, Reliquaries
  (`MODIFIER_SINGLE_CITY_ADJUST_GREATWORK_YIELD` ScalingFactor 300 on Relics),
  Lay Ministry (`BELIEF_YIELD_PER_DISTRICT`: +1 Faith per Holy Site, +1
  Culture per Theater Square in following cities, to the founder) and Sacred
  Places (`BELIEF_YIELD_PER_CITY_WITH_WONDER`: +2 of each yield per following
  city with a Wonder) are modelled from the database's own modifier rows.
- **The export carried no host Faith rate to be corrected to.** `science` and
  `culture` were per-turn rates from the host; `faith` was a balance.
  `faith_per_turn` (`GetFaithYield`, the top bar's figure) and `faith_sources`
  (`GetFaithYieldToolTip`) now join the player-level correction,
  `great_person_points_per_turn` lets the model's per-class rate be judged
  against the host's, `civ6_mirror_check.py` guards the board's faith/turn,
  and `civ6_yield_drift.py` gains a PLAYER block — reading the host income
  from `faith_per_turn`, or on an older export from the balance's next-turn
  change where no purchase intervened.

Replayed with the new decider on the recorded run: Rome 35/35, Ostia
12.6/12.6, the empire 114.6 against a host income of 113 at t238 (the model's
Scientist rate is 31 to the host's 30 and its Writer 17 to 16 — the per-class
export will name that next), and the city-level Faith residual over t100–239
falls from 58.8 persistent + 94.8 transient to 0. Two smaller readings from
the same instrument stand open: the model's Great Person rates against the
host's once `great_person_points_per_turn` is in the export, and whether
Anarchy suspends the payout (the whole Faith line reads "No Faith due to
anarchy"; the engine follows that).

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
packs. That column is the content backlog. **Features is now empty**: the eight
Natural Wonders it used to name (the Bermuda Triangle, Eyjafjallajökull, the
Fountain of Youth, Lysefjord, Païtiti, Mount Roraima, Tsingy de Bemaraha and
Sahara el Beyda) all exist, so CIVVIS carries the whole thirty-four-wonder
roster with the shipped yields, appeal, impassability and sight.

**The audit's own loader had two blind spots, and both of them libelled a
wonder.** Fixed together with the roster:

- An expansion ships a compatibility overlay that applies only when the *other*
  expansion is installed. `DLC/Expansion1/Data/Expansion1_Expansion2.xml` is
  Gathering Storm's rebalance of Rise and Fall content, and sorted filename
  order applied it *before* the rows it edits existed — so every `<Update>` in
  it silently matched nothing. The Eye of the Sahara kept Rise and Fall's 1
  Production against CIVVIS' correct 2, and Pike and Shot's maintenance was
  read as 4 when Gathering Storm sets it to 3 (that one was a real CIVVIS
  error, now fixed). Cross-expansion overlays are applied last.
- `RemoveData` files were excluded as cosmetic. They are how the later packs
  retire content: Byzantium & Gaul deletes the Biosphere's `+8 Science` when
  Gathering Storm is active, so the audit reported CIVVIS as missing a yield it
  is correct not to have. Loading them also retires Twilight Valor and Letters
  of Marque. CIVVIS now removes both cards and treats any policy that exists
  only on its side as a fidelity divergence, so that a future retired card
  cannot hide behind the report's informational "Only in CIVVIS" count.

With both fixed, **Wonders and Features compare 53 and 50 entries with zero
divergences and nothing missing on either side.**

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
camps), and each new outpost now receives its fortified anti-cavalry guard
and land/naval recon role; nuclear device stats live in `data/wmds.json`
with maintenance charged from data; the Cultural Heritage Inspiration now fires via a
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

**Dark Age policy cards.** A Dark Age also opens a Wildcard slot to
era-appropriate cards with no civic unlock: strong effects bought with a real
drawback. The five early cards detailed here are Isolationism (+2 Food/+2
Production on qualifying domestic routes, but no Settlers trained, bought or
settled), Monasticism (+75% Science in Holy Site cities, -25% Culture),
Inquisition (+15 Religious Combat Strength at home, -25% Science), Elite
Forces (+100% unit experience, +2 Gold per military unit) and Robber Barons
(+50% Gold with a Stock Exchange, +25% Production with a Factory, -2 Amenities
everywhere). They are offered only while the civilization is actually in a Dark
Age and inside the card's own eras, and an age change takes them back out of
their slot. Twilight Valor and Letters of Marque appear in the older expansion
rows but are deleted by Byzantium & Gaul's all-content Gathering Storm removal
data, so CIVVIS neither offers nor executes them.

### Next inside phase 1

Known simplifications not yet expressed in data: civic-gated valid terrain
(farms on Hills at Civil Engineering, already era-exact through
`tree_effects`' hill_farms), wonders' widening `Building_ValidFeatures`
rows (CIVVIS is the permissive side there), theming (complete: works are
era-and-creator pieces, museums theme on the shipped rules — three
artists for art, three origin civilizations for artifacts — with the
+100% bonus), the DLL-side details of barbarian camp spawn cadence and
boldness (placement floors, outpost roles, and naval production are modeled),
and WMD delivery detail
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

The first runtime slice is now checked in: `ModifierSpec` carries an explicit
`player`, `player_cities`, or `player_units` collection and a validated
`all`/`any`/`none` requirement set. The collector evaluates those predicates
against the current player facts without cloning state, and static rules-object
attachments reject contextual bundles rather than applying them unconditionally.
This is interpreter infrastructure, not a claim that the 698 effects are done;
the shipped modifier catalog stays empty until rows are imported from the
compiled database.

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

### The trace comparator (the dynamic boundary that is now executable)

`tools/civ6_differential.py` is the first reusable piece of stage 2. It compares
two JSONL traces without starting either engine, which makes it cheap enough to
run on every replay artifact and in CI:

```sh
python3 tools/civ6_differential.py \
  --oracle traces/stock.jsonl --candidate traces/new.jsonl \
  --require-contiguous
```

The selected transition records (by default `state`, `orders` and `turn`) remain
in source order. Each is keyed by `(turn, phase, occurrence)`;
reordering a phase, dropping a state, or appending a duplicate therefore fails
before a later matching score can hide it. The payload is canonicalised and
SHA-256 hashed, with the first differing JSON-pointer path and both leaf values
reported. `--json` emits the same result as a machine-readable report for a
golden-corpus gate. Exit status is 0 for equal traces, 1 for a semantic or
structural drift, and 2 for a malformed or contract-invalid trace.

Only transport envelope fields (`run`, `ctx`, revision and timestamps) are
ignored by default. Decision fields such as an order's `source` remain strict.
Set-like schema fields whose
order is not a transition (`techs`, `civics`, `policies` and religion lists)
are canonicalised as unordered; all other lists remain ordered unless a
reviewed `--unordered /path` waiver is supplied. A waiver is a schema decision,
not a way to make a failing replay green: the command records no silent
wildcards, and duplicate JSON keys, non-finite numbers, backwards turns and
missing selected turns fail closed. Live tails may opt into one unterminated
final line with `--allow-trailing-partial`; golden traces should not.

The repository now carries a small sanitized transition spine in
`tests/fixtures/differential/`. `tools/check_differential_golden.py` validates
its strict frame keys and canonical payload hashes, compares a transport-
reencoded candidate, and runs a deliberate mutation that must be reported at
the changed JSON pointer. The committed `manifest.json` is a reviewable hash
ratchet: changing a fixture or canonicalisation rules requires updating the
expected transition proof in the same change.

This corpus does not pretend to be a full turn-0 replay. The action injector
and forced-randomness recorder still have to produce the candidate trace. Once
they do, this boundary is the part that turns their output into a deterministic
first-divergence test instead of a final-score eyeball check. The hermetic
contract remains pinned in `tools/test_civ6_differential.py`, and the golden
ratchet runs in the required CI test job.

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
