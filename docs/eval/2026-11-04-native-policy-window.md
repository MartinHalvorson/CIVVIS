# Native policy-change windows in the planner

The native controller now respects the Government UI's free-change window
(#4428), but the Rust mirror still treated every available policy card as
immediately swappable. In the observed `civvis-20261104T183134Z` game, the
original planner proposed 267 decks: 20 in free windows, seven while a change
was pending, and 240 with a positive unlock quote. This is proposal telemetry,
not proof of fee payment or accepted changes.

The mirror now imports `policy_change` into seat zero, stamped with the observed
turn. An object permits changes only when its finite unlock cost is exactly
zero and `made` is explicitly false. A present partial or invalid observation
blocks changes. Absent observations preserve legacy behavior; a stale reading
does not lock hypothetical future turns. Ordinary simulated games have no
host observation. No fee payment is planned.

The action menu, card mutation methods, normal policy review and advanced
policy retirement share this restriction. A confirmed free window triggers
normal review even off its periodic cadence. Government changes remain
independent. Already-held-card errors retain their existing behavior; Lua's
complete-deck acknowledgement remains a separate executor concern.

The behavior follows shipped
`Base/Assets/UI/Screens/GovernmentScreen.lua:1383-1422`: `PolicyChangeMade`
and `GetCostToUnlockPolicies` control the pending, free Confirm, and paid
Unlock branches. Line 1592 separately requests `UNLOCK_POLICIES`. The native
controller guards execution independently.

## Captured-native qualification

Source revision: `57b5d266d94ea0033cb19a24c3206803a5fdd5c9`.
The observed binary's exact argv/cwd, immutable binary hash, actual genome,
signed decisions and all captured read versions were preserved. Two original
controls reproduce all 102 complete replies through turn 45/frame 2.

Candidate source checkpoint:
`af101683091f3c0870d32f8e23211e46d3ecd4f9`.
Its frozen local executable is a constructed query candidate, not an observed
native process. It uses profile `ci` with local overrides
`CARGO_PROFILE_CI_LTO=off CARGO_BUILD_JOBS=4`; hosted gates retain their normal
settings. Its exact executable and production-source hashes accompany the
machine-readable report.

The first full-payload difference is at 1/0: consuming the known
`policy_change` field removes only `schema:state.policy_change` from the
unmapped-field metadata. Orders are identical there. Two separate action
probes first differ at 17/1, removing the pending duplicate deck. The free
17/0 deck is retained. Paid proposals at 40/0, 40/1 and 40/2 are removed.
The newly observed free window at 42/0 permits a deck even off normal cadence;
42/1 also repeats a proposal against this frozen original history.

Complete per-arm expected replies are retained for every query. Two successful
pairs each run all 102 requests, use the same capture argument within the pair,
verify every actual read's bytes/count/order and identity, retain raw capture
maps, and close stdin with actual exit zero. Their genomes match exactly;
only the executable changes. A separate verifier reconstructs actual reads,
checks signed native records and archive hashes, and binds every selected
native state to the actual captured last read. Verification-message changes
at turns 18, 41 and 43 are retained: query-issued bookkeeping changes while the
host outcomes remain those of the original game. They are not evidence of a
continued candidate world.

Two additional candidate controls replay 290 requests through 120/2 from the
older `civvis-20261104T140152Z` corpus without `policy_change`; every full reply
and actual read sequence matches the original. These checks establish legacy
compatibility on that corpus, not every possible older snapshot.

The independent check covers 34 free and 68 blocked requests. The candidate
emits neither policy-card actions nor policy decks on the blocked requests.

The accompanying JSON records the independent qualification and all order
changes; the compressed native prefix binds the observations. Runtime artifacts
and full query receipts remain outside the repository under
`civvis-tactics-results/2026-11-04/native-policy-window/`.

No gene is added or promoted. This corrects the planner's current-turn action
assumptions; there is no strategy-strength or win-rate claim. Fresh native
execution after merge is tracked separately from captured-history queries.

## Local validation

`cargo test --profile ci --locked` completed with 6,848 passing tests,
53 ignored unit tests and four ignored doctests, using the local profile
overrides disclosed above. All six new host-window/mirror tests pass.
Incremental `tools/rust_quality.py` reports the changed lines formatted and
warning-free. The latest main merge was already current at the source revision
above; no production source changed after the frozen candidate was built.
