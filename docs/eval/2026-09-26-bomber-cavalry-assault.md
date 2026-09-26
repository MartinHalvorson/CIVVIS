# Cavalry spotting and bomber city assaults

The operator requested cavalry to reveal cities for Bombers, then use its remaining movement to capture or retreat after the sorties. Existing tactics finished one unit at a time and scheduled siege/air units before cavalry. The native per-unit queue cannot guarantee settlement between different units.

This candidate extends the existing opt-in air surge. A bounded prepass tries up to four healthy cavalry and four nearby Bombers against the active city objective. A spotting move must leave a legal return to a position whose modeled incoming damage is below half the cavalry health. Sorties must have positive modeled value. Capture is selected only after the volley succeeds on the trial board, retains at least 30 cavalry HP, passes the existing loyalty screen, and survives the modeled reply. Otherwise the spotter returns; a subsequent observed failed breach prompts withdrawal. Selected units are reserved from the ordinary unit loop. Other aircraft retain their existing strike, pillage and rebase policy.

For the native bridge the host advertises its exact `ReplanFrames` cap. The AI reads current host visibility, not remembered coordinates or inferred simulator visibility alone. An unseen-city opening needs two remaining observations; a visible-city volley needs one. Older hosts without a numeric budget do not start this maneuver. The bridge stops the speculative batch at the required observation: first the spotting move, then the bomber volley, then the cavalry decision from the fresh board. Later orders are recomputed after that observation so they cannot rely on a city captured only in speculation. A bounded `observe/AIR_ASSAULT` batch directive requests the next existing replan even when the host refuses the move or every sortie; it cannot exceed the advertised cap. Native refusal cooldowns enter the planning board before the maneuver is selected.

The installed game uses `Base/Assets/UI/WorldInput.lua:2077–2078` to check and request `UnitOperationTypes.AIR_ATTACK` at the target coordinates. The existing control mod includes `PlayersVisibility[pid]:IsVisible(x, y)` in tile signatures, so regaining sight of already explored ground opens the same replan path as new exploration. A Lua 5.1 test exercises the actual exported functions: regain sight, frame 1, strike, frame 2, cap exhausted, and numeric seat readback.

## Evidence status

All nine focused policy tests and five bridge tests pass. They cover successful capture, affordable return, failed observed breach, refused sorties, explicit host frame limits, reservation from the ordinary unit loop, pending city disposition, and immediate capture with an unused Bomber when no profitable sortie remains. After merging main at `4c3938afd`, `cargo test --profile ci --locked` passes 4,310 tests with 53 existing ignores. All 64 discovered control-mod suites pass under Lua 5.1, each in a separate process. `QUALITY_BASE=origin/main python3 tools/rust_quality.py` reports formatted, warning-free changed lines; `git diff --check origin/main...` passes. No native game has run this candidate, and no win-rate improvement is claimed.

The completed `civvis-20260926T190901Z` baseline at source `01385ba836d0749d4391bf64c83f14a0c1624179` held Egyptian Swenett by turn 183 and the original capital Râ-Kedet by 185; owned-city snapshots show both at turn 188. The game still lost to Mapuche Culture. This is evidence of existing capture capability, not of this candidate. Native runtime adoption belongs to the verification agent at completed-game boundaries; this task never drives the active game tab or writes its order database.

## Preregistered simulator diagnostic

Compare source `5725e0ce77669bd3a87445e7bd51b414d5f8c83c` with this candidate on four complete fixed-profile pairs, seeds 37730000–37730003, using the existing `air-surge-2` evaluator toggle. Each source runs both toggle arms; compare matching arms across sources. Finish all games without early stopping. This is a small diagnostic for regressions and whether behavior changes, not a native win-rate claim or a promotion screen. The opponents are CIVVIS controllers, not Firaxis AI.

Build both with `cargo build --profile ci --locked --features developer-tools --bin victory_eval`, freeze the binaries, and record their SHA-256 hashes before running:

```sh
victory_eval --domination-pair air-surge-2 --games 4 --start-seed 37730000 --out <fresh-file>
```

The runner fixes Gran Colombia seat zero, King, four players, 60×38 Pangaea, six city-states, Online, barbarians, all victories, and the natural 250-turn horizon. Both sources carry the identical compiled live-force-on bundle. Primary checks are focal wins, major cities and original capitals held, and completion; score and turn are secondary diagnostics. Native replay proposals and actual native outcomes will be reported separately.

