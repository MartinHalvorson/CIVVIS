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
python tools/civ6_modifiers.py --max-unmodelled N    # CI ratchet
```

It shares the rules audit's install detection and load order, and applies the
same baseline exclusions, so the two tools describe the same ruleset.

## What the census says

3,405 modifier rows across **698 distinct effects**, in the Gathering Storm
baseline with optional game modes excluded.

| Status | Effects | Rows |
|---|---:|---:|
| implemented | 25 | 825 |
| partial | 3 | 340 |
| unmodelled | 669 | 2,085 |
| out-of-scope | 1 | 155 |

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
| 50% | 32 |
| 80% | 181 |
| 95% | 528 |
| 100% | 698 |

Thirty-two effects get you half the rows. The remaining half needs another
666, most of which appear two or three times each. That shape is the argument
for phase 2 stated numerically: hardcoding is efficient right up until it
isn't, and the crossover is around the 50% mark, which CIVVIS is already
approaching. Past it, each additional effect buys roughly three rows, and
there is no batch large enough to be worth a bespoke implementation.

The single largest entry says the same thing from the other direction:
`ATTACH_MODIFIER` (336 rows) is the primitive that lets one modifier attach
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
2. **Close the three `partial` entries.** `ADJUST_PLOT_YIELD`,
   `ADJUST_BUILDING_YIELD_CHANGE` and `GRANT_ABILITY` are 340 rows between
   them, and each is partial for the same reason: a fixed set of named sources
   executes where the game takes an arbitrary one. They are the cheapest
   rehearsal for a general effect table.
3. **Then the interpreter**, in the shape phase 2 of FIDELITY.md describes:
   collections, effects, requirement sets, and a loader that reads the shipped
   `Modifiers` rows rather than transcribing them.

Content scope — the civilizations, units and buildings CIVVIS does not model
at all — is measured separately by the "Only in Civ VI" columns of
`tools/civ6_fidelity.py`. The two backlogs are independent: implementing an
effect makes the content that uses it expressible, and adding content makes
the effects it needs load-bearing.
