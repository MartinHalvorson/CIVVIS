# Aircraft must use air strikes

The native `053144Z-cont5` game emitted ground `RANGE_ATTACK` orders for Bombers. The full frame planner could select `Action::Ranged` for a finishing blow because both tactical legality and execution accepted aircraft, despite the ordinary legal-action enumerator correctly offering air operations.

`Base/Assets/UI/Civ6Common.lua:139-148` in the installed Civilization VI scripts branches on `DOMAIN_AIR`, calls `UnitManager.CanStartOperation` / `RequestOperation` with `UnitOperationTypes.AIR_ATTACK`, and uses `RANGE_ATTACK` in the other branch. This change rejects aircraft in ground ranged legality and execution, preserving the existing air-strike path.

## Validation

Three regression tests failed before the change and pass after it: four aircraft types reject a ground shot without spending movement/attacks or damaging the defender while air strikes remain usable; ground/naval ranged controls remain legal; a full native decision frame uses an air strike for a Bomber finishing blow.

`cargo test --profile ci --locked`: 3,645 library and 204 binary/integration tests passed; 49 library and four documentation tests ignored. Latest main was merged before this run.

Crash soaks (general engine stability, not native domination evidence):

```sh
target/ci/civvis soak --games 6 --players 4 --start-seed 936004 --jobs 4 --width 32 --height 20 --turns 160 --start-era atomic
target/ci/civvis soak --games 6 --players 4 --start-seed 936010 --jobs 4 --width 32 --height 20 --turns 160 --start-era ancient
```

Both completed 6/6 games, exit zero, without crashes. The atomic start does not establish that aircraft were exercised; the focused tests do.

## Frozen native decision replay

Replayed 62 distinct turn/frame decision prefixes from turns 193–217 of `civvis-20260920T053144Z-cont5`. Each arm uses one persistent `civvis_orders --serve --fresh-board` process with the same 19 forced genes and domination target. The exported event file is extended only through each corresponding await marker, so later host refusals are not visible prematurely. Baseline is the production code at c321c208e; candidate changes only aircraft ranged legality/execution. Both arms exit zero.

For native Bomber IDs 10158084 and 10878985:

| Command | Baseline | Candidate |
| --- | ---: | ---: |
| RANGE_ATTACK | 11 | 0 |
| AIR_ATTACK | 9 | 10 |
| Promotion | 1 | 1 |

Twelve decision frames change actionable orders after excluding `order_failed`, `order_verified`, and `turn_verified` telemetry. This is a fixed-observation replay, not a counterfactual native game: historical failures remain in later inputs. It demonstrates removal of the wrong command, not eleven additional successful strikes. Some native AIR_ATTACK requests also failed, and no new capture or domination win is established here.

Local evidence: `/tmp/civvis-air-domain-replay/frame-{baseline,patched}-orders.jsonl`, corresponding why logs, and `/tmp/civvis-air-frame-replay.py`; test and soak logs use `/tmp/civvis-3604-*`.
