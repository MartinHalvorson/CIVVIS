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

Validation is in progress. Recorded-board replays use fixed observations and are
not counterfactual host games or evidence of a win-rate improvement.
