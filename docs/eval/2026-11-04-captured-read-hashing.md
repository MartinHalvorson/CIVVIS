# Reuse captured-read hashes without changing the audit

A late native request contained 18 reads of the same 91,231,928-byte snapshot.
`expanded_reads` represents that count by repeating one immutable bytes object.
The auditor nevertheless hashed all 18 entries in both its expected and actual
read lists. This costs work between native queries without adding evidence.

`read_sha256` reuses a digest only for consecutive references to the same
immutable bytes object, retaining a separate entry for every read. Its state
lives only within the call and holds one preceding object. Equal but distinct
objects are hashed separately; mutable buffers are always hashed again.
`run_control` still compares the complete byte lists with `expected == actual`,
and retains every existing identity, full-payload, read-count/order and delivery
check. Its receipt schema is unchanged.

On this MacBook Pro, six interleaved component pairs on that fixed snapshot
produced identical eighteen-entry arrays. Median parent CPU time was 0.54897
seconds for the old hash loop and 0.03097 for the helper: 18 SHA invocations
versus one. The companion JSON retains all samples and the snapshot hash.
These are hash-component measurements, not whole-replay, CI-cost or gameplay
improvement claims.

Twenty-seven replay tests and eleven decision-trace tests pass. New regressions
cover repeated-object counts and order, distinct objects with equal content,
mutable buffers, call boundaries and invalid input. Existing independent child
process tests still cover extra/fewer reads, changed versions and reply gates.

The new auditor also passed the original native history142014 through turn
360/frame2: all 996 complete replies and actual read byte sequences match.
Every normalized request receipt is identical to the previously completed
control from the old auditor. Their common canonical receipt digest is
`3a1ac9ba78396f11ed2fc61d49afee5d12605bbbf93f8e7c42741c315bcb0a44`.
The optimized tool source is `ac7274cbe1698d2500c8ca900938ce8e0f6a0184`;
the unchanged native executable source is
`94ae21fb7fc6cf6d0862b29e32ab4f8f08e26338`.
Native raw captures and full control logs remain in the external experiment
folder `civvis-tactics-results/2026-11-04/bankruptcy-native-142014`.
The second slow attempt was interrupted and retained as incomplete; it is not
counted as a successful control. The new completed run supplies that repetition.

No gameplay code, live treatment setting, compiler profile or CI configuration
changes. The faster auditor is now being used for the separate offline bankruptcy
treatment investigation; that investigation establishes no native execution or
strength result from this tool change.
