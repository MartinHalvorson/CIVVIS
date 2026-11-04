# Opt-in capital Governor Loyalty reserve

The native Campus-screen game `civvis-20261104T192352Z` lost its original
capital, Bogotá, to Free Cities at turn 90. It was undamaged, at peace and on
1.89062 Loyalty with a -3.95312 rate in its last owned turn. This is an observed
Loyalty loss, not a siege classification inferred only from disappearance.

At 69/0 the original planner moved Magnus from Bogotá to Caracas for emergency
Loyalty relief. Bogotá had 96.3125 Loyalty but was already losing 0.515625 a turn
with Magnus. Caracas had 66.7188 Loyalty and a -4.21875 rate. The assignment was
checked natively; at 70/0 Bogotá's rate was -9.0625. Magnus returned at 78/0,
when Bogotá had 22.875 Loyalty; that assignment was also checked. Retaining
Magnus is a plausible intervention, not proof the capital survives: nearby
population pressure continues changing, and the two cities compete for one
Governor.

The fixed first-ten Campus launch-context audit contains seven own-city
Governor-transfer proposals. This was the sole donor already losing Loyalty.
The other capital donor had a rate of +28. These are observed proposals in
mixed worlds/revisions, not randomized treatment outcomes. Their state lines
and signed decision identifiers remain in the external audit.

`capital-keeps-its-governor` is a new, independently screenable opt-in, default
OFF. It prevents emergency and economic Governor departures from our own
original capital when its estimated post-departure Loyalty rate is negative.
Other cities, foreign-founded capitals, unassigned Governors and safe capital
departures retain their old permissions. It neither adds a Governor nor spends
a title or policy unlock.

A positive current rate is insufficient: it already includes the Governor.
The helper removes the fixed observed Loyalty-rate override in an analysis
clone, measures the rules model's support difference with and without that
Governor, then subtracts that support from the actual current rate. This also
prices modeled policy and local Governor-dependent effects. The model-derived
difference is an estimate of immediate departure effects, not a native
counterfactual measurement or forecast of future pressure. No trial action or
metadata mutation reaches the actual game.

Shipped primary rule:
`DLC/Expansion1/Data/Expansion1_Governors.xml:72` gives Magnus
`IdentityPressure="8"`. Native readback at 79/0 already reflects his return
before establishment, so the guard must account for assigned Governors rather
than waiting for their economic promotions to establish.

## Qualification scope

Two original-binary controls reproduce all 225 full native replies and exact
actual read sequences through 90/2, with successful EOF exits. The raw prefix,
signed decision records, binary/argv/cwd/genome context and sealed capture
archives are preserved under
`civvis-tactics-results/2026-11-04/loyalty-donor-audit/`.

