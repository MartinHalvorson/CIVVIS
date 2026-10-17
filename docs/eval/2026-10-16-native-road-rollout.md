# Native siege-road memory rollout and failed replay control

## Status and scope

The persistent replay control **failed**. The registered A/B comparison is
therefore **not run**. Three consecutive, preselected Native attempts are
being observed independently; they are not a paired strength experiment.
No highest-difficulty win, conquest win, speed improvement, or causal
strength improvement is established.

This follows the city/unit identity-memory repair in PR #4084, revision
`7f5232e8a657478225a9d8b81bb62d2d486d6f7a`. This task changes evaluation
documentation only, not production code, treatment selection, Native policy,
difficulty, executable pins, or the verifier. The unattended Native seat
continues normally; no game is restarted for this study.

## Registered selection and gates

The adjacent registration JSON was written at **2026-10-17 04:50:22 UTC**
(October 16, 21:50:22 PDT), before the first selected attempt began at
04:50:45 UTC. It fixes:

- Every original state frame, in file order, from the beginning of
  `civvis-20261017T025117Z` through turn 150; no alternate world or suffix.
- An A/A control using the same frozen `53ee80a45` executable in both arms,
  its actual SHA-256 checked, and the original selected 271 forced treatments.
- A strict full-history gate: every complete orders/decision reply must match
  the original Native record, both children must exit zero, identities must
  match, and inputs must remain unchanged. Self-agreement alone is insufficient.
- A/B only if that gate passes. Failure is retained rather than tuning the
  input prefix, selecting another world, or silently restarting the study.
- The first three newly created normal Native run directories absent from
  the registration's existing-ID snapshot, ordered by run ID, regardless of
  source, settings, errors, termination, or result. No replacements.

## Persistent A/A result

The existing `tools/native_city_route_replay.py` harness processed **411**
frames in 25.281 seconds. Both children exited zero, all checked inputs were
unchanged, and all request identities were valid. The two identical arms
agreed on every complete reply and order. Nevertheless, only **328 of 411**
frames matched the recorded Native decisions: **83 mismatched**. The harness
history gate and the stronger registered gate both failed.

The earliest mismatch is index 0, turn 1, frame 0. Native recorded five
orders, including research, a warrior fortification, founding the capital,
and two policies. The replay instead returned no orders and the note
`no revealed terrain or no state yet`.

In the immutable original event stream, the first `state` occurs at line 67
(byte range 15074–22771), before the first `tiles` at line 70
(23330–28623) and `tiles_done` at line 71 (28623–28799). The replay harness
cuts a request prefix immediately after its state event. Thus its first
request lacks the terrain that the recorded Native decision used. The
original run does not retain an exact per-request input-read capture that
would resolve the actual Native read boundary.

This explains the first mismatch, **not necessarily all 83**. Do not treat
the failed control as a counterfactual policy result. A future fidelity
study needs independently specified request-read capture; this study does
not repair or tune the replay harness after seeing its outcome.

## Native observation checkpoint

The first selected run, `civvis-20261017T045045Z`, is terminal. Its frozen
executable reports revision `7f5232e8a`; its actual SHA-256 matches its
provenance. Configured readback is true, Emperor, Gathering Storm, Tiny
Pangaea, Online, Gran Colombia, Domination target.

It is **verifier-abandoned at turn 150**, production rank 4 where the
unchanged policy allows at most rank 2. Its audited summary records zero
cities taken, four lost, 29 kills and 37 losses. The last own-city roster
contains no observed original capital. No victory or defeat event is
recorded; abandonment is not a completed game loss. Its journal has zero
`Standing down a siege the army cannot reach` lines, so this attempt does
not demonstrate runtime firing of the repaired memory path.

The second selected run is `civvis-20261017T045855Z` and is in progress.
The third has not yet started. Their eventual outcomes remain in the
registered cohort, including errors or unverified results. A final summary
alone does not make an attempt terminal while its actual game/brain/decider
process still runs.

## Artifacts and validation

The local evidence root is
`~/civvis-tactics-results/2026-10-16/native-road-rollout/`.
`registration.json` is the original registration; `aa/` retains both complete
decision streams and journals plus `provenance.json` and
`native-control.json`. Original inputs are hash-frozen under the preceding
`native-muster-approach/frozen-original-inputs/` study. Completed cohort
attempts are copied into `native/<run>/` with before/source-after/copy hashes
equal; the raw streams are not committed to Git.

The observer was hardened before any terminal cohort result to check actual
executable hashes and explicit local/team victory identity. An ambiguous
observer-only stop selector aborted without signaling; its shell continued
and briefly launched a second read-only observer. Both owned Python readers
were then stopped, with no terminal results yet, and one hardened reader
resumed. Native processes and selection rules were unchanged. Observer
versions and the restart receipt are retained; duplicate interim observer
receipts are not independent attempts.

`python3 -m unittest discover -s tools -p test_native_city_route_replay.py`
passes all **28** tests. That tests the harness implementation, not the
failed original-history fidelity assumption. Rust is unchanged; the full
engine suite and soak are not rerun for this documentation-only study.
