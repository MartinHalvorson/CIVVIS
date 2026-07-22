# What Civ VIS takes from Unciv

[Unciv](https://github.com/yairm210/Unciv) is the mature open-source Civilization V
reimplementation (Kotlin/libGDX, ~4k files). Civ VIS aims at the same slot for
Civilization VI, but from the other end: headless and AI-first, with the GUI as a
client of the engine rather than the other way round.

That difference means most of Unciv's code is not portable — its UI stack, its
translation pipeline, its Android packaging — but its *architecture* is the product of
eight years of running a moddable 4X, and several of those decisions are ones we would
otherwise have to learn the hard way. This document records what we studied, what we
are adopting, and what we are deliberately not.

Read alongside [FIDELITY.md](FIDELITY.md) (which decides *what the numbers should be*)
and [AI_GAPS.md](AI_GAPS.md) (which decides *how strong the agents are*). This document
decides *what shape the engine is*.

## The one big idea: effects are data, not code

Unciv's defining decision is that every game effect is a **unique** — a parameterized
sentence stored on a ruleset object:

```json
"uniques": ["[+2 Production] [in this city] <when at war>", "[+25]% Production when constructing [Melee] units"]
```

Each unique is parsed once into a placeholder template plus parameters, matched against
a `UniqueType` enum, and evaluated against a `GameContext` (civ / city / tile / unit).
Conditionals (`<when at war>`, `<in cities with a [Garrison]>`) are themselves uniques
attached as modifiers, and numeric parameters may be **countables** — an expression
language over game state (`[Cities]`, `[Science] Per Turn`, `[turns]`, arithmetic).

The payoff is that a mod author adds a genuinely new effect without touching Kotlin,
and the engine has exactly one place where effects are resolved. The cost is real —
1,200 lines of `UniqueType`, 800 of parameter typing, 1,500 of trigger activation, a
deprecation table with an auto-updater, and a permanent regex-parsing tax that Unciv
has had to cache around (`LocalUniqueCache`, the "only parse once per unique" comment).

**Our position.** Civ VIS already has effect data (`tree_effects.json`, effect fields on
`BuildingSpec`/`WonderSpec`), but each effect kind is a hand-written field consumed by
hand-written code, which is why `game.rs` is 37k lines. FIDELITY.md phase 2 already
calls for a modifier interpreter that ingests Civ VI's own `Modifiers` table; Unciv is
the proof that the design carries a full 4X. We adopt the *shape* — one typed effect
value, one evaluation context, one resolution site — but as a serde-tagged Rust enum
rather than free text, so that effects are checked at compile time and cost no regex at
runtime. The text surface belongs in the mod loader, not in the hot path.

## Adoption ledger

| # | Unciv idea | Verdict | Where it lands here |
|---|---|---|---|
| 1 | Uniques: effects as parameterized data with conditionals | **Adopt (typed)** | FIDELITY.md phase 2 modifier interpreter |
| 2 | Countables — expression language for numeric parameters | Adopt subset | with (1) |
| 3 | Ruleset validation with severities + author-controlled suppression | **Adopted** | `civvis validate` |
| 4 | Difficulty levels as data, with AI/human handicaps | **Adopted** | `data/difficulties.json` |
| 5 | Game speeds as data | **Adopted** | `data/speeds.json` |
| 6 | Leader personalities driving AI weighting | Adopt next | Civ VI ships `Agendas.xml` — real content for it |
| 7 | Notifications: categorized, located, per-player event stream | **Adopted** | `events` in `obs`, GUI log |
| 8 | Victories as data with ordered milestones | Adopt next | `data/victories.json` |
| 9 | Mod folders overlaid on the base ruleset at load | Adopt next | `--mods` overlay for `data/*.json` |
| 10 | Dev console for state inspection/mutation | Adopt next | GUI console + `civvis console` |
| 11 | Civilopedia generated from the ruleset | Adopt next | `/pedia` endpoint |
| 12 | Unit automation + autoplay for the human seat | Adopt next | reuse `AdvancedAi` per-unit |
| 13 | Deprecation table + auto-updater for mod data | Note | when the data format stabilizes |
| 14 | Translation pipeline, libGDX UI, Android packaging | **Skip** | wrong shape for a headless engine |
| 15 | Multiplayer via dumb file server | Skip for now | our clients are agents, not phones |

### 3. Validation — adopted

Unciv's `RulesetValidator` (851 lines) checks a ruleset the way a compiler checks a
program: every cross-reference resolves, every unique is known, every parameter has the
right type. Findings carry a severity (`Error`, `Warning`, `OK`), and mod authors can
*suppress* a specific finding with a unique on the mod itself — an escape hatch that
keeps the checker strict without making it a nuisance.

We had none of this: a bad `data/*.json` edit surfaced as a serde panic at startup, or
worse, as a silently dead rule. `civvis validate` now cross-checks the shipped ruleset
and any mod overlay, reports errors and warnings separately, and is asserted clean by a
unit test — so an unresolvable reference fails CI rather than a playthrough.

We take the severity split and the suppression escape hatch. We do not take Unciv's
text-similarity "did you mean" pass; our identifiers are machine-generated, not typed by
hand.

### 4 & 5. Difficulty and speed — adopted

Unciv keeps `Difficulties.json` and `Speeds.json` as ordinary ruleset objects: a
difficulty is a bag of multipliers (research/unit/building cost, AI equivalents,
barbarian bonus and spawn delay, free AI techs, bonus starting units), and a speed is a
bag of cost modifiers plus a turn-length table. Nothing about difficulty is compiled in.

Civ VIS had neither. That was a real hole in two directions at once:

- **For the player.** Every game was one fixed difficulty, so there was no way for
  Martin to pick a level.
- **For the AI track.** [AI_GAPS.md](AI_GAPS.md) gap 9 is eval calibration, and Elo
  between our own bots is a closed system — it says a bot is 130 points better than
  another bot, and nothing about whether either is any good. A difficulty ladder is an
  *external* yardstick: "beats Emperor, loses to Immortal" is a claim a Civ player can
  read.

We take Unciv's data shape but not Unciv's numbers, because we have a better source —
Civ VI ships its own handicap scaling in `Leaders.xml`
(`HIGH_DIFFICULTY_SCIENCE_SCALING` and friends, `LinearScaleFromDefaultHandicap` from
Prince). See `data/difficulties.json` for the resulting table and
[MECHANICS.md](MECHANICS.md) for coverage.

### 6. Personalities — the next batch

Unciv gives every leader a personality vector (culture/faith/gold/military/aggressive/
declareWar/loyal/expansion…) plus branch priorities, and `NextTurnAutomation` weights its
decisions by it. The effect is that AI civs feel like *someone* rather than like the same
bot in different colours.

Civ VIS has the machinery for this already — `BasicAi` carries 29 GA-tuned weights and
`AdvancedAi` picks grand strategies — but every major civ runs identical weights, so
Trajan and Cleopatra play the same game. Civ VI also ships the content Unciv had to
invent: `Agendas.xml` has each leader's historical agenda, and the AI's own
`BehaviorTrees.xml` and bias tables sit next to it. Per-leader personalities sourced
from that XML are the natural next batch, and they pay into AI_GAPS gap 7 (diplomacy
depth) at the same time.

### 7. Notifications — adopted

Unciv's notifications are a per-civ list of categorized entries (General, Trade,
Diplomacy, Production, Units, War, Religion, Espionage, Cities), each carrying icons and
*actions* — clicking one moves the camera to the tile, opens the city, or selects the
unit.

