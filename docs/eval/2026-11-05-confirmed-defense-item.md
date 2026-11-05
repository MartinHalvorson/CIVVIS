# Keep the defense item selected for this turn

The frozen native Deity Korea Campus-v2 ON arm
`civvis-20261105T175245Z-campus-parity-02-on` lost its original capital,
Gyeongju, at turn33. The game was still running when this investigation
selected the already-recorded opening. This is a retrospective mechanism
witness, not a completed whole-game strength result or treatment estimate.
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

The fix preserves the exact claim through the remaining production passes.
An already-running defender still survives when the selector reserves it.
A queue without a confirmed claim remains with its ordinary production writer.
The existing threat proof, legal-item selector, one-city arbitration and
opt-in activation paths stay in place. No new policy, default, registry entry,
native adapter or game rule is added. This is separate from PR4601's recon
classification fix; their helper hunks do not overlap.

Regression tests cover a fresh Archer replacing selected Walls after the
threat evidence disappears, retaining an existing Archer through the selector,
and leaving a queue without a claim unchanged. Exact-source CI and Mac native
qualification are pending. Actual Walls construction, capital survival and
Science strength after this fix remain unmeasured.