The policy-window correction (#4430) changes earlier metadata and policy
orders, independently of this new treatment. Its frozen local executable is
therefore separately queried twice on this corpus as the reference for the
candidate with this gene OFF. The second reference checks complete full
payloads from the first; both finish all 225 requests with actual exit zero.
A new gene comparison must use the same corrected executable and background
for OFF and ON, and identify its first difference separately from the earlier
policy correction.

The Campus screen remains unchanged. There is no new live arming, default
promotion, saved-capital claim or win-rate claim from this work.

## Verified proposal change and safe-transfer control

Candidate checkpoint: `f088c8e7f6715397ed72dd203938fbf294190343`.
Executable SHA256:
`9f1ef5d168f321108da7475eebd822a047c447587afeac36c2cb576fe2375997`.
Both candidate-OFF controls reproduce the corrected reference's complete
orders/decision payloads and actual read sequences on all 225 requests. Two
ON probes first differ at 69/0, request 164. Two successful persistent OFF/ON
prefix pairs each close stdin and actually exit zero; they share the capture
argument within each pair, vary only the named argv/genome bit, and verify
every actual read byte version/count/order and the full per-arm payloads.
Only the `governor_assign` of Magnus to native city 196609 is removed. There
are no added orders, and every other order keeps its exact sequence.

The independent verifier checks canonical native decision hashes, binary
metadata, source exports, full repeated payloads, exact actual read sequences
and capture maps, and binds the 69/0 native pre-state to its captured last
read. The original native assignments at 69 and 78 are checked in the preserved
host reports, with actual Free Cities ownership at 90/0. These remain original
host outcomes; the ON prefix stops at its first difference.

A separate authentic native history, `civvis-20261104T193444Z`, contains a safe
capital transfer at 106/0, with +28 Loyalty per turn before Magnus leaves.
Two original controls reproduce all 344 native payloads through 130/2. Two
corrected reference queries repeat the same corrected planner, then two
successful OFF/ON pairs each query all 344 requests. Every orders/decision
payload remains unchanged, including the safe transfer to Guayaquil (native
city 262147); all actual reads match and all four EOF exits are zero. The
independent verifier checks this negative control too.

The hash-bound `.native.tar.xz` archive contains raw captured inputs, signed
native records, actual per-arm replies and EOF receipts, both source contexts,
source exports, verifiers, build/test receipts and the frozen pre-query intent.
Exact duplicate files are tar hardlinks after SHA256 and streaming byte equality
checks; the raw content is preserved. Binaries are identified by immutable
hashes and source checkpoints rather than bundled. See its `README.txt`.

## Validation and deployment

At the initial qualification checkpoint, all six new focused tests pass. The
full local Rust suite passes 6,854 tests,
with 53 unit tests and four doctests ignored. Incremental Rust quality reports
the changed lines formatted and warning-free. The local `ci` build uses
`CARGO_PROFILE_CI_LTO=off CARGO_BUILD_JOBS=4`; repository/hosted profiles are
unchanged. Own compiler and test processes were restored from Darwin background
scheduling so the builds could finish, retaining the four-job build cap.

No engine rule changes or deployment-default changes are made. This is a
selected development history and negative control, not a strength screen.
The ongoing Campus screen and force-on file remain unchanged; this gene stays
OFF and unarmed. Any later live screen must track loss of both the original
capital and the city that no longer receives the Governor, alongside science,
production and expansion.

Fresh extraction re-verifies all 167 manifest file hashes, source exports and
the independent positive/negative corpus checks (verifier exit zero). The
3,242,756-byte archive SHA256 is
`6e5a42bf8368647443f963f1e9492ca94b1f7554d4664a2bb368de49d53e114c`.
`tools/gene_fires.py --max 0` passes: 769 genes, 733 shown to fire,
36 existing waivers, zero unproven; no new waiver. Evaluation inventory and
gene-ledger/ranking consistency checks pass.

A subsequent append-marker placement correction moves four marker comment
lines only. Removing append-marker lines leaves every remaining source byte
identical, including field and registry order. The rebuilt production
executable is byte-for-byte identical to the qualified executable above;
14 targeted append-point tests and incremental quality pass after the repair.
The `.layout-repair.json` records old/new source hashes and the rebuild.
The original raw evidence archive remains unchanged.

## Requalification after the main conflict

The real main conflict was resolved against `923ff7807` in checkpoint
`ff86613495432b0ead5c36ec6df3ccf36d7250cc`, retaining both independent capital
safeguards and the existing registry order. The dedicated Governor helper and
its tests are unchanged. This is a fresh build and query qualification, unlike
the earlier byte-identical marker-only repair. Executable SHA256:
`5629bd2114abb55add8647e6a92d62c541aad99d47c1c224eea0fb9de480f00f`.

The merged candidate repeats both full 225-request OFF controls, both ON
first-difference probes and both successful 164-request OFF/ON prefix pairs.
The only order removed remains Magnus's transfer at 69/0. The safe corpus
repeats all four 344-request OFF/ON queries and preserves every payload,
including the safe 106/0 transfer. Original native controls and corrected-policy
reference queries are reused unchanged and are not claimed fresh. One initial
negative ON query timed out after 44 replies; its partial data is preserved
and excluded from successful controls. The complete four-query retry restores
only its own child's Darwin scheduling, retaining the same code and deadline.

The merged library suite passes 6,486 tests, with zero failures and 53 ignored;
14 append-point tests and incremental Rust quality pass. The current firing
ratchet reports 770 registered, 733 proven, 37 existing waivers and zero
unproven; this feature adds no waiver. The prior 6,854-test full-suite result
belongs to the initial checkpoint.

The separate `.post-merge.native.tar.xz` composite archive preserves the raw
inputs, reused original controls/reference, fresh per-arm queries, source
exports, verifiers, timeout data and validation logs. Fresh extraction verifies
all 166 manifest files and reruns the independent verifier successfully.
Archive size: 3,235,644 bytes; SHA256:
`e3c3691fbf75ff4306fc1e8a8fe85808d43cb95d217318ecc7e884ff05f32209`.
The `.post-merge.json` records the full results and source hashes. The original
archive remains unchanged. Default OFF, no native ON or strength claim.