We adopted the model rather than the UI. The engine now records a per-player event
stream, exposed through `obs` alongside everything else an agent sees, because "what
just happened to me" is exactly the signal a learning agent needs and it was previously
reconstructable only by diffing observations. The GUI renders the same stream as a log,
which also fixes a spectate complaint: it was possible to watch a turn resolve and have
no idea *why* a city changed hands.

### 8. Victories as data — the next batch

`VictoryTypes.json` in Unciv lists ordered **milestones** (`"Build [Apollo Program]"`,
`"Add all [spaceship parts] in capital"`), each matched to a `MilestoneType` enum. A
modder can define a new victory condition, and the victory screen shows progress through
it for free.

Our six victory types are hardcoded, which is fine for fidelity and bad for mods, and it
means the GUI cannot show "you are 2 of 3 milestones into Science". `data/victories.json`
with ordered milestones is the fix; the enum of milestone kinds stays typed in Rust.

### 14 & 15. What we are not taking

Unciv's translation system (crowdsourced `template.properties` with placeholder
matching), its libGDX screen stack, its skin/tileset system and its Android packaging
are all excellent and all answer a question we do not have. Our client is a browser page
served by the engine, and our primary consumer is an agent reading `obs`.

Its multiplayer is a genuinely elegant minimum — a dumb file server that stores game
state blobs, with all logic client-side — and is worth revisiting the day Civ VIS wants
human-vs-human play. Today our "multiplayer" is a tournament harness.

One last thing worth naming: Unciv annotates functions with `@Readonly`/`@Pure` from a
compiler plugin to keep game-state reads from mutating anything, because Kotlin will not
enforce it. We get that from `&self` for free. It is a good reminder that some of what
looks like missing infrastructure here is infrastructure Rust already provides.
