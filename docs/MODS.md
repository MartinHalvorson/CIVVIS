# Writing a mod

A Civ VIS mod is a folder of JSON. There is no code, no build step, and no
plugin API: you write the same files the engine ships in `data/`, and the
engine merges yours onto them at load.

```bash
civvis simulate --mods path/to/my-mod
civvis play --mods path/to/my-mod,path/to/another
civvis validate --mods path/to/my-mod     # check it without playing
```

Mods apply in the order given, so a later mod wins where two disagree. Every
command takes `--mods`; the ruleset is installed once, before the first game
exists, and a save records which mods it was played under.

## The folder

```
my-mod/
  mod.json          # optional: {"name": ..., "description": ...}
  units.json        # any subset of the files in data/
  modifiers.json    # generic Civ VI-style behavior rows
  difficulties.json
```

Overlay files must be named exactly like the ones in `data/` — `units.json`,
`techs.json`, `civs.json`, `agendas.json`, `speeds.json`, and so on. A file
with any other name is an error rather than a silent no-op, because a
misspelled `unit.json` that quietly does nothing is the worst possible
outcome.

## The three merge rules

Each ruleset file is an object keyed by entry id, and each key in your overlay
does one of three things.

**Add** — an id the base ruleset does not have is inserted whole:

```json
{
  "skirmisher": {
    "class": "military", "cost": 40, "moves": 3, "strength": 22,
    "promotion_class": "ranged", "tech": "archery"
  }
}
```

**Override** — an id it does have merges field by field, recursively, so you
only write what changes:

```json
{ "warrior": { "cost": 20, "moves": 3 } }
```

The Warrior keeps its Strength, its promotion class, its unlock and everything
else. This is the difference between a tweak and a rewrite, and it is why
overlays are merged rather than replaced.

**Remove** — an id mapped to `null` is deleted:

```json
{ "tlatoani": null }
```

The same rule applies one level down, so `{"Aztec": {"agenda": null}}` clears
one field rather than the whole civilization. That matters: removing an agenda
without also releasing the leader who held it leaves a dangling reference, and
the loader will tell you so.

## Validation is not optional

The merged ruleset runs through the same checker as `civvis validate` before it
is installed, and a mod with an error is refused:

```
$ civvis simulate --mods ./bad-mod
the modded ruleset does not validate:
error   units/warrior: tech names "phlogiston", which is not a known technology
```

That is deliberate. A dangling reference used to surface as a panic partway
into a game, or as a rule that silently never fired; now it surfaces as one
line naming the file and the entry. Warnings do not block anything — run
`civvis validate --mods ./my-mod` to read them.

## What you can change today

Everything the ruleset holds: terrain, features, resources, improvements,
units, districts, buildings, wonders, Great People, governors, projects, the
technology and civic trees and their effects, governments, policies,
promotions, beliefs, civilizations, leader agendas, difficulty levels, game
speeds, goody huts, eras, and generic modifiers.

`modifiers.json` is the first slice of the general modifier interpreter. A
root row names its owner, collection, effect, arguments, and optional owner or
subject requirement sets. A row without an owner is inert until another row
reaches it through `ATTACH_MODIFIER`:

```json
{
  "river_farm_food": {
    "owner": {"kind": "civilization", "id": "CIVILIZATION_SUMERIA"},
    "collection": "COLLECTION_PLAYER_PLOT_YIELDS",
    "effect": "MODIFIER_PLAYER_ADJUST_PLOT_YIELD",
    "arguments": {"YieldType": "YIELD_FOOD", "Amount": 1},
    "subject_requirements": {
      "mode": "REQUIREMENTSET_TEST_ALL",
      "requirements": [
        {
          "kind": "REQUIREMENT_PLOT_IMPROVEMENT_TYPE_MATCHES",
          "arguments": {"ImprovementType": "IMPROVEMENT_FARM"}
        },
        {"kind": "REQUIREMENT_PLOT_ADJACENT_TO_RIVER"}
      ]
    }
  }
}
```

Argument values may be strings or numbers. Identifiers may use Civ VI names
(`YIELD_FOOD`, `IMPROVEMENT_FARM`) or native CIVVIS ids (`food`, `farm`). Sets
accept `all`/`any` or `REQUIREMENTSET_TEST_ALL`/`_ANY`; each predicate may set
`"inverse": true`. The current subject handlers are arbitrary plot, building,
city, and district yield changes, plus city/district yield percentages. Owners
cover civilizations, leaders, gameplay traits,
policies, governments, technologies, civics, beliefs, improvements,
buildings, wonders, districts, governors, and governor promotions. Unknown
effects, predicates, owners, dangling children, and attachment cycles fail
validation by modifier id instead of silently doing nothing.

Other effect and collection families still need runtime handlers. Existing
numeric `effects` maps continue to work, so a mod may rearrange those named
primitives while the interpreter expands. See [MODIFIERS.md](MODIFIERS.md).

Policy overlays may also declare `"governments": ["communism"]` for
government-exclusive cards and `"obsoletes": [...]` when one unlocked card
replaces several predecessors. Changing governments automatically removes
exclusive cards that no longer qualify.

## Example

`mods/swift-legions/` is a complete two-file scalar overlay.
`mods/bbg-7.4.6-supported/` is the pinned, deliberately partial example for
generic modifiers. The tests in `src/mods.rs` specify the loader and
validation behavior.
