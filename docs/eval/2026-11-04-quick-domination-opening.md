# Early development audit: Standard and Quick Immortal domination

Evidence cutoff: native states through turn 91 of `civvis-20261104T055705Z`. Both runs use Gran Colombia at Immortal on Small Continents. The earlier run is Standard; the latter is Quick. The latter is still running at the cutoff. This is a diagnosis, not a treatment win-rate estimate.

## What changes the next action

Early expansion has two distinct failure paths. The Standard run `053740Z` did not observe a second owned city until turn 76, despite a Settler appearing in the capital queue by turn 10. The Quick run `055705Z` observed its second city at turn 21 and third at turn 26, but lost control of Guayaquil at turn 53. Faster founding alone cannot solve both cases.

The strongest next candidate is to improve the housing and defence of early expansion together. Quick Guayaquil and Quito both began with two housing. At turn 35 both had population two and the host reported a housing growth multiplier of 0.25. Quito remained at two housing through turn 75. By turn 87 its Granary had raised housing to four. This is direct native evidence of a long growth restriction, not an estimate from the simulator.

Guayaquil was damaged and had positive loyalty per turn in the last observation before it disappeared. These fields do not prove the mechanism of its loss. Do not label it a loyalty flip from city disappearance alone. The later Cartagena de Indias was founded at the same coordinates (60,25) at turn 75; losing and replacing that settlement consumed another expansion cycle.

The Standard opening needs a separate production-continuity replay: its capital repeatedly switched between Settler and military queues before its first expansion. Queue observations establish interruption, not wasted production or proof that retaining the Settler would survive the attack. Bogotá disappeared from our ownership at turn 85. Preserving the first expansion must be evaluated with capital survival as a guard.

## Selected native city observations

Each row uses the last exported state for that turn (including replans), rather than counting several frames as independent city-turns. Missing queue fields mean no item observed.

| Speed/run | Turn | City | Population | Housing | Housing growth multiplier | Queue |
|---|---:|---|---:|---:|---:|---|
| Standard/053740Z | 10 | Bogotá | 2 | 6 | 1 | UNIT_SETTLER |
| Standard/053740Z | 20 | Bogotá | 2 | 6 | 1 | UNIT_SETTLER |
| Standard/053740Z | 41 | Bogotá | 3 | 6 | 1 | UNIT_SETTLER |
| Standard/053740Z | 75 | Bogotá | 4 | 7 | 1 | UNIT_SPEARMAN |
| Standard/053740Z | 76 | Bogotá | 4 | 7 | 1 | UNIT_SPEARMAN |
| Standard/053740Z | 76 | Caracas | 1 | 5 | 1 | empty |
| Standard/053740Z | 85 | Caracas | 1 | 6 | 1 | UNIT_SPEARMAN |
| Quick/055705Z | 21 | Guayaquil | 1 | 2 | 0.5 | empty |
| Quick/055705Z | 26 | Guayaquil | 1 | 2 | 0.5 | BUILDING_MONUMENT |
| Quick/055705Z | 26 | Quito | 1 | 2 | 0.5 | empty |
| Quick/055705Z | 35 | Guayaquil | 2 | 2 | 0.25 | BUILDING_WALLS |
| Quick/055705Z | 35 | Quito | 2 | 2 | 0.25 | BUILDING_WALLS |
| Quick/055705Z | 41 | Guayaquil | 2 | 2 | 0.25 | BUILDING_WALLS |
| Quick/055705Z | 41 | Quito | 2 | 2 | 0.25 | BUILDING_WALLS |
| Quick/055705Z | 52 | Guayaquil | 3 | 2 | 0.25 | UNIT_SPEARMAN |
| Quick/055705Z | 52 | Quito | 3 | 2 | 0.25 | UNIT_WARRIOR |
| Quick/055705Z | 53 | Quito | 3 | 2 | 0.25 | UNIT_SETTLER |
| Quick/055705Z | 75 | Cartagena de Indias | 1 | 2 | 0.5 | BUILDING_GRANARY |
| Quick/055705Z | 75 | Quito | 3 | 2 | 0.25 | BUILDING_GRANARY |
| Quick/055705Z | 87 | Cartagena de Indias | 2 | 4 | 1 | UNIT_TRADER |
| Quick/055705Z | 87 | Quito | 3 | 4 | 0.5 | UNIT_ARCHER |

## Why the existing housing treatment can yield

`src/ai/advanced/granary_claims_the_bound_queue.rs:55` excludes the threatened city and recently attacked cities. `bound_granary_may_defer` preserves Walls and Settlers and, during any major war, military unit queues. The live force-on configuration includes this treatment. Its presence alone therefore does not establish that a growth-bound city can claim its Granary. The next replay should determine which guard blocks Quito, whether a local threat actually requires the unit queue, and whether another production pass overwrites the claim. Do not remove the defence guards globally from this observation.

## Priorities and acceptance evidence

1. Replay housing-bound secondary cities, starting with Quito at turns 35, 50 and 75. Verify the exact guard and final emitted queue before changing policy. A candidate should obtain earlier useful housing in an unthreatened city while preserving urgent Walls and actual local defence. Measure city growth, production and survival, not just the presence of a Granary order.
2. Replay the Standard first-Settler interruptions, starting with turns 14–23 and 41–43. Distinguish queue suspension, completion, capture and overwritten orders. Evaluate a bounded continuity rule against native attack pressure before live activation.
3. Evaluate ranged finishing separately. The open ranged finishing PR addresses a different bottleneck; the existence of good combat exchanges cannot compensate for a weak production and science base. Avoid combining opening, housing and combat changes into one uncontrolled live comparison.

For subsequent complete games, retain speed, revision, active treatments, second/third city observation turns, city losses, production and science at speed-normalized checkpoints, and actual victory type. Use first and last state frames consistently. Quick turn 67 is approximately Standard turn 100 by the 67/100 cost ratio; equal numbered turns are not equal economic time. Several nearby runs were deliberately interrupted, so they cannot be scored as natural losses or wins. Native replay proves emitted behavior only; full-game outcomes are needed to establish improved strength.

## Failure telemetry caveat

Through order turn 90, Standard `053740Z` reports 192 `CONGRESS_GUARDS_THE_LEADER/combat_policy_not_acknowledged` failures and 21 `MOVE_TO/did_not_move` failures. Quick `055705Z` reports 231 and 92 respectively. Policy acknowledgements, superseded orders and actual unmoved units require separate counts. A total `order_failed` count is not a movement-failure rate, and repeated replans are not independent turns.

## Reproduction and source integrity

Sources are the run-local `events.jsonl`, `decisions.jsonl`, and completed-run `summary.json` under `~/civvis-civ6-runs/control/`. The Quick decider snapshot identifies revision `4cde232756de364389b56af2359b8de40c5b5a2d` and SHA-256 `ad628e49e701fda696512c59809554b50d97f5752cf4dc9a761eb28c075de67d`. Check native city fields `pop`, `housing`, `housing_growth_mult`, `producing`, `damage`, `loyalty` and `loyalty_per_turn`; units use `kind`, not `type`.

For reproducible cutoff receipts, SHA-256 below is over the last turn-91 state serialized with Python `json.dumps(state, sort_keys=True, separators=(',', ':')).encode()`. Later exports do not change the selected cutoff.

- `civvis-20261104T053740Z`: `fa85c0053f697bde4d59f286ef88d9bf8a412646610d54ebb900e53267d1c5a1`
- `civvis-20261104T055705Z`: `9637b08f7dfcbbc20fd1f3b930d9ca653398d2b49e9744373f64301dad3c5622`
