# District defense range table

The foreign district health exporter read `GameInfo.Districts[kind].AttackRange`.
The installed Gathering Storm schema instead defines `Districts_XP2.AttackRange`,
keyed by `DistrictType`. With schema-shaped rows, the exporter discarded visible
Encampment defense observations before reading their health. This could leave the
mirror relying on its unknown-health approximation. No native win-rate effect is
established.

The source is `DLC/Expansion2/Data/Expansion2_Schema.sql:180-188`:
`CREATE TABLE "Districts_XP2"`, `"AttackRange" INTEGER NOT NULL DEFAULT 0`,
and `PRIMARY KEY(DistrictType)`.
`DLC/Expansion2/Data/Expansion2_Districts.xml:45-46` gives
`DISTRICT_CITY_CENTER` and `DISTRICT_ENCAMPMENT` `AttackRange="2"` in that table.
The installed Ikanda, Thanh and Oppidum compatibility rows also give range 2.
A read-only staged Gathering Storm load through `civ6_fidelity.load_database`
passed its expansion sentinel guard and found no `AttackRange` field on the base
rows for these four forts. An unrelated default compiled cache failed the guard
and was not used or rebuilt. This is installed-source evidence; no live game's
Lua `GameInfo` projection was read back.

The correction looks up the expansion row by the base row's district type.
When the expansion table exists, its zero or absent row cannot fall back to a
stale positive base value. Environments without that table retain a flat range
field. City centers remain excluded. The observation cache, visibility checks,
completion checks and independent garrison/outer health pools are unchanged.

The existing health suite retains all 19 test bodies byte for byte; only its
mock tables now follow the installed schema. A new suite checks Encampment,
Ikanda, Thanh and Oppidum export, differing numeric/type keys, expansion versus
base precedence, sparse rows, legacy flat tables and nondefending districts.
Oppidum export coverage does not establish a modeled Oppidum ranged action.

| Validation | Result |
| --- | --- |
| Unchanged exporter, schema-shaped health suite | 11 of 19 fail; 8 pass |
| Unchanged exporter, new range suite | 8 of 11 fail; 3 pass |
| Unchanged exporter, all other discovered Lua suites | 86 pass |
| Baseline parsing/source freeze | All 93 control-mod Lua files parse under 5.1; 636 inputs unchanged |
| Corrected exporter, discovered Lua suites | Pending |
| Final independent CI | Pending |

Artifacts are retained under
`civvis-tactics-results/2026-10-08/public-campus-finish-before-defender/`:
`defending-district-range-table-gs-install-static-investigation.json` and
`district-defense-range-unchanged-baseline-lua.{registration,receipt}.json`.
The complete baseline log SHA256 is
`7459aee1d108f6e0119009c6c9a3b8bcc2594a6fc2c7a8cfd360139bc937d741`.

This change corrects a reproduced export boundary. It changes no Rust engine,
AI policy, native lane, installed mod, runtime pin or game settings. No controlled
candidate games have been played and no domination win-rate gain is claimed.
