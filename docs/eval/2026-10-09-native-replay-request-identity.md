# Native replay request identity

The replay's native comparison checked the complete orders and decision, but
did not bind each reply to the request that produced it. A reply with outer
`turn: 99`, inner `decision.turn: 1` and unchanged planning fields could pass
the control for requested turn 1. A candidate naming another decision frame
could also pass because its original arm matched native at the first change.
The CLI returned zero in both cases. Orders without a decision had the same
problem when the original arm's planning payload matched native.

Both arms now need an explicit integer outer turn equal to the requested
turn. A present decision must be an object whose integer turn and frame equal
the requested state frame. Booleans, floats, missing identities and other
frames fail this check. A reply before revealed terrain exists remains valid
when it identifies its turn and has empty orders without a decision; an
explicit null decision does not qualify as that reply.

`native_history_gate_passed` retains the existing native comparison and first
complete-change rule. `request_identity_gate_passed` separately checks every
observed reply in both arms. The overall gate requires both. Invalid replies
are retained in `request_identity_errors`, with their full payload, arm,
request, index and reasons. A later valid reply cannot erase an earlier
identity failure. Native mismatch counts continue to describe the orders and
decision comparison, independently of request identity.

The test-only checkpoint `c0738a782` independently reproduced the failures in
CI run `37871999622`: 3,266 tooling tests ran, with 21 failing new subcases,
two missing-new-diagnostic-field errors and 215 skipped tests. Three CLI
negative controls incorrectly returned zero. The local candidate passes all
28 replay tests and all 11 existing decision-trace tests. The controls retain
valid empty replies, strict JSON comparison, first-change selection, earlier
mismatches, partial-process failures and unchanged-input requirements.

Candidate CI run `37873044143` passes all 3,266 tooling tests with 215 skipped.
The registered original-binary replay through turn 161 also finished as
expected: 438 replies, zero identity errors, 296 native matches, 142 native
mismatches and zero missing native records. Identical arms produce zero
changed replies. Both deciders exit zero, all input hashes remain unchanged,
and the tool exits 2 because the native-history control still fails. The
required full Rust regression is recorded separately in the PR before ship.

Request identity cannot resolve unrecorded input timing or make a mismatching
original replay conform to native. This change modifies verification tooling
only. It changes no native lane, pin, policy, mod, private source or AI
selection, and establishes no native win-rate gain.
