# Native material trial: input qualification

The eight-start trial registered in #4119 was armed at 12:24:41 UTC on
2026-10-25 after the repaired, unarmed qualification described below. Its
comparison remains the existing `army-upgrade-materials` buyer plus the new
stock reserve against that policy being off. All original inclusion,
continuation, outcome and interpretation rules remain in force.

## Why the recorder had not activated

The supported host policy contains `CIVVIS_CAPTURE_DECIDER_INPUTS=1`, but
the actual game driver for `civvis-20261025T051724Z` lacks the capture flag
and its run has no input archive. At the beginning of this continuation the
live supervisor is still the process launched from the older first-foothold
worktree. Its source has no input-capture selector. Updating the runtime
checkout does not replace this long-lived shell or its inherited environment.
That game does not qualify the recorder and receives no trial arm.

That completed game lost to rival Culture at turn 203, with eleven cities,
six cities at turn 60, three combat captures, four city losses, 46 kills and
26 unit losses. It used frozen source `8810924ee`, before the stock reserve.
It is development evidence and does not measure the repair.

## Boundary reload and actual qualification

The previously requested safe boundary reload completes after that game.
At 05:39:53 UTC the authoritative process snapshot shows supervisor 68618
running the managed source, and game driver 69075 for the new unarmed game
`civvis-20261025T053159Z` carries `--civvis-capture-inputs`. The companion
precondition archive preserves the complete actual process commands and
policy digest. It proves capture is enabled, rather than the earlier
inference that this second game also lacked it.

A prospective per-invocation policy fallback was implemented and tested in
this branch while the reload was unresolved. Once actual capture appeared,
that extra change was removed from the final diff. The registered reload
already supplies the required recorder, so no new launcher behavior is needed
for this trial. Its intermediate pushed commit remains in branch history.

The new game's first decision is turn 1, frame 0. It reports a complete
capture containing seventeen reads of snapshot zero. Reconstruction returns
28,858 bytes with SHA-256
`b365f5ae46cc8f1b4bc6ea55d97e29b2a028ed6f8c2e0dfac21cdf3120c36a55`.
Those bytes include the seat, native state and terrain export. The
qualification archive retains the complete descriptor, first decision,
status, runtime receipt and reconstructed read. This verifies captured input
structure and bytes, not host execution or replay equivalence.

The frozen binary is still source `8810924ee`, SHA-256
`20fe763757f68a6ccf024ad8d57ab7bf6b35acdadc56842761b802d36f0b531e`.
It predates the stock reserve. The repaired native binary must be
verified after #4119 merges before arming the previously registered trial.
A failed or missing qualification never starts that cohort, and an assigned
game's later capture failure never removes or replaces it.

## Repaired native deployment and occupied screening slot

PR #4119 merges as `a605bd591cab1ab6d294691f412e0d5e42e8ded4` at
05:44:18 UTC. Native game `civvis-20261025T055158Z` runs that revision.
Its actual frozen binary hashes to
`cf351ea77cdf0b10d5f5d6ecbd322006c8d1f2f96f5b299f20ee7a01ac02df77`,
matching both runtime and decision receipts. Its first captured request is
complete and its referenced reads reconstruct. The native-deployment archive
retains the actual driver, runtime, first decision and genome evidence.

This game belongs to a different owner's `unwalled-ring-steps-in` screen.
The material policy is absent from its played and forced lists. It is not
a material-trial assignment or an unarmed repaired qualification. Preserve
that screen; do not activate this trial over it. The previously prepared
`2026-10-24-upgrade-material-reserve-native-trial` directory is another
owner's state and was inspected read-only. At that inspection no observer from this PR had
been activated. Native deployment was verified; the separate local
exhibition server was still serving `c1803a7df` when inspected.

## Armed qualification and first assignment

The other owner's screen subsequently completed and released the slot. The
unchanged bounded observer qualified the unarmed Emperor game
`civvis-20261025T122036Z`, then exclusively created the
`army-upgrade-materials` screen setting at 12:24:41 UTC. Its frozen native
source is `8b131f11070379326ef98b6b6fc95ce0e5cd5e86`, verified to descend
from the material repair. The actual binary SHA-256 is
`c39a27153b657e29e3ba46b231b2243bf8e3593bbb543e3ecec2d159df42d2f0`,
matching its runtime and first-decision receipts. All seventeen first-request
reads reconstruct to 30,939 bytes, SHA-256
`bbf2a0e954040543a4ddaee3ea3c8018703999e79209a9001bd8dd36e8ef49a1`.

The qualification game is excluded from the cohort. At the 12:37:55 UTC
readback, one future start has been assigned: `civvis-20261025T123652Z`.
Its actual driver carries `--screen-gene army-upgrade-materials` and
`--screen-arm on`. Played-genome readback and terminal results remain pending.
The screen chooses each arm from the existing tag hash; eight starts do not
mean eight treatment games. All failed starts, missing readbacks and
continuations remain assigned; none can be replaced. The observer is running
and will remove only its own unchanged screen setting after eight starts.

The activation archive is 15,452 bytes, SHA-256
`1bd97039f44aa3647e876f27ce61e600147465f8a6708b57556db5d14c1632f6`.
Every member was read back and compared with its source. It retains the
qualification, reconstructed input, observer source/log, assignment-state
snapshot and actual driver/screen-identity readback. This proves activation
and qualification, not a native strength result. Outcome collection remains
mandatory after the start cutoff.

## Validation

The discarded fallback passed eleven focused policy/runner tests and all
220 launcher tests. Its broader developer-tool run executes 3,431 tests:
one fails because the existing roadmap names `src/ai/advanced/tests.rs`
as a conflict hotspot although its current measured touch rate is 3%, below
the 5% floor; one test is skipped. This unrelated failure is preserved and
is not a claim of full-suite success. No Rust or launcher changes remain in
this evidence-only final diff. There is no repair-strength result yet.

## Interim retained outcome and native upgrade audit

The first assigned game, `civvis-20261025T123652Z` (on), loses to rival
Culture at turn 174 with zero combat city captures. Its played genome and
actual driver match the independently rederived tag-hash arm. All 476 decision
capture descriptors are complete and have no failed read. Each issued upgrade
is checked against every actual captured read and the next unique native state;
all eleven have a unique quoted successor at the same tile with matching XP,
level, formation and promotions. These include upgrades outside the material
buyer's scope and do not establish attribution to purchased material.

The companion `.audit.py` retains missing quotes, changed veteran identity,
multiple possible successors, host blocks and incomplete/ambiguous observations
as unresolved. Its three tests cover thirteen upgrade-identity cases, the last
actual native export, and missing/malformed inputs. The unarmed qualification
game is a separate development check (sixteen confirmed transitions from twenty
issued orders); it is excluded from trial results.

The first-game archive retains complete event, decision, reasoning and brain
logs, input-capture byte archives/indices, runtime/genome/arm readbacks, all
upgrade audit states and the auditor/control source. It is 1,748,756 bytes,
SHA-256 `25c7dfc8e94753c808535667de1617f02d5a650fd4f50a6adff99330cc4fe29c`;
all fourteen members are read back against their sources. Descriptor completeness
alone does not establish whole-history replay equivalence. The second game is
assigned off and remains in progress. Keep all eight starts and terminal or
missing outcomes; these interim observations never change the cutoff or arms.
