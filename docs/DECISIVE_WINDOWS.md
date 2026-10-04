# Decisive windows: where the tech and civic trees decide a war

Status: study + two opt-in genes (`decisive-window`, `unique-unit-preference`)
and the unique-unit roster completed from the shipped game database.
Written 2026-10-04 for the operator goal "study the most decisive tech
advantages in the Civ VI tech and civics trees, tech to decisive points and
launch the attack; every civilization techs to its unique unit and uses it".

## 1. What decides a city assault

A city falls when two things hold at once: **something opens its walls**, and
**something that can capture beats what defends it**. Both move in tiers, and
a handful of technologies set the tiers.

### Wall tiers (engine: `city_max_wall_hp`, `siege_support_effects`)

| Rival technology | Wall pool | What stops working |
|---|---|---|
| Masonry | 100 | — (a Battering Ram lets melee hit the city at full strength) |
| Castles | 200 | Battering Rams (`battering_ram_immunity`) |
| Siege Tactics | 300 | Siege Towers (`siege_support_immunity`) |
| Steel | 400 in **every** city (Urban Defenses) | every support unit |

Castles lists only Construction as a prerequisite and Siege Tactics only
Castles, so a rival holding Castles is always one step from Renaissance walls.

### Breakers (bombard strength vs the walled City Center)

A siege shot does `30·e^((B − C)/25)` to the walls, where C is the walled
City Center strength (strongest unit − 10, +3 per wall level, +3 Palace).
Two guns empty a pool in six turns when one shot does at least pool/12.

| Breaker | Unlock (std path) | B | Opens |
|---|---|---|---|
| Battering Ram | Masonry (105) | — | tier 1, melee/anti-cavalry only |
| Siege Tower | Machinery (805) | — | tiers 1-2, melee/anti-cavalry only |
| Catapult | Engineering (305) | 35 | tier 1 reliably, tier 2 against a weak city |
| Trebuchet | Military Engineering (890) | 45 | tier 2 |
| Bombard | Metal Casting (3105), Niter | 55 | tier 3 |
| Artillery | Steel (9785), Oil | 80 | tier 4 |
| Bomber | Advanced Flight (12810), Aluminum | 110 | tier 4, from 10 tiles |

### The assault ladder (strength bought per beaker)

| Assault | Unlock (std path) | Strength | Notes |
|---|---|---|---|
| Horseman | Horseback Riding (195) | 36, 4 moves | the cheapest +16 over a Warrior |
| Swordsman | Iron Working (225) | 35 | needs Iron |
| Man-at-Arms | Apprenticeship (690) | 45 | |
| Knight | Stirrups (1080) | 50, 4 moves | best Medieval strength per beaker; Stirrups lies on the Advanced Flight path |
| Musketman | Gunpowder (2375) | 55 | Niter |
| Line Infantry / Cavalry | Military Science (3930) | 65 / 62 | the Llanero is Gran Colombia's Cavalry |
| Infantry | Replaceable Parts (8275) | 75 | Oil |
| Tank | Combustion (12405) | 85 | Oil |

Class matchups the engine resolves: anti-cavalry +10 against light/heavy
cavalry, melee +5 against anti-cavalry. A margin of +5 strength is 37 damage
dealt for 24 taken — the first margin that wins trades; +10 is 45 for 20.

## 2. Live evidence: we arrive one tier late, every tier

