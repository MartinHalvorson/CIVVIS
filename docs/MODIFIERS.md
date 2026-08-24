# The modifier backlog, measured

[FIDELITY.md](FIDELITY.md) phase 1 established that CIVVIS' rules *numbers*
match the shipped game database — 22 tables at zero divergence. That covers
what things cost and what tiles yield. It says nothing about what things *do*.

Civ VI keeps almost all of that in one place. A leader ability, a belief, a
policy card, a governor promotion and a wonder's effect are the same
construction: a row in `Modifiers` naming a `ModifierType`, which
`DynamicModifiers` resolves into an `EffectType` (what happens) and a
`CollectionType` (who it happens to), plus `ModifierArguments` and an optional
`RequirementSet`. CIVVIS instead hardcodes each effect in Rust.

`tools/civ6_modifiers.py` measures what that costs:

```sh
python tools/civ6_modifiers.py                       # ranked report
python tools/civ6_modifiers.py --effect ADJUST_PLOT_YIELD   # every row using one effect
python tools/civ6_modifiers.py --max-unmodelled N    # ratchet
python tools/civ6_modifiers.py --emit-catalog        # import into data/modifiers.json
python tools/civ6_modifiers.py --check-catalog       # the committed catalog still matches
```

It shares the rules audit's install detection and load order, and applies the
same baseline exclusions, so the two tools describe the same ruleset — and the
importer below is in the same file for the same reason.

## What the census says

2,908 modifier rows across **639 distinct effects**, in the Gathering Storm
baseline with optional game modes excluded, read from the installed load order.

| Status | Effects | Rows |
|---|---:|---:|
| implemented | 76 | 1,599 |
| partial | 2 | 21 |
| unmodelled | 560 | 1,139 |
| out-of-scope | 1 | 149 |

The compiled `Cache/DebugGameplay.sqlite` is a smaller ruleset than the install
walk — 2,626 rows across 581 effects, of which 1,021 are unmodelled and 21
partial — because it is whatever the game last compiled for itself rather than
a chosen content set in a chosen order. Both are real readings of the same
tool; a run states which one it took. The ratchet therefore has two values:

```sh
python tools/civ6_modifiers.py --max-unmodelled 1160            # installed load order
python tools/civ6_modifiers.py --cache --max-unmodelled 1042    # compiled database
```

Neither runs on a CI runner, which has no install and no cache — the reason
`tools/test_ci_wiring.py` lists this tool as one the fleet checks by hand. What
*does* run in CI is `tools/test_civ6_modifiers.py`, whose hermetic fixtures pin
the importer's translation and, more importantly, its refusals.

`tools/modifier_coverage.json` holds those judgements with a reason each.
They are seeded by reading the engine for each effect family. **Every covered
row is now verified row by row** — all 1,250 of them, against the shipped
`Modifiers` tables read from the compiled gameplay database. Each entry's note
records what was checked and what it found. Anything absent from the file
counts as unmodelled, so newly shipped content raises the backlog rather than
hiding.

## The finding that matters

The work is not concentrated:

| Share of rows | Effects needed |
|---|---:|
| 50% | 29 |
| 80% | 178 |
| 95% | 494 |
| 100% | 639 |

Twenty-nine effects get you half the rows. The remaining half needs another
610, most of which appear two or three times each. That shape is the argument
for phase 2 stated numerically: hardcoding is efficient right up until it
isn't, and the crossover is around the 50% mark, which CIVVIS is already
approaching. Past it, each additional effect buys roughly three rows, and
there is no batch large enough to be worth a bespoke implementation.

The single largest entry says the same thing from the other direction:
`ATTACH_MODIFIER` is the primitive that lets one modifier attach
another to a collection. It is not a game rule at all — it is the
interpreter's own composition operator, and nothing built out of it can be
expressed without building the interpreter.

## Order of work

