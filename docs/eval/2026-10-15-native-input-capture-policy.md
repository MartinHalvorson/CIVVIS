# Native decider input capture through verification launchers

The native decider already supports `--capture-inputs`, and `civ6_play`
forwards its `--civvis-capture-inputs` option to that worker. The installed
verification chain exposed neither option. Its host launcher ignored the new
key as unknown, and both runner wrappers omitted the request.

Add `CIVVIS_CAPTURE_DECIDER_INPUTS=0|1`, default zero, to the verified-head
launcher and installer policy keys. The launcher clears inherited values,
validates the policy and exports it. The supervisor rejects invalid direct
environment values and forwards `--capture-decider-inputs` to either the
visual climb or the fixed capture-free runner. Each wrapper translates it
to the existing player option. Saved-game continuations retain the same
request. Input archiving is independent of desktop screenshots.

The existing brain reserves a new archive for each decision-worker process
and records whether the worker reported the capture descriptor. This change
does not strengthen those receipts into proof of archive integrity, executed
orders or native outcomes. It adds no AI treatment and remains off unless
the host explicitly enables it at a launcher boundary.

Seven new controls pass: policy defaults and stale-environment clearing;
accepted and rejected values; both actual supervisor runner commands; visual
CLI and continuation forwarding; capture-free parsing and attach forwarding;
installer seeding; and the player-to-brain option. Existing suites also pass:
14 verified-head launcher, 25 ladder objective, 33 host automation, 18
capture-free runner and 216 visual-climb tests, for 313 tests total.
Shell syntax and whitespace checks pass. No Rust, game rules or AI code
changes, so no local Rust rebuild or engine soak is required for this wiring.

No live host policy was changed during this validation. Actual native archive
creation, byte verification, overhead and replay fidelity remain to be checked
in a separately recorded attempt.
