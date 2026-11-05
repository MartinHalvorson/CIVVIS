# Publish the native map before waking the decision worker

Two qualified cold continuations loaded identical initial full mechanical states from the same native save, with the same executable and only the Campus opt-in differing. Their first movement orders already diverged at turn 5, before any Campus order.

Actual captured inputs establish the boundary failure: all 18 registered baseline reads for turn 5/frame 0 end at its new `state` line. The treatment's 18 reads include a following terrain delta. That delta supplies newly revealed ground, including (16,28), (15,29), and (16,29), plus an ownership change at (19,30). The Python worker dispatches on `state`; the Lua opening handler published state before terrain. The captured request can therefore freeze an incomplete board even though later mirror reads correctly understand same-turn tile records.

The archive `2026-11-05-native-board-completion.native-inputs.tar.xz` (19160 bytes, SHA-256 `902d802c166697602f1109ac13531a6d6c37b23db1ef815b97e853a8ddabad76`) contains both complete captured input byte strings, original hashed decision receipts, observed runtime contexts, and a manifest. Its snapshots are real native reads, not constructed replays. The complete post-run input archives are separately sealed and preserved in the owned pilot results. This evidence identifies a publication race; it does not establish why one worker read later than the other.

The repair publishes all terrain chunks before the state line that wakes the worker. The existing unconditional export-timing record still flushes that state. Frame exports retain their existing sequence and behavior. The ordinary non-CivVis fallback receives the same publication order. No policy, genotype, tile visibility rule, or event schema changes.

Native visibility remains host-authoritative: `Base/Assets/UI/Choosers/EspionageChooser.lua:426` uses `if localPlayerVis:IsRevealed(city:GetX(), city:GetY()) then`. The tile exporter continues to use native visibility and reveal predicates; this change orders their publication.

## Validation and limits

The executable Lua 5.1 regression calls the actual opening-turn closure with instrumented exporter upvalues. Before repair it fails with `consumer woke on state before the new terrain export`; after repair, multi-chunk, empty-delta and failed-export cases preserve the boundary. A failed terrain export must not publish a decision-triggering state.

All 125 discovered mod regressions pass under Lua 5.1, and every mod Lua file passes its syntax check. The public archive verifier validates both original decision receipt hashes, all 36 registered reads, the single-bit genome/configuration comparison, equal city/unit state, and differing terrain frontiers/orders. Native follow-up verification remains pending. A later native request must contain the completed terrain publication before its state to establish live coverage. The running four-arm Campus pilot remains frozen and does not include this repair. Its first OFF result was a Culture loss at153, and the first ON result was a Science loss at164 with no Campus retargeting; neither supports a strength gain.
