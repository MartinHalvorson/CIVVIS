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
shipping qualification.

The refined candidate `9b72a967d7968c666582ce87b23235d98bdc74ef` passed
all eight CI gates. Run42091934983 passed7091 Rust tests with53 skipped,
separate doctests0 passed/4 ignored, and isolation79+21+24. All49 focused
regressions passed:5 confirmed-fortification,3 defense-repair,12 culture-defense
and29 adopted-faith tests.

Mac run42091996102 passed the same49 tests, both native binary builds and both
smokes using Rust1.97.1. All3999 source-file hashes matched the clean pinned
checkout before queries. Orders executable SHA256:
`395c406b130eda25d1d6d5379ded595f35456bae7c454142ca89d4c2a3f64bb7`.
Both refined-candidate cold queries emitted legal Walls, each on17 identical
actual reads and with matching full decision/order replies. The original two
queries emitted Slingers. There are no source-default treatment additions or
removals in these queries. Different source versions and original private
planner histories still prevent an isolated-source causal strength inference.

The portable raw archive `2026-11-05-confirmed-defense-item.candidate.native.tar.xz`
is230288 bytes, SHA256
`8b56ac3bbb68deb0e9eb82866dd9548b26b57a4d9c5252db57b2e19d3dd89eeb`.
It includes the original18 actual native reads and all68 proposal reads, full
replies, raw genome/command logs, build/source receipt and standalone verifier.
The extracted verifier passed. Executables are identified by receipts and are
not packaged; checksums establish integrity, not signatures.

Actual Walls construction, capital survival and Science strength after this
fix remain unmeasured. A separately registered four-role native industry-claim
comparison uses the frozen qualified9b72a967d7 source on two fresh saved worlds.
It changes only the producer-queue claim, keeps Campus-v2 and near-rival ON,
and retains every outcome/failure. That source excludes PR4601's separate Scout
fix; no combined-defense or population-strength result is inferred.