The first immutable opening-frame replays (190901Z turns 166, 182, 183, 184) all select a combined volley. All four objectives are already visible; they do not prove native spotting. Three trial boards forecast capture, but exported orders contain the sorties plus an observation request and withhold capture. These are fresh-AI, one-frame proposals with `replan_frame_limit=2` explicitly assumed, not native executed outcomes or reconstructions of persistent agent state.

Those replays exposed a prepass integration defect: after a simulated capture, the city-disposition prompt blocked later unit attacks. Candidate `38eb4b15f` invokes the existing disposition policy before returning to the ordinary unit loop; the capture regression now asserts no pending disposition remains. All eight policy tests pass after that correction. The initial frozen pilot at `b7a02984a` will finish unchanged, and the corrected candidate will run the same complete seed set against the same frozen baseline. No outcome-driven stopping or seed substitution is used.

A further regression reproduced a final-frame edge case: an unused Bomber made the prepass wait for another volley even when a visible city already had one HP and no useful air damage remained. `fc32446e0` counts only a profitable visible sortie as requiring another observation; the existing safe capture check can now finish immediately. The regression failed before the fix. The final candidate will repeat all four original seeds in two parallel two-pair shards (37730000–1 and 37730002–3), preserving the evaluator's alternating off/on execution order. The completed earlier variants remain separate evidence.

The final frozen CLI repeats all four immutable native prefixes and emits the same orders as the disposition-corrected variant: two city sorties followed by `observe/AIR_ASSAULT`, with capture withheld. Turn 183 still records one later pending-disposition rejection after a separate ordinary-unit capture. The prepass disposition is resolved; the older ordinary-unit-loop limitation is not changed here.

## Reproduction artifacts

The immutable binaries, source/binary hashes, complete JSONL rows, replay prefixes and manifests are retained at `~/civvis-tactics-results/2026-09-26/bomber-assault/`. No file in that directory is a native runtime input.

| Evaluator | Source | SHA-256 |
|---|---|---|
| Baseline | `5725e0ce77669bd3a87445e7bd51b414d5f8c83c` | `33e8182da972d2cf404b98f295d86261ce48799d9cf28f949e027f6e76d0dea2` |
| Prototype | `b7a02984ac340c983216b7d08602031f8380b01c` | `4e696cb9f36485d38bae2244aab66c0642efa4fdae6137d2205af4e7ae722d27` |
| Disposition correction | `38eb4b15f` | `4f06f2df8db33d388b8850ccab676005d22bc249519936c988871fa896914753` |
| Final capture correction | `fc32446e0` (documentation head `1bd8d8fdb`) | `2d9b4278a947ed5d89bc72cbf951521472d9f93aa436cc02f94af3f3757a3b92` |

The final frozen `civvis_orders` SHA-256 is `470e62feabc5f4eed8726bcf74d40a386eabd892c56e672a48bd5835d360627b`. The pilot isolates the tactic against the same baseline. Later main integration adds independently tested envoy permissions, ladder accounting and speculative-branch performance changes; the full local suite above covers that integrated source, while the pilot binary remains pinned.

## Completed diagnostic

Every preregistered source completed all four pairs, with no crashes or early stops. Each source therefore contributes eight focal results (four per toggle arm). All four sources have zero focal wins, zero original capitals held at the end, five foreign-city holds summed across the eight results, and two foreign major cities ever observed held across those results. The final and disposition-corrected variants match on the recorded turn, victory, score, and conquest summaries.

| Seed | Toggle | Baseline turn / score | Final turn / score | Rival victory |
|---|---|---|---|---|
| 37730000 | off | 180 / 473 | 182 / 479 | Culture |
| 37730000 | on | 185 / 450 | 185 / 450 | Culture |
| 37730001 | off | 178 / 408 | 178 / 408 | Religion |
| 37730001 | on | 180 / 385 | 180 / 385 | Religion |
| 37730002 | off | 232 / 657 | 231 / 650 | Science |
| 37730002 | on | 244 / 348 | 244 / 523 | Science |
| 37730003 | off | 221 / 582 | 221 / 582 | Science |
| 37730003 | on | 211 / 614 | 210 / 624 | Science |

Score changes do not establish stronger domination play. The maneuver is integrated under the existing air-surge opt-in because the controlled tests and bridge replays establish the requested capability; neither deployment defaults nor a promotion ledger change. Native execution of spot → observe → volley → observe → capture/retreat remains outstanding. The broader objective of consistently winning Domination is not achieved by this result.