1. ~~**Verify the 28 implemented and partial effects row by row.**~~ **DONE**:
   all 33 entries, 1,250 of 1,250 covered rows. Drill in with `--effect`, check
   each row's arguments *and its requirement set* against the CIVVIS path that
   claims to cover it, mark the entry `verified`, and demote whatever does not
   hold. The report prints verified rows against covered rows.

   **The lesson of the pass: the amounts were nearly all right and the
   conditions were not.** `ADJUST_PLOT_YIELD` is the clearest case — 123 rows,
   two defects, and a value-only comparison would have reported zero, because
   both had the correct amount attached to the wrong tiles. Read `Inverse` on
   every requirement (76 of 809 shipped requirements are inverted), read
   `RequirementSetType` (84 of 867 sets are `TEST_ANY`), and follow the grant
   chain: a modifier row says an effect exists, only its owner says whether the
   shipped game ever fires it.

   The divergences it found:

   - a city's Commercial Hub or Harbor granted no Trade Route at all, and
     Merchant Republic none of its two;
   - Theocracy could Faith-buy a Giant Death Robot;
   - the Statue of Liberty granted no Settlers;
   - Laissez-Faire and Nobel Prize each paid one flat number per building tier
     instead of the shipped 2/4 split, and Military Organization was missing
     its flat +4 Great General;
   - Colonial Taxes applied its +25% Gold but not its +10% Production;
   - the Giant Death Robot's tech upgrades hung off the wrong nodes, with an
     invented healing upgrade on Cybernetics and the Particle Beam Siege
     Cannon missing entirely;
   - every unit-Production policy card ignored its era window, so Agoge
     boosted a Modern Infantry as readily as a Warrior.

   Seven errors in ten effects is the argument for running the pass to the
   end:
   the seeded statuses were inspection judgements, and inspection is not
   finding these.

   **Sweep a whole class against its own wording.** `--sweep improvements`
   and its siblings print every entry of a CIVVIS data file beside the
   description the game shows. Effect rows say what a number is; descriptions
   say what clauses exist, and a missing clause is invisible to a row-by-row
   diff because the row simply is not there. Sweeps so far: improvements found
   the Lumber Mill's Mercantilism gate, the Solar Farm's Snow exclusion and a
   hardcoded Appeal list that did not match the shipped table; policies found
   four wrong cards; buildings and districts came back clean.

   Two cautions the sweeps taught. A description can be stale where a table is
   not -- the Ski Resort reads "+4 Tourism" but ships TOURISMSOURCE_APPEAL, so
   **tables win on magnitude and mechanism, descriptions on the existence of a
   clause**. And a card whose effects look nothing like its description is
   usually half-implemented rather than wrong: Finest Hour and Public
   Transport each had one of their two modifiers, and the description named
   the other.

   **When two rows disagree, ask the shipped text.** Gathering Storm restates
   beliefs, promotions and city-state bonuses without deleting the base rows,
   so the tables alone often admit two readings. `--describe <tag fragment>`
   prints the localised descriptions with the Gathering Storm wording first: a
   `_EXPANSION2_DESCRIPTION` tag is what the player is actually shown, and the
   plain tag is superseded. That settled the founder beliefs after the tables
   could not, and it is faster than the two structural tells below.

   **Read the condition, not just the amount.** The database ships more than
   one ruleset. City-state Envoy bonuses exist twice over: base-game rows that
   pay the Capital at 1 Envoy and the tier-1 building at 3, and Ethiopia-pack
   rows -- the final-patch structure a Gathering Storm game actually runs --
   that pay Capital *and* tier-1 at 1. They are distinguished only by an
   `_ETHIOPIA` suffix on the `ATTACH_MODIFIER` that binds them. Reading the
   base amounts alone, I "corrected" a correct implementation and had to
   revert it. `--effect` now prints each row's resolved requirement set for
   exactly this reason, and the loader honours `Delete` on attachment tables
   so a detached modifier stops reading as live.

   `ADJUST_CITY_FREE_POWER` is the clean case and shows the shape: fifteen
   rows, of which twelve execute with the shipped amounts (Geothermal Plant 4,
   Hydroelectric Dam 6, three renewables at 2, Reyna's Renewable Subsidizer
   adding 2 to each, Aerospace Contractors' Spaceport 3) and three belong to
   Cardiff, a city-state CIVVIS does not model — content scope, not effect
   scope. Distinguishing those two failure modes is the point of the exercise.
2. **Close the three `partial` entries.** They are now
   `ADJUST_BUILDING_YIELD_CHANGE`, `GRANT_ABILITY` and
   `CITY_GRANT_RANDOM_RESOURCE_PRODUCT` — `ADJUST_PLOT_YIELD` was promoted to
   `implemented` once its two condition defects were fixed and all 123 rows
   checked.

   The first two are partial for the same structural reason: a fixed set of
   named sources executes where the game takes an arbitrary one. **Everything
   they do model is verified correct**, so closing them is not bug-fixing but
   generalisation — which makes them the cheapest rehearsal for the effect
   table in step 3, not independent work.

   `CITY_GRANT_RANDOM_RESOURCE_PRODUCT` is partial for a different reason and
   should not be lumped in: the shipped `ResourceCorporations` effects are
   **+40% modifiers** where CIVVIS pays flat yields. It is Monopolies &
   Corporations content, which the CPL lobby disables with every game mode, so
   it changes no tournament game — and a partial fix there (Wine is grouped
   with Salt where the shipped table puts it with Silk) would leave it
   differently wrong. Convert the whole flat-to-percentage model or leave it.
3. **Then the interpreter**, in the shape phase 2 of FIDELITY.md describes:
   collections, effects, requirement sets, and a loader that reads the shipped
   `Modifiers` rows rather than transcribing them.

The first interpreter slice is now in the engine. A named bundle may declare a
`collection` of `player`, `player_cities`, or `player_units`, plus a borrowed
requirement set with `all`, `any`, and `none` groups. The live collector evaluates
those predicates against the player's current government, civilization,
religion, pantheon, Secret Society, age, policies, technologies, and civics;
changing one of those facts changes the effect without reattaching the bundle.
Static rules-object attachments reject contextual bundles instead of applying
them unconditionally, and nested bundles must be unconditional and stay in the
parent collection. This keeps the existing flattening fast while making the
new conditional path safe to extend.

## The import (shipped)

`data/modifiers.json` is no longer empty, and it is no longer written by hand.
`tools/civ6_modifiers.py --emit-catalog` reads `Modifiers`, `DynamicModifiers`,
`ModifierArguments`, `RequirementSets`, `Requirements` and
`RequirementArguments` through the same loader and the same baseline exclusions
the census uses, translates every row of a **declared** effect into a named
`ModifierSpec` bundle, and prints the wiring that follows: for each bundle, the
CIVVIS ruleset objects the shipped owner tables say grant it. Those objects
carry a `modifiers: ["<bundle>"]` reference, and `Rules::from_values` folds the
bundle into their ordinary effect map, so an imported row executes through the
consumer a hand-written number used to — with the difference that the number is
the game's own. `--check-catalog` re-derives both halves and fails on any drift,
in either direction: a bundle nothing attaches, and an attachment no shipped row
grants, are both errors.

Three refusals keep the import from inventing rules, and they are the part worth
reading:

1. **An effect is imported only when the tool declares a translation for it.**
   Everything else stays in the census as unmodelled. A row emitted under a key
   no consumer reads would be inert data counted as fidelity.
2. **A row carrying a requirement set is refused.** `ModifierRequirement` covers
   player facts only. The Diplomatic Quarter's Envoy is conditional on plot
   adjacency and Phoenicia's Settler sight on being embarked; emitting either
   unconditionally would be silently wrong in every game that touched it.
3. **Only `COLLECTION_OWNER` and `COLLECTION_PLAYER` are folded.** Those are the
   rows whose scope is the owning object's own. A `PLAYER_CITIES` or
   `PLAYER_UNITS` row means something the static fold cannot say, and the engine
   rejects it rather than guessing.

The first slice is ten effects and 46 rows, chosen by frequency among the rows
whose owners CIVVIS models: `GRANT_INFLUENCE_TOKEN`,
`ADJUST_PLAYER_DIPLOMATIC_VICTORY_POINTS`,
`ADJUST_PLAYER_EMBARKED_UNIT_MOVEMENT`, `ADJUST_PLAYER_TOURISM`,
`ADJUST_PLAYER_SPY_BONUS`, `ADJUST_PLAYER_DISTRICT_AIR_SLOTS`,
`GRANT_AIR_SLOTS`, `ADJUST_UNIT_ATTACK_RANGE`, `ADJUST_UNIT_NUM_ATTACKS` and
`ADJUST_UNIT_ATTACK_AND_MOVE`, plus `ADJUST_UNIT_SIGHT` demoted honestly to
`partial` for the one row a unit-state predicate would be needed to express.
`src/game/modifier_tests.rs` proves each one lands in a running game rather than
only in the ruleset.

**Most of the fold restores what CIVVIS already carried; four rows did not.**
Eleven of the thirteen civics `GRANT_INFLUENCE_TOKEN` pays now award their
Envoys — CIVVIS handed out four of twenty-five — Jakob Fugger awards his two,
Sweeping Wind gains the `MOD_MOVE_AFTER_ATTACKING` it shares with Elite Guard
and Breakthrough, and Computers multiplies Tourism by the +25% its row states
instead of +100%. That is the argument for the import in one paragraph: the
amounts CIVVIS transcribed were mostly right, and the rows it never noticed were
invisible to any amount-checking pass.

### What the next slice should be

The remaining backlog is genuinely long-tailed and its head is content CIVVIS
does not model — Great Person individuals, Rock Band promotions and unique
civilization traits. Two things are worth more than another lap of the ranking:

- **A unit predicate in `ModifierRequirement`** — unit state (embarked,
  wounded) and unit tag (`UNIT_TAG_MATCHES`). It is what refusal 2 costs today:
  it holds back `ADJUST_UNIT_SIGHT`'s last row and all five `GRANT_PROMOTION`
  rows, every one of which is gated on `UNIT_TAG_MATCHES(CLASS_GIANT_DEATH_ROBOT)`.
- **The Rock Band promotion family.**
  `ADJUST_UNIT_ROCK_BAND_LEVEL_DISTRICT` (15 rows) and
  `ADJUST_UNIT_TOURISM_BOMB_DISTRICT` (7) are the largest unmodelled effects
  whose owners CIVVIS already has: every one of the 22 rows hangs off a
  promotion that is already in `data/promotions.json`, so they need no new
  content — only a district-keyed selector of the kind `building_yields`
  already is.

Checked and rejected as a next slice:
`ADJUST_PLAYER_TRADE_ROUTE_YIELD_MODIFIER` looks ideal at 12 rows on
`PolicyModifiers` alone, and **none** of its owning cards are in the CIVVIS deck
(all twelve are Letters of Marque and its siblings). Owner tables say what
grants a row, not whether this engine has the grantor; ask the ruleset before
ranking.

Content scope — the civilizations, units and buildings CIVVIS does not model
at all — is measured separately by the "Only in Civ VI" columns of
`tools/civ6_fidelity.py`. The two backlogs are independent: implementing an
effect makes the content that uses it expressible, and adding content makes
the effects it needs load-bearing.
