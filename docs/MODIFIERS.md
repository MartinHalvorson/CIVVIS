# The modifier backlog, measured

[FIDELITY.md](FIDELITY.md) phase 1 established that CIVVIS' rules *numbers*
match the shipped game database — 22 tables at zero divergence. That covers
what things cost and what tiles yield. It says nothing about what things *do*.

Civ VI keeps almost all of that in one place. A leader ability, a belief, a
policy card, a governor promotion and a wonder's effect are the same
construction: a row in `Modifiers` naming a `ModifierType`, which
`DynamicModifiers` resolves into an `EffectType` (what happens) and a
`CollectionType` (who it happens to), plus `ModifierArguments` and an optional
`RequirementSet`. CIVVIS historically hardcoded each effect in Rust. The first
generic runtime slice now reads the same shape from `modifiers.json` and
executes attachment graphs for plot, building, city, and district yield
subjects.

`tools/civ6_modifiers.py` measures what that costs:

```sh
python tools/civ6_modifiers.py                       # ranked report
python tools/civ6_modifiers.py --effect ADJUST_PLOT_YIELD   # every row using one effect
python tools/civ6_modifiers.py --max-unmodelled N    # CI ratchet
```

It shares the rules audit's install detection and load order, and applies the
same baseline exclusions, so the two tools describe the same ruleset.

## What the census says

The last full local-database census before that runtime landed found 3,405
modifier rows across **698 distinct effects**, in the Gathering Storm baseline
with optional game modes excluded.

| Status | Effects | Rows |
|---|---:|---:|
| implemented | 25 | 825 |
| partial | 3 | 340 |
| unmodelled | 669 | 2,085 |
| out-of-scope | 1 | 155 |

Those table counts are a historical baseline and should not be read as the
post-interpreter report on a machine without a Civ VI install. The live
`tools/modifier_coverage.json` now marks `ATTACH_MODIFIER` partial alongside
the two generic yield effects; rerunning the census against the game database
produces the current row totals.

`tools/modifier_coverage.json` holds those judgements with a reason each.
They are seeded by reading the engine for each effect family and are mostly
**not** yet verified row by row (568 rows are, so far) — an `implemented` entry is a claim to be checked, and
checking them is the next step. Anything absent from the file counts as
unmodelled, so newly shipped content raises the backlog rather than hiding.

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
interpreter's own composition operator. CIVVIS now validates and executes
that graph for the implemented plot/building/city/district yield subjects, including cycle
and dangling-child rejection; other attached effect and collection families
keep its coverage status partial.

## Order of work

1. **Verify the 28 implemented and partial effects row by row.** The census is
   only as honest as `modifier_coverage.json`, and most entries are still
   inspection judgements. Drill in with `--effect`, check each row's arguments
   and requirement set against the CIVVIS path that claims to cover it, mark
   the entry `verified`, and demote whatever does not hold. The report prints
   verified rows against covered rows, so the ratchet is visible.

   Ten are done so far (568 of 1,165 covered rows), and seven of them found
   real divergences:

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
2. **Expand the interpreter slice.** `ADJUST_PLOT_YIELD` and
   `ADJUST_BUILDING_YIELD_CHANGE` now accept arbitrary owners, yield/building
   arguments, all/any/inverse requirements, and `ATTACH_MODIFIER` composition.
   City/district flat and percentage yield effects now use the same runtime.
   The checked-in `mods/bbg-7.4.6-supported` overlay exercises the path with
   15 current BBG rows. Close their remaining shipped collection and
   requirement variants, then complete `GRANT_ABILITY` and
   `CITY_GRANT_RANDOM_RESOURCE_PRODUCT`.
3. **Import rather than transcribe.** Extend the runtime subject/effect table
   in frequency × impact order and have the data tool emit `modifiers.json`
   from the shipped database and pinned BBG SQL. Validation already rejects
   unknown effects, requirements, owners, dangling attachments, and cycles.

Content scope — the civilizations, units and buildings CIVVIS does not model
at all — is measured separately by the "Only in Civ VI" columns of
`tools/civ6_fidelity.py`. The two backlogs are independent: implementing an
effect makes the content that uses it expressible, and adding content makes
the effects it needs load-bearing.
