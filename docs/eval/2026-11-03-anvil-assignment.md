# Candidate: the Anvil follows its assigned defense

Default-OFF `anvil-follows-the-defense` lets an already eligible Anvil use its task force's explicit objective-board `Defend(city)` assignment. The existing global threat warning, nearest-land-force check and six-tile relief limit remain required. Withheld, board-disabled, unknown-assignment and other-mission cases preserve the original routing. An assigned city must still be ours and within range. Existing civilian guard reservations, formations, garrison swaps and attacks retain their dispatch paths.

## Native routing mismatch

On Immortal Byzantium Religion start `civvis-20261103T034543Z`, turn 39's board assigned the sole four-unit force 17 to **Defend Constantinople**, while the Anvil formed around **Trebizond**, the strategic plan's threat city. The native capital was lost at 42, Trebizond at 46, and generic local defeat was observed at 81. These later outcomes do not establish a saved-city counterfactual.

A frozen census of 42 recent Immortal/Deity journal and summary prefixes contains 145 distinct single-force Defend/Anvil observations. In 53 observations across 22 runs, the named formation city differs from the explicit assignment. Deduplication uses run, turn, assigned city and formation city; several observations can belong to one turn. The parser requires exactly one board force and a subsequent same-turn Anvil before the next board. Repeated observations are not independent games or treatment effects.

## Recorded controls

The original native executable (`4f269c…`) reproduced 82 requests through turn 40 of 034543, 66 through turn 27 of 040708, and 63 through turn 31 of 025902, each twice: **422 full-payload and actual-read-trace matches**. Its process arguments, working directory, binary bytes and provenance were observed. The newer live `827f5b…` program is separate from these controls.

The final candidate's OFF arm reproduces all 82 Byzantium requests twice, including full orders and decision payloads. ON matches the first 78 order lists and stops at the first changed order at **39/frame 0**, twice. Frame identity and actual read count, order and bytes match throughout each compared prefix. Both ON repetitions produce matching full replies. Its journal names Constantinople as the Anvil city. The first changed emitted orders concern Slinger 196608; a formation label does not establish arrival or execution.

Twenty additional CLI queries compare four fixed captured boards. Each case has older/OFF, candidate/OFF, repeated candidate/OFF, candidate/ON and repeated candidate/ON. Before/OFF and repeat controls match full orders and decision payloads.

| Source run suffix | Turn | Proposed orders change |
| --- | ---: | --- |
| `034543Z` | 39 | Yes |
| `034543Z` | 40 | No |
| `040708Z` | 26 | No |
| `025902Z` | 29 | Yes |

These are development inputs reused for scope refinement, rather than held-out confirmation. The shadow history stops at its first changed order. Later fixed boards are independent proposal queries and cannot describe a resulting world trajectory. There is no native execution, saved-city, victory-rate or default-promotion result.

## Scope refinement and validation

The first prototype also formed an Anvil at turn 6 without the original global warning. That unexpected activation was reproduced twice; its incomplete full Rust run was intentionally stopped. The narrower revision preserves every original eligibility check and overrides only the destination of an eligible formation. The initial explanation blamed a non-Defend fallback; the actual trace already contained Defend, and the correction is retained beside the raw traces. Both prototypes, their controls and the unhelpful early activation remain recorded.

The original-routing scaffold passed three compatibility tests and failed five initial routing tests. The narrowed revision defines nine routing checks, including absent-warning and nearest-force guards. All 14 append-point checks and 21 manifest tests pass. An initial append placement failure, a formatting-order failure and a verifier profile-metadata normalization error were preserved and corrected. No input, payload or read-trace gate was relaxed.

Local `cargo test --profile ci --locked`: **6,705 passed, zero failed, 56 ignored**, including all nine routing checks. The gene activation gate accepts the hash-bound captured-action evidence: 717 of 747 registered genes shown to fire, 30 waived, zero unproven. `genes.py check` and the evaluation manifest check also pass. The new flag remains unarmed and default-OFF; deployment selection, the strength ledger, native policy and reserved research confirmation seeds are unchanged.

The [machine-readable report](2026-11-03-anvil-assignment.json) binds the [raw evidence archive](2026-11-03-anvil-assignment.raw.tar.xz): **4,741,352 bytes, 379 members**, SHA-256 `525c1678935ea8d7a1592f288afe788e0f585734839ebc41721b290212380b0d`. Its manifest and independent verifier cover source inputs, raw replies, actual reads, profile controls and census reproduction. Executable bytes remain preserved outside the task worktree.
