# Ordered native input control

## Registration

This is an instrument qualification for conquest-policy investigation, with
no policy comparison or strength estimate. The earlier handover one-prefix
control stays rejected. It cannot reproduce requests that read different
event snapshots within one decision.

The attempt to retain live process context for `civvis-20261025T055158Z`
found that its decider had exited. Keep that missing-context receipt; do not
invent its arguments or working directory. Before inspecting the next game's
outcome or policy effects, fix the first subsequent frozen attempt,
`civvis-20261025T060426Z`, as this separate qualification case. Do not replace
it if setup, capture, context collection, or a control fails. Native screen
ownership and gameplay settings remain with their existing owners.

Retain its actual executable, process arguments, working directory, frozen
provenance, and all chronological decisions through turn 70. Require complete
capture descriptors, unique request identities, and all referenced bytes.
The replay uses the original immutable executable and its observed arguments,
changing only the mirror and capture destinations to exclusive temporary
directories. It never publishes orders to the game.

Investigate a POSIX FIFO adapter that serves the recorded text separately on
each actual file open. Replace the pathname with the next FIFO before closing
the current writer, so the next open cannot consume bytes from the preceding
read. The mirror's cache may take a different internal path on a FIFO; exact
complete outputs and recaptured read bytes must therefore establish behavior,
never an assumption about cache equivalence.

First validate the adapter with independent subprocess consumers, including
changed text, repeated reads, too few reads, too many reads, partial writes,
and early process exit. Then run two chronological original-binary controls.
Every request must match the complete recorded native orders and decision,
the exact turn/frame, and the expanded ordered input byte sequence. Preserve
all replies and discrepancies. Any mismatch rejects the control; never
substitute a later frame or relax a field. A passing control establishes
reproduction of this observed history only, not counterfactual execution,
policy activation, safety, survival, or wins.

## Results

The fixed case's actual native process context is retained before its turn-70
cutoff: PID 60548, working directory `civvis-verification-clean-runtime`, frozen
source `a605bd591cab1ab6d294691f412e0d5e42e8ded4`, and binary SHA-256
`cf351ea77cdf0b10d5f5d6ecbd322006c8d1f2f96f5b299f20ee7a01ac02df77`.
Its actual 614 argument words include 301 `--with` options and its existing
`unwalled-ring-steps-in` screen. This is an original-binary instrument control
of that played configuration, independent of the queued material trial.

The first 13-test adapter run has 11 passes and two failures. One test assumes
`/tmp` is the canonical path, while macOS resolves it to `/private/tmp`; its
expected destination is corrected. The incomplete-consumer test also exposes
an implementation diagnostic bug: cancelling its writer overwrites the useful
one-of-two read failure. Preserve the pre-cancellation error before cleanup.
These are development test failures, never a policy RED or native result.

After correction, all 13 adapter tests pass. The existing replay tests pass
36/36 and the capture/decision tests pass 11/11.

The first original-binary control at tool source `581eaffe5` matches every
complete native payload and expanded read sequence for 62 requests, through
turn 23/frame 3. It times out at turn 24/frame 0 under the registered
30-second request budget. That request records 18 reads of snapshot 63.
The first control is rejected, and a second control is not run. No treatment
is evaluated. A FIFO changes filesystem metadata and the mirror's cache path;
matching the earlier requests does not establish equivalence through turn 70.

The timeout receipt lacks the writer's served-read count. Add that diagnostic
before a separate full-history diagnostic repetition on the same fixed case,
with the same original binary, ordered bytes, cutoff, and timeout. Preserve
the initial failure and both source versions. Do not extend the timeout,
append guessed reads, or select a later starting frame. This diagnostic is
not permission to reinterpret the rejected control as a success.

The diagnostic source `1a874f324` also fails, earlier at turn 19/frame 1
after 49 matching requests. Its writer reports 15 of 17 expected reads served
before the unchanged deadline. Both native replies and the writer stop before
the complete request can be checked. The handshake's failure point is not
stable; its cause remains unresolved. Keep this tool in draft, with both
failed complete-history attempts retained. Do not infer that increasing the
budget, repeating until a pass, or supplying extra reads would qualify it.

The unchanged Rust/Cargo source tree equals merged `a605bd591` in both this
worktree and the reserve worktree. A fresh required
`cargo test --profile ci --locked` invocation in that existing compiled
reserve worktree passes 6,255 tests, with 56 ignored. Its complete raw output
is retained alongside the 60 focused Python test results. This reruns the
actual identical Rust sources without rebuilding an identical task copy.

The compressed controls archive preserves the selected chronological native
decisions, the necessary original archive/index prefixes, complete emitted
replies, recaptured bytes, journals, actual context, both tool sources, and
validation logs. Each entry carries its byte length and SHA-256. The original
25 MB native executable remains an external input identified by its frozen
hash; the archive is not a standalone executable distribution. No native
screen, game, policy, deployment default, or source binary was modified.

