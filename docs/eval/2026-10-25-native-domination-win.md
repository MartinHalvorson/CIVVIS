# Verified Native Emperor Domination benchmark record

The shared ladder's known retrospective run **civvis-20261025T145013Z**
records a host-awarded **VICTORY_CONQUEST** win at **turn 210**,
2026-10-25 15:05:46.888 UTC. Its raw Native TeamVictory event is unique and
identical to the terminal summary. The winner and local team are both 0;
the local seat is 0. The run's actual victory table maps index 2 to Conquest.
This is the Native game's result, rather than a simulated win or an accepted
order. The primary shipped UI uses the same event/table pairing in
`Base/Assets/UI/Automation/Automation_NarrationManager.lua:107–111` and
`Base/Assets/UI/EndGame/EndGameMenu.lua:1039–1054`.

This is one deliberately selected, already-known winning record, not an
unbiased trial or a win-rate estimate. It is outside both the eight-game
material and sixteen-game Culture-air cohorts; their zero-win results remain
unchanged. It predates the default-off Slinger and spy-spotting candidates and
cannot establish their effect. No new Native game or whole-game replay is run.

| Actual recorded setup | Value |
|---|---|
| Difficulty | DIFFICULTY_EMPEROR |
| Ruleset | RULESET_EXPANSION_2 |
| Civilization / leader | CIVILIZATION_GRAN_COLOMBIA / LEADER_SIMON_BOLIVAR |
| Map / size | Pangaea.lua / MAPSIZE_TINY |
| Speed / players / city-states | GAMESPEED_ONLINE / 4 / 6 |
| Enabled victory conditions | Conquest, Culture, Diplomatic, Religious, Score, Technology |
| Requested strategy | Domination |

The settings, requested ruleset and actual seat readbacks agree. No victory
condition is disabled. The frozen adapter revision is
`9a645d8b92b4fd9e732cfa1ea15becf4882b13b3`, SHA-256
`c6f5c41eaf0a129593aafad3c5dc79b0f25ede1748c28cec53aa28dab4785fdc`. Its bytes match the
runtime receipt and every one of **609** recorded controller decisions uses
that same digest. All 609 capture descriptors are complete. The final actual
input is retained with **seventeen calls to one distinct read**; its last
state exactly matches the last Native event state.

The summary reports 5,768 unit orders and 5,293 applied unit orders, with
CIVVIS unit share 1.0 and zero engine exploration orders. Those are recorded
controller/application metrics; they do not independently verify each order.
The source-revision Lua reference and primary UI search are retained, while
**historical loaded mod-byte identity remains unattested**.

## Limits of the last city roster

The last exported state is turn 210/frame 1, before the victory callback. Its
own roster marks Bogotá and Râ-Kedet as major original capitals, while the
rival roster still marks Tyre and Patna. This record therefore does not
independently reconstruct possession of every original capital from the final
roster, and no missing capture or after-victory state is invented. The verified
result is the host's explicit Conquest award. A follow-up strategy audit must
retain this distinction rather than turn the roster into a fabricated ending.

## Verification and retained data

Three control tests cover one positive and eight negative cases, including a
summary without a raw callback, duplicate callbacks, a different winner/type,
wrong difficulty, disabled victory and a nonterminal record. All pass.
The complete verification is byte-identical after extracting the raw packet to
a different directory and explicitly relocating the adapter/capture paths.
This reproduces the evidence checks, not another Native game's outcome.

The **26-member** archive is **8,755,712 bytes**, SHA-256
`b66838a8867ac47e5c05cd9d9a91ab906dbe9da936e13701e52433e508ebe0f0`. Every member is checked against its
source bytes and all original selected files remain unchanged. It contains
all fifteen non-image run files, the actual frozen adapter and provenance,
input bytes/index, verification source/results/controls and primary searches.
Diagnostic image identities are listed in the manifest without duplicating
those images. The orders database and its WAL/SHM are retained together.

Restore the archive, run the companion tests, then run the verifier with
`--run <packet>/run/civvis-20261025T145013Z`,
`--binary <packet>/adapter/civvis_orders`,
`--capture-directory <packet>/run/civvis-20261025T145013Z/input-captures/process-sdvljul5/reads`
and a new `--out`. The default `--tools` points to the repository helpers.
All raw decision descriptors remain unchanged; relocation uses an in-memory
copy. No gameplay, deployment ledger, runtime setting or Native driver is
modified by this evidence task. Final Git/CI integration remains pending.
