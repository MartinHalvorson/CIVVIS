# Keep the fortification selected for this turn

The completed native Deity Korea Campus-v2 ON arm
`civvis-20261105T175245Z-campus-parity-02-on` lost its original capital,
Gyeongju, at turn33 and was locally defeated at148. The25/0 opening witness
was selected retrospectively at18:35:21UTC, after the arm finalized at18:32:18.
The initial draft called it ongoing before checking that terminal status; this
corrects that label. It is a mechanism witness, not a treatment effect or
strength sample.
Its source is `e520fc4582e5267060c66f099f81228a28faf09d`, with executable
SHA256 `88257cc63ce3eccfdd4aeec1250fbca514af99de3e614cdbf899a61064688e2c`.

At25/0 the native capital had finished its Seowon, had an empty production
head and offered legal four-turn Ancient Walls. Germany was at war. The
journal selected Walls for the confirmed siege, then the production governor
switched the simulated queue to a Slinger. The actual top-level native order
was `UNIT_SLINGER`, not Walls. At26 the journal again selected Walls and then
switched to a Slinger. At27 it selected Walls and then an Archer. The native
capital accumulated damage and had no completed Walls before its loss.

The original25/0 record has canonical SHA256
`47ddc1ea686e254392990291eb64f5e3a2d6c98ae5f8bf3c0179375c87b9948c`.
All18 actual reads are identical, with input SHA256
`0f6804643755d1127b92e9c2f6215a4136ad58ecd37f1b74b1ac300413bce971`.
Two cold original-source queries reproduced the Slinger order and the same
full reply, with17 identical actual reads each. They did not deliver orders
to Civilization VI and cannot recreate the original private planner.

`redirect_unsafe_city_queue_for_defense` already preserves an existing legal
defensive head by returning that exact item as its claim. However,
`reapply_confirmed_defense_queue` accepted any later defensive military item,
allowing a fresh governor pick to replace the Walls it was meant to restore.

The fix preserves a selected non-unit fortification through the remaining
production passes. Unit and economic reservations retain the existing allowance
for a later local defender, so an earlier one-turn Campus reservation cannot
block a fresh military response. An already-running defender still survives
when the selector reserves it. A queue without a confirmed claim remains with
its ordinary production writer.
The existing threat proof, legal-item selector, one-city arbitration and
opt-in activation paths stay in place. No new policy, default, registry entry,
native adapter or game rule is added. This is separate from PR4601's recon
classification fix; their helper hunks do not overlap.

Five regression tests cover restoring selected Walls over a fresh Archer,
retaining an already-running defender, leaving an unclaimed queue alone,
allowing a later defender over a reserved Campus finish, and retaining the
legacy defensive refinement of a unit reservation.

The initial82bcc3d3b3 prototype passed full CI and Mac run42086133426
(47 focused tests, both native binaries/smokes); both cold queries selected
legal Walls with17 identical actual reads each. Review found its exact-all-claims
restoration too broad, so that prototype remains diagnostic and is not a
shipping qualification. The refined source needs fresh exact-source CI and
Mac qualification. Actual Walls construction, capital survival and Science
strength after this fix remain unmeasured.
