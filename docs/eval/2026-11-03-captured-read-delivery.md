# Captured native read delivery: explicit hybrid mode

`tools/civ6_captured_read_replay.py --delivery hybrid` uses a regular snapshot
only when every captured read within that request has identical, nonempty bytes.
Requests containing different reads retain ordered FIFO delivery. FIFO remains
the default, including for existing API callers.

Regular-file readiness does not acknowledge consumption: `served` is unknown.
Qualification still requires the actual native read trace to have exactly the
recorded count, order and bytes, the expected turn/frame identity, and the full
native payload. Extra or missing opens cannot qualify merely because their
orders match. The child process's input/output streams are explicitly closed
on both success and failure.

## Recorded controls

| Original native process | Prefix | Control requests | Regular / FIFO per control |
| --- | --- | --- | --- |
| Mali `20261102T232731Z`, program `3a260648…` | Through turn 60 | 134 + 134 | 128 / 6 |
| Culture `20261103T012701Z`, program `4f269c73…` | Through turn 100 | 279 + 279 | 274 / 5 |

All 826 requests passed the unchanged payload, identity and actual read gates
using the new CLI. These are separate original-program controls, not treatment
arms. The Mali execution preceded a module-docstring-only edit; the archive
preserves its executed driver and an AST comparison excluding that docstring.
The final driver was used for both Culture controls.

A deliberately wrong first reference count of 16 was rejected against 17 actual
native opens, despite matching full payload and frame identity. No host action
or altered strategy was executed by this negative control.

The original Culture FIFO attempt is retained: 266 matching requests followed
by a turn 96/frame 1 deadline, with 14 of 17 identical reads delivered. An earlier
unchanged FIFO unit test also timed out at five seconds. Their intermittent root
cause remains unresolved; hybrid success does not establish that FIFO is fixed.
A private hybrid prototype's additional 558 matching requests are retained
separately and excluded from the 826 CLI total.

## Validation and evidence

The 23 focused Python tests cover real child-process opens, fewer/extra reads,
variable-read FIFO selection, incorrect payload/frame, incomplete capture,
default delivery, and child stream closure. The first new suite exposed open
stream warnings; the cleanup fix and subsequent clean suites are retained.

[Machine-readable adjudication](2026-11-03-captured-read-delivery.json) records
the exact source hashes, per-control counts and archive hash. The
[raw archive](2026-11-03-captured-read-delivery.raw.tar.xz) contains 87 members,
3,594,796 bytes, SHA-256
`269d7e91ef5c7c97e40c0203f5352cf68689cfaab927ae21d8b9b88eb61e63b4`.
It includes successful and failed controls, source/context records, sealed
captures, source patch, logs, `capture-remap.json`, and a per-member hash manifest.
`independent-gate-verification.json` records a separate comparison of every
completed request's archived read bytes, hashes, full payload and frame identity.
Absolute original paths remain provenance; the remap locates archived captures.

This is replay conformance evidence. It changes no policy or live-game setting
and provides no estimate of native win rate or strategy strength.
