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
| Corrected exporter, discovered Lua suites | All 88 pass, including all 11 range cases and all 19 original health assertions; 636 inputs unchanged |
| All scripts in both CI mod directories | All 95 parse under Lua 5.1 |
| Independent source-head CI | Control-mod, native-type/sandbox/local-slot guards and cargo integrity checks pass; Rust gate explicitly reports `rust_gate=false` |
| Rust/data/build inputs | All 548 byte-identical to the task base; no redundant compilation required by the tools/docs fast lane |
| Local normal overwrite guard | Pass; five recent deleted lines below its threshold; no exception |

The first hosted overwrite job timed out during repository checkout after
15 minutes; its blame step never ran. That failure is retained in
`district-defense-range-source-head-overwrite-checkout-timeout.{log,receipt.json}`.
The final head must still pass the ordinary hosted guard. The paired-cost job
passed its scope decision and skipped both builds and measurement for unchanged
Rust; it supplies no measured cost or speed result.

Artifacts are retained under
`civvis-tactics-results/2026-10-08/public-campus-finish-before-defender/`:
`defending-district-range-table-gs-install-static-investigation.json` and
`district-defense-range-unchanged-baseline-lua.{registration,receipt}.json`.
The complete baseline log SHA256 is
`7459aee1d108f6e0119009c6c9a3b8bcc2594a6fc2c7a8cfd360139bc937d741`.
The complete passing candidate log SHA256 is
`91da0b3ab50502cd23d45416659e97258637f083b2b43494d6a7d731bc00acb6`.
`district-defense-range-candidate-validation-proof.json` records the original
19 test-body identity and the unchanged Rust/data/build inputs. Evaluation-only
changes reuse the passing candidate's byte-identical validated inputs.

This change corrects a reproduced export boundary. It changes no Rust engine,
AI policy, native lane, installed mod, runtime pin or game settings. No controlled
candidate games have been played and no domination win-rate gain is claimed.