30 live King Gran Colombia games, 2026-10-01..04 (state-frame census of our
`techs` against each rival's `tech_names`). Turn first held; "best" is the
first rival in each game, then the median across games.

| Technology | What it is | Us | Best rival | All rivals |
|---|---|---|---|---|
| Castles | rams stop working | 130 (22/30) | 76 | 88 |
| Siege Tactics | towers stop working | 169 (18/30) | 96 | 106 |
| Steel | 400 walls everywhere | 176 (13/30) | 137 | 142 |
| Military Engineering | our Trebuchet | 105 | 72 | 81 |
| Metal Casting | our Bombard | 139 | 90 | 101 |
| Stirrups | our Knight | 96 | 74 | 83 |
| Military Science | our Llanero | 171 (16/30) | 104 | 114 |
| Military Tradition (civic) | flanking, Maneuver | 75 | 34 | 52 |
| Nationalism (civic) | Corps | 150 (20/30) | 106 | 114 |

Our Trebuchet came after the rivals' Castles, our Bombard after their Siege
Tactics and level with their Steel. **Not one Llanero existed at any
checkpoint in any of the 30 games.** The tech count gap (23 vs 31 at t100) is
the science lane's; the *order* is this lane's: G47's research from t65 was a
chain of "modernize the standing army at war" single steps (Machinery,
Gunpowder, Rifling, Steel, Combustion, Plastics, Composites) plus the
bomber beeline, so the army was always one generation behind the walls it
marched on.

Declarations were never the problem: they open at 300-800 power against
30-170. Captures are.

## 3. The decisive windows for a King Gran Colombia seat

| Window | Package | Closes when the rival gets |
|---|---|---|
| W1 (t20-45) | Archers + Warriors (the conquest opening) | Masonry |
| W2 (t40-75) | Swordsmen/Horsemen/Man-at-Arms + Battering Rams | Castles |
| W3 (t60-100) | Knights + Siege Towers/Trebuchets | Siege Tactics, Pikemen |
| W4 (t95-140) | Line Infantry/Llaneros + Bombards | Steel (Urban Defenses) |
| W5 (t140+) | Bombers, Artillery, Tanks | — |

## 4. `decisive-window` (opt-in gene)

For the campaign target (the plan's, else the nearest legal major) it reads
the wall tier the target holds — or the next one when that technology is
already open to it — and the strongest defender it trains **now**. It prices
every package of one land assault our civilization can train that beats that
defender by 5 (class matchups included) and one breaker that opens that tier,
and researches the cheapest one within 25 Online turns. Our unique unit is
credited +5 strength and priced at 75% of its research; a civic-gated unique
unit (Samurai, Tagma, Winged Hussar, Mountie) becomes the civic goal, and
otherwise Nationalism does once it is inside the same horizon — Corps are +10
strength on every body, and the live seat forms them as soon as it can (6-19
FORM_CORPS/FORM_ARMY orders a game) but reached Nationalism at t150 against
the best rival's t106. An unlocked package is an **open window** and yields
the research slot. The goal
sits ahead of the air surge; at Urban Defenses the Bomber is itself a priced
breaker, so the two converge. Code: `src/ai/advanced/decisive_window.rs`.

Frame-0 replays of live G47 (civvis-20261004T040138Z), the deployed genome
with and without the gene, at turns where the seat chose research:

| Turn | Deployed genome researched | With `decisive-window` |
|---|---|---|
| 46 | Astrology | Masonry — Man-at-Arms beats Canada's best (36) by 9; Battering Ram opens tier-1 walls |
| 58 | Apprenticeship | Masonry (deployed got it at t72) |
| 72 | Masonry | Stirrups — Knight beats Canada (45) by 5; Catapult opens tier-2 walls in 6 turns |
| 90 | Castles (toward Rifling) | Metal Casting — Musketman + Bombard vs tier-3 walls in 5 turns |
| 100 | Ballistics | Printing toward Military Science — Line Infantry beats Canada (55) by 10; Bombard opens tier 3 |
| 115-137 | Education … Industrialization (air surge) | unchanged: no package inside 25 turns, the bomber beeline keeps the slot |

### Measurements so far

- **Fires** (`docs/gene_screens/fires/decisive-window.json`, 24 standard
  games, 144 seats, Domination target mix): `decisive-window` +4.6 pp wins
  (z +0.58), share +1.67 pp (z +1.04); `unique-unit-preference` +5.1 pp
  (z +0.71). A fires check, not a verdict.
- **Paired live-profile probe** (`victory_eval --domination-pair
  decisive-window`, 24 seeds 37160000-37160023: Gran Colombia focal, King,
  4 players, 60x38 Pangaea, Online, 250 turns, the live force-on genome): no
  measurable effect. Score -11 ± 28, foreign cities held +0.08 ± 0.16, first
  major war 2.9 turns earlier (z -1.58), t100 science -1.5 (z -1.58), techs
  at t150 identical; the focal seat won no game in either arm. The simulated
  focal seat trails its rivals far more than the live seat does, so this is a
  no-harm check; the live replays above are the evidence that the research
  order changes.

## 5. `unique-unit-preference` (opt-in gene)

`BasicAi::best_military` credits our own unique unit +5 strength while it is
under half of its role (melee or ranged) in the land army, counting each
city's current build. A Llanero (62) is then trained over the Line Infantry
(65) and Cuirassier (64) it unlocks beside — and the column still gets Line
Infantry beside it, which it needs against Pike and Shot (+10 vs cavalry).

The same gene points the research scorer's +55 credit for our own unique
unit at the rules' `unique_to`. That credit read `civs.json`'s `unique_unit`
field, which names one for only 14 of 105 civilizations — Gran Colombia's
Llanero was never credited.

## 6. Every civilization's unique unit

The rules carried 25 of the shipped game's unique units, so most
civilizations could never tech to theirs (Persia, Arabia, Zulu, Spain,
Korea, England, France, Hungary, Sweden, Norway…). The missing 32 were added
from the shipped database (`tools/civ6_fidelity.py`'s loader: base game, both
expansions and the content packs, Gathering Storm resource costs), with the
mirror's inbound aliases and the bridge's outbound names. Abilities beyond
the stat line are not modelled (as for most existing unique units); the
Immortal is flagged as melee *and* ranged so it can still take a city.
`tools/civ6_fidelity.py` now compares 132 units against the game with **0
divergent fields** (it compared 100 before). Because a civilization that owns
one now trains it instead of the unit it replaces, the frozen `advanced_v1`
anchor moved (v43 in `docs/ELO_REPINS.md`).

The research scorer's one-step lookahead had always counted *other*
civilizations' unique units as unlocks (the Pitati behind Archery for
everyone); with the full roster Military Tactics alone would have carried the
Impi, Berserker and Khevsureti, +24 on Mathematics for every civilization. It
now counts only units the seat's own civilization can field.

| Civilization | Unique unit | Replaces | Unlock | Path cost (std) | Strength (vs base) | Ranged/Bombard | Moves | |
|---|---|---|---|---|---|---|---|---|
| America | p51 mustang (air) | fighter | advanced flight | 12810 | 105 (+5) | 105 | 10 | added |
| America | rough rider | cuirassier | ballistics | 4035 | 67 (+3) |  | 5 |  |
| Arabia | mamluk | knight | stirrups | 1080 | 50 (+0) |  | 4 | added |
| Australia | digger | infantry | replaceable parts | 8275 | 78 (+3) |  | 2 | added |
| Aztec | eagle warrior | warrior | — | 0 | 28 (+8) |  | 2 |  |
| Babylon | sabum kibittum | — | — | 0 | 17 |  | 3 | added |
| Brazil | minas geraes (naval) | battleship | nationalism (civic) | 5290 culture | 70 (+10) | 80 | 5 | added |
| Byzantium | dromon (naval) | quadrireme | shipbuilding | 250 | 20 (+0) | 25 | 3 | added |
| Byzantium | tagma | knight | divine right (civic) | 1730 culture | 50 (+0) |  | 4 |  |
| Canada | mountie | — | conservation (civic) | 7070 culture | 62 |  | 5 | added |
| China | crouching tiger | — | machinery | 805 | 30 | 50 | 2 |  |
| Cree | okihtcitaw | scout | — | 0 | 20 (+10) |  | 3 | added |
| Egypt | maryannu chariot archer | — | wheel | 105 | 25 | 35 | 2 |  |
| England | redcoat | line infantry | military science | 3930 | 70 (+5) |  | 2 | added |
| England | sea dog (naval) | privateer | mercantilism (civic) | 3680 culture | 40 (+0) | 55 | 4 | added |
| Ethiopia | oromo cavalry | courser | castles | 890 | 48 (+2) |  | 5 |  |
| France | garde imperiale | line infantry | military science | 3930 | 70 (+5) |  | 2 | added |
| Gaul | gaesatae | warrior | — | 0 | 20 (+0) |  | 2 |  |
| Georgia | khevsureti | man at arms | military tactics | 695 | 48 (+3) |  | 2 | added |
| Germany | u boat (naval) | submarine | electricity | 7430 | 65 (+0) | 75 | 3 | added |
| Gran Colombia | llanero | cavalry | military science | 3930 | 62 (+0) |  | 5 |  |
| Greece | hoplite | spearman | bronze working | 105 | 28 (+3) |  | 2 |  |
| Hungary | black army | courser | castles | 890 | 49 (+3) |  | 5 | added |
| Hungary | huszar | cavalry | military science | 3930 | 65 (+3) |  | 5 | added |
| Inca | warakaq | skirmisher | machinery | 805 | 20 (+0) | 40 | 3 | added |
| India | varu | — | horseback riding | 195 | 40 |  | 2 |  |
| Indonesia | jong (naval) | frigate | mercenaries (civic) | 1445 culture | 45 (+0) | 55 | 5 | added |
| Japan | samurai | man at arms | feudalism (civic) | 935 culture | 48 (+3) |  | 2 |  |
| Khmer | domrey | trebuchet | military engineering | 890 | 40 (+5) | 50 | 2 | added |
| Kongo | kongo shield bearer | swordsman | iron working | 225 | 38 (+3) |  | 2 |  |
| Korea | hwacha | field cannon | gunpowder | 2375 | 45 (-5) | 60 | 2 | added |
| Macedon | hetairoi | horseman | horseback riding | 195 | 36 (+0) |  | 4 | added |
| Macedon | hypaspist | swordsman | iron working | 225 | 38 (+3) |  | 2 |  |
| Mali | mandekalu cavalry | knight | stirrups | 1080 | 55 (+5) |  | 4 |  |
| Maori | toa | swordsman | iron working | 225 | 38 (+3) |  | 2 |  |
| Mapuche | malon raider | — | gunpowder | 2375 | 55 |  | 4 | added |
| Maya | hulche | archer | archery | 75 | 15 (+0) | 28 | 2 | added |
| Mongolia | keshig | — | stirrups | 1080 | 35 | 45 | 4 |  |
| Netherlands | de zeven provincien (naval) | frigate | square rigging | 2275 | 50 (+5) | 60 | 4 | added |
| Norway | berserker | man at arms | military tactics | 695 | 48 (+3) |  | 2 | added |
| Norway | viking longship (naval) | galley | sailing | 50 | 35 (+5) |  | 3 | added |
| Nubia | pitati archer | archer | archery | 75 | 17 (+2) | 30 | 3 |  |
| Ottomans | barbary corsair (naval) | privateer | medieval faires (civic) | 1355 culture | 40 (+0) | 50 | 4 | added |
| Ottomans | janissary | musketman | gunpowder | 2375 | 60 (+5) |  | 2 | added |
| Persia | immortal | swordsman | iron working | 225 | 35 (+0) | 25 | 2 | added |
| Phoenicia | bireme (naval) | galley | sailing | 50 | 35 (+5) |  | 4 |  |
| Poland | winged hussar | cuirassier | mercantilism (civic) | 3680 culture | 64 (+0) |  | 4 |  |
| Portugal | nau (naval) | caravel | cartography | 1545 | 55 (+0) |  | 4 |  |
| Rome | legion | swordsman | iron working | 225 | 40 (+5) |  | 2 |  |
| Russia | cossack | cavalry | military science | 3930 | 67 (+5) |  | 5 |  |
| Scotland | highlander | ranger | rifling | 8535 | 50 (+5) | 65 | 3 | added |
| Scythia | saka horse archer | — | horseback riding | 195 | 20 | 25 | 4 |  |
| Spain | conquistador | musketman | gunpowder | 2375 | 58 (+3) |  | 2 | added |
| Sumeria | war cart | — | — | 0 | 30 |  | 3 |  |
| Sweden | carolean | pike and shot | metal casting | 3105 | 55 (+0) |  | 3 | added |
| Vietnam | voi chien | crossbowman | machinery | 805 | 35 (+5) | 40 | 3 |  |
| Zulu | impi | pikeman | military tactics | 695 | 45 (+0) |  | 2 | added |

## 7. Not done here

- **Launch on the window.** An open window whose target has its next wall
  tier one step away is a closing window; the declaration logic (owned by the
  one-war/diplomacy lane) does not yet read it.
- **Military civics.** Military Tradition t75 vs a rival's t34, Nationalism
  t150 vs 106: the civic chooser has no military-timing goal beyond the
  civic-gated unique units.
- **Unique-unit tactics.** The Llanero's adjacency bonus and the Hoplite's
  pairing reward keeping them together; formations do not yet prefer it.
- **Existing fidelity gaps** the game database shows in older entries (the
  Toa's unlock is Construction, not Iron Working; the Maryannu Chariot Archer
  replaces the Heavy Chariot) are left for the fidelity ratchet.
