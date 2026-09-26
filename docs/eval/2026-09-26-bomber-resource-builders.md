# Connecting aluminum for the bomber wing

The completed native run `civvis-20260926T190901Z` revealed Taruga's Aluminum at offset `(40, 24)` on turn 135. Gran Colombia already held suzerainty, nine Envoys against the nearest rival's seven. Its own Builder `5570585` stood at `(39, 24)` in a turn-137 state with one charge. The deposit was not mined until turn 151; Aluminum income first appears at turn 152. Advanced Flight had arrived at turn 140, and Cali already held a completed, unpillaged Aerodrome. No Bomber was native-buildable there at turns 140, 145 or 150; it appears in the turn-154 menu, and the first plane is observed at 159.

The existing engine permits a Suzerain to improve and repair a city-state resource, verified by `suzerains_improve_repair_and_accumulate_city_state_resources`. Both ordinary Builder job sweeps enumerate only their own cities' tiles. A nearby unconnected client deposit is therefore absent from the job candidates. This supports a supply task; it does not prove the peace at turn 132 caused the delay or establish the counterfactual date of the first native Bomber.

The candidate will reuse the existing bomber supply-shortfall calculation rather than invent a second demand target. Only the existing air-surge opt-in in the Domination lane can assign the new task. An available Builder may connect a nearby revealed source in our territory or a current Suzerain's territory, after higher-priority civilian safety and project duties. Resource visibility, legal improvement/repair, path length, other Builder reservations and the risk of capture bound the errand. The native host remains authoritative over acceptance and actual income.

## Preregistered checks

- Reproduce the missing allied job through the ordinary `advanced_builder_step`, before adding the policy; assert actual resource income after mining in the corrected fixture.
- Cover lost suzerainty, sufficient supply, hidden Aluminum, missing commitment, existing Builder reservation, repair, hostile reach and route limits.
- Preserve the existing resource-purchase tests while extracting the shared calculation; run full Rust and changed-line quality checks.
- Compare one-frame proposals on immutable opening-frame prefixes of native turns 135 and 137. Baseline frozen CLI source is `fc32446e0`, binary SHA-256 `470e62feabc5f4eed8726bcf74d40a386eabd892c56e672a48bd5835d360627b`; its later air-assault code has no aircraft to act on these boards. The task starts from main `4c3938afd`, which also includes independently merged envoy permissions and performance work, so full replay differences are not an isolated whole-agent A/B.
- Complete two candidate smoke pairs with the existing `victory_eval --domination-pair air-surge-2 --games 2 --start-seed 37790000` profile: Gran Colombia, four players, King, Tiny Pangaea, Online and the natural 250-turn clock, against CIVVIS opponents. Finish both seeds regardless of result. This checks game completion; it is not a before/after comparison or a win-rate screen.

Artifacts live under `~/civvis-tactics-results/2026-09-26/bomber-resource-builders/`; the earlier compact source timeline is `../bomber-assault/upstream-aluminum-timeline.json`. Source-event SHA-256 is `2751e3a479dcc8e470bf347f4b1cd4721e3a0d741be55093155700816cdad94a`. These are disposable, read-only replay inputs. No active game tab, runtime or order database is touched.

## Local results so far

The original ordinary Builder regression compiled and failed at the missing Mine assertion (`None` versus `mine`). The corrected version connects the tile during the Radio-to-Advanced-Flight window and derives two Aluminum per turn from the engine. Five new tests pass, including thirteen no-action cases, client-mine repair, lost-suzerainty follow-up and visible hostile reach. The three existing resource-purchase tests selected by `cargo test --profile ci --locked --lib air_resource` also pass. `cargo test --profile ci --locked` passes 4,301 tests before main integration, then 4,318 after integrating main `72b0d8bee` (including the assault and native-flooding changes); both runs have 53 existing ignores. Changed-line Rust quality and whitespace checks pass after integration.

Frozen candidate source is `58908497b98e317fd5929e941f3ee4877bd98ba9`. Its CLI SHA-256 is `fe659b1fdaeace5e926a46c1d3aa78ad0792f6800a3531dcdf375faf1b6f7c17`; evaluator SHA-256 is `61a39f6b0c46055742579d20a3d43ed65c84d727dbb8a13af0490d7cf7034152`.

Both immutable native replay commands complete. At turn 135 the selected Builder's order is unchanged. At turn 137 the new policy records an approach to Aluminum `(40, 24)` and exports a legal modeled `MOVE_TO (39, 25)` for native Builder `5570585`; the baseline exports no order for that unit. These are fresh-agent proposals on an opening frame, not actual native movement or a mined deposit. The controlled fixture proves the eventual income; native acceptance and timing remain to be observed.

Both preregistered candidate smoke pairs completed, with no crashes or early stops:

| Seed | Toggle | Ending turn | Score | Rival victory |
|---|---|---|---|---|
| 37790000 | off | 145 | 352 | Culture |
| 37790000 | on | 143 | 349 | Culture |
| 37790001 | off | 236 | 774 | Science |
| 37790001 | on | 140 | 463 | Religion |

All four focal games lost, with no foreign major city ever observed held and no foreign original capital held at the end. These are candidate-only completion checks on the frozen pre-integration source, not evidence of improved play. The later integrated source has the complete test coverage described above. No deployment default or promotion record changes. No native gain or win-rate improvement is claimed.
