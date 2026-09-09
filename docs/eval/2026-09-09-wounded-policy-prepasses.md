# Selected withdrawals before tactical kill prepasses

The `wounded-out-of-reach` opt-in had a working withdrawal policy that several
attack prepasses could bypass. The live finishing volley runs before the native
AI; native battle planning and its immediate-kill predecessor run before the
ordinary military step that invokes the withdrawal.

In `civvis-20260909T013436Z`, turn 177 frame 0, Galley 851976 began at
(65,40), 40 HP. It attacked the 24 HP enemy Galley at (65,41), survived at
10 HP, then died to an enemy Galley it had last seen on turn 173. A persistent
21-frame replay through the unmodified live decider attacked with and without
`wounded-out-of-reach`. A direct invocation of the existing withdrawal policy,
primed with the observed turn 173 frame 2 board, instead moved the Galley to
(67,36). The observed enemy was retained in memory; no hidden current position
was supplied to the decision.

This repairs the selected policy's integration. The live volley reserves units
whose withdrawal policy takes their turn. Native battle planning executes the
withdrawal before selecting strikers and retains those units as already ordered.
The immediate-kill predecessor similarly excludes withdrawn units and leaves
their remaining turn alone. This includes a ship holding position with no better
refuge: ships cannot issue the engine's Fortify action, so movement points alone
cannot identify a completed withdrawal decision. Bound escorts, civilian rescue
priority, and the ordinary threatened-city exception remain in force on the
native path. No deployment default changes.

Evidence files are retained under the local tactical evidence directory:
`013436-turn173-frame2-with-map`, `013436-turn177-frame0-with-map`,
`013436-galley-memory-persistent`, `galley-policy-probe.rs`, and
`/tmp/civvis-galley-direct-policy.log`. The native full-turn probe also attacked
with battle planning disabled: disabling it enables the immediate-kill prepass,
so that probe alone did not isolate the withdrawal. The direct policy diagnostic
did isolate it.

The four new library controls passed within 20 focused wounded-unit tests, and
one live finishing-volley integration test passed. These cover policy off,
withdrawal through both native prepasses, holding a ship that cannot fortify,
finishing the only threat, and retaining threatened-city defense priority.

A same-binary, three-arm persistent replay passed on all 21 observed frames from
turn 171 frame 0 through turn 177 frame 0. With the withdrawal explicitly
withheld, the final order remains `ATTACK (65,41)`. With the selected withdrawal,
and with withdrawal plus `doomed-blow-veto-2`, the final orders move to (65,39),
(66,38), (66,37), and (67,36). Evidence and exact source/binary hashes are in
`013436-galley-memory-repaired-control/provenance.json`. This binary was built
from `ef2bc4ef7`.

The current base already includes `wounded-out-of-reach` in its deployment
genome after a separate measurement-driven change. Consequently the treated
arm uses the deployed configuration, and the control explicitly uses
`--without wounded-out-of-reach`. Trying `--with wounded-out-of-reach` is
correctly rejected as a duplicate deployed selection. This repair makes no
change to that selection.

The complete non-documentation suite passed 3,363 tests with 46 ignored;
documentation tests then passed with four ignored. The changed-line formatting
and clippy gate passed after formatting the new code. A transient formatting
edit dropped the new collection import; the documentation build detected it,
and the import was restored before its successful rerun.

Recorded-board replays use fixed observations and are not counterfactual host
games or evidence of a win-rate improvement.