Archive: `2026-10-25-captured-read-replay.controls.json.gz`, 5,497,657 bytes,
32 entries, SHA-256
`a6ea80e709833d06476fbba635c0cf0d6744e98731b5eeea6953d8cdb9ca1150`.
Its fixed case contains all 186 native requests through turn 70. Every archived
entry's length and digest verifies after decompression.

## Read-handshake diagnostic registration

An independent subprocess consumes 1,600 consecutive opens, each with
1,260,000 bytes, successfully in 43.59 seconds. This tests the primitive only.
The host's CPU headroom service explicitly exempts `civvis_orders` and
`--serve`/`--mirror` processes; it is not changed for this investigation.

Before any further native execution, add the writer's current open/write
stage, read number and sent-byte count to failure receipts. Register one more
full-history diagnostic on the same fixed original binary and case, again
starting at the first request, with the same turn-70 cutoff, expected bytes,
and 30-second deadline. A separate read-only sampler may record this
diagnostic child while a request is pending, without changing its input or
deadline. This is failure localization, not a new policy or a repeated search
for a passing control. Both earlier failures remain rejected; no treatment
may be interpreted from this diagnostic.

The phase diagnostic happens to complete both 186-request controls with exact
native payload and ordered-byte matches. The sampler finds no stalled request.
This does not resolve either earlier failure or qualify policy attribution:
phase counters do not repair an input protocol. Investigate the writer's
missing consumption acknowledgement separately. A successful pipe write can
leave bytes queued while the reader has yet to consume them. Register a
subprocess test that opens the reader, holds it before reading, and requires
the writer to remain connected until those bytes are consumed. Run it first
against the unchanged writer; then require a real protocol fix before further
native qualification, retaining all three earlier attempts.

The paused-reader test fails against the unchanged writer: the writer finishes
before its queued byte is consumed. The other 13 tests pass. Fix this concrete
delivery contract by retaining the writer until a separate observation-only
read descriptor reports an empty receive queue. Open that descriptor only
after the actual consumer has connected; never read bytes from it. Close it
before closing the writer. On macOS the FIFO ioctl routes by descriptor access
mode, so querying the write descriptor would inspect the wrong socket; see
[Apple's FIFO implementation](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/miscfs/fifofs/fifo_vnops.c).

Empty recorded reads cannot provide this acknowledgement and are explicitly
unsupported. The observer descriptor can delay a broken-pipe error after an
early consumer exit; the outer process/reply check still rejects that exit,
and the primitive's incomplete-delivery test requires rejection without
claiming a specific errno. This fix is motivated by the failed delivery
contract. It does not establish the cause of the earlier native stalls.

After the paused-reader and existing subprocess tests pass, register a new
corrected-source qualification of the same complete 186-request history,
with both original-binary controls, unchanged exact payload/read checks,
turn-70 cutoff and 30-second deadline. Earlier failures and the lucky phase
diagnostic remain separate. Interpret no policy or strength result.

## Corrected qualification

The corrected writer at `0b018690913d1ab9f49204670d9e4f86ff55cc49`
passes all 15 subprocess/evidence tests. The paused-reader test now holds the
writer until consumption, while incomplete reads, extra opens, early process
exit, truncated archives and partial replies still fail. Empty inputs are
explicitly rejected.

Both corrected original-binary controls pass all 186 chronological native
requests through turn 70. Each control checks 3,231 logical reads, including
four requests with multiple different captured snapshots. Every request has
the exact recorded turn/frame, complete native orders and decision, and
expanded ordered input bytes. The process exits zero after 147.15 seconds;
the tool's SHA-256 remains
`f0cb7f2b84173dca2115514b35e12a55acea7cffe303c3d0ba22f41d6c628e7e`
throughout the run. The request deadline remains 30 seconds.

This qualifies reproduction of this fixed observed history with this binary
and recorded command. It neither erases the earlier failures nor establishes
their exact cause. It does not establish general reliability, another
platform's native equivalence, any candidate's baseline equivalence, treatment
activation, host execution, survival or victory. The handover experiment and
queued material trial retain their independent registrations and gates.

The qualification archive is
`2026-10-25-captured-read-replay.qualification.json.gz`: 2,939,798 bytes,
39 named entries sharing 24 content-addressed blobs, SHA-256
`c7b9df3b8e16c94191a3607af4facf21330d0f3ce966e58624846cac3f571930`.
Schema 2 maps each entry's digest to its exact base64 bytes; every length and
digest verifies after decompression. It retains the successful phase
diagnostic, corrected controls, 13/14 paused-reader failure, 15/15 corrected
tests, primitive stress, source versions and zero-exit/source-seal receipt.
The original rejected controls remain in the earlier independent archive.
