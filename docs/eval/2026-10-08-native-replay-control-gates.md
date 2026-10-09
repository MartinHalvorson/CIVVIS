# Check the original native replay before interpreting its changes

The observed Bomber rebase cycle in Emperor Gran Colombia run
`civvis-20261008T221617Z` led to an identity replay using its exact snapshot
binary, SHA-256
`10f8dda1519989d8dc41a6b771048605c7ba920d72f4f61d57afa853ff41d1df`.
Two independent copies of that binary replayed the same history through turn
161. All 438 complete replies agreed between the replay arms. Only 296 agreed
with the recorded native orders and complete decision. Duplicate agreement
therefore establishes repeatability of this replay, not native conformance.

The original complete decision at the first investigated rebase, 153f0,
matched native. Its complete orders did not: verification entries differed.
That comparison failed its registered gate. Later matching frames were
retained as observations and did not replace the first investigated frame.
The first native mismatch was 1f0, where replay had no revealed board. Input
timing remains a limit of replaying one event prefix at each state marker.

## Automated control

`tools/native_city_route_replay.py` now reads the run's `decisions.jsonl`, or
an explicit `--recorded-decisions` file, and compares the original replay's
complete orders and decision at every explicit turn/frame. It retains all
native mismatches, the first mismatch's payloads, and both replay arms.
Duplicate native frame keys are ambiguous and rejected. Unkeyed records and
missing frame coverage are reported instead of assigning a frame number.
Comparisons preserve JSON scalar types using the existing decision-trace
comparator: a Boolean is not a number, and an integer is not a float.
Non-finite reply values fail validation.

For differing arms, the first changed **complete reply** selects the native
comparison gate. A telemetry-only or decision-only difference counts; the
tool does not substitute a later action-changing or matching frame. A passing
gate licenses only the original reply at that first change. Earlier native
mismatches remain in the report, so this is not a claim that the entire
history reproduced native.

For identical arms, every observed original frame must match native. A
failing control exits 2 after writing `native-control.json` and
`provenance.json`. The final validation status also requires successful
decider exits and unchanged before/after hashes of events, native replies,
policies and binaries. Neither gate estimates survival, victories or win rate.

## Validation evidence

Nineteen focused tests include persistent-decider CLI fixtures. They cover
identical replay arms that differ from native, first telemetry and decision
changes, whole-decision comparison, retained earlier mismatches, missing or
ambiguous native records, a changed input, a partial replay after a decider
exits, and a turn-limited prefix. Additional controls reproduce Boolean/number
equality falsely passing native agreement, a type-only first change being
replaced by a later native match, and non-finite replies being accepted. Those
three controls failed before strict comparison (six failing subcases); all
19 tests pass after it. They run through the existing tools unittest discovery
in the collaboration workflow.

The new tool also replayed the frozen native witness with the exact original
binary in both arms, the same 224 requested treatments and `--explain`.
Both processes exited 0. The tool correctly exited 2: 296 matches, 142 native
mismatches, zero missing native records, zero changed complete replies, and
every input hash unchanged. This reproduces the manual control failure with
an executable gate.

The frozen evidence, commands, complete replies and failed-control diagnostics
are under
`~/civvis-tactics-results/2026-10-08/public-campus-finish-before-defender/native-baseline-fb5ac/`.
The native event file SHA-256 is
`bc8494a2a5089396554a924978bc083cdf93f868ded04913299c9e586b0e74b9`.
Additional repository validation is recorded in the PR.

The implicated pad-rebase selector belongs to the separately pinned private
source and is absent from public main. This change makes its replay control
failure explicit; it does not fix that selector or change the live native
pin, mod, retirement policy or genome. No candidate native game or win-rate
gain is credited.
