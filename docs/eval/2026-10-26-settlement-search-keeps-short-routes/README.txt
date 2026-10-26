Find nearby settlement sites by their shortest route

Historical case: civvis-20261026T021028Z, Immortal / Gran Colombia / four players /
Tiny Pangaea / Online / Domination assignment, all victory types enabled.
The recorded result was a Science loss on turn218. This is a retrospective case.

The original bounded depth-first search marks each tile visited on its first
route. An eight-step first visit can prevent exploring a shorter route through
that tile. The correction uses one breadth-first queue seeded by every city,
retaining the same validity, terrain, water and distance predicates.

Two private logging-only controls reproduce all266 complete planning payloads
and4621 ordered captured reads through turn100. Both entire root traces match.
On17/3, after city two is founded, the original helper reports no practical site;
the separate diagnostic BFS finds modeled tile(6,16) four steps from an own city.
The original queue revalidation then replaces the capital's paid Settler with a
Builder despite width2 and no walking Settler. Native events confirm that the
opening lock rejects that Builder, then Monument and Scout, before accepting a
Warrior on20 with immediate_threat=false. The diagnostic never changes a choice.
A modeled usable site is not an observed native founding or a proven completion.

The false negative appears in537 repeated calls across92 recorded requests;
these are repeated observations within one game, not independent experiments.
The first diagnostic attempt passed266 requests once, then timed out at53/0
while delivering5of18 reads in its second control. It is retained. The same
binary passed two fresh complete controls after changing only the harness reply
timeout from120 to600 seconds; no comparison predicate was relaxed.

Original argv/cwd were reconstructed from the archived header and launcher.
The actual historical process argv/cwd were not observed. Two original native
binary controls qualify the reconstruction before these diagnostic controls.

Files
  diagnostic.json.gz: source scope, instrumentation, all diagnostic controls,
    the timeout rejection, exact root trace, native queue events and registration.
  validation.json.gz: original assertion RED, same-test GREEN and source quality
    (required before final delivery; currently pending).
  first-change.json.gz: two registered complete-payload/read-prefix comparisons,
    including both complete observed reply streams and root traces.
  verify.py: verify the evidence, with --baseline-only during preparation.

The shared native events, original controls and captured-input restorer are in
../2026-10-26-released-queue-keeps-paid-work/ at published revision b3d9803.
Their exact hashes are in diagnostic.json.gz. There is no duplicate native game.
No strategy flag, setup, target, lane or running game was changed for this case.
Two historical candidate comparisons preserve all42 earlier complete replies
and all747 ordered native reads through the first changed reply,17/3. Both
complete changed replies and root traces match. The only difference is removal
of the capital Builder produce order and its matching modeled action. The real
game originally rejected that Builder; actual native Settler completion and a
competitive benefit remain unproven. The comparison stops at this first change.

The candidate build succeeded. Its first comparison wrapper failed to import
frozen helper modules before any request; that rejection is retained. Supplying
the same frozen modules on PYTHONPATH allowed the same binary and unchanged
comparison predicates to run. No rebuild or changed historical game was used.

The original production code fails the three intended corrected-fixture tests
while three controls pass. An earlier fixture used an out-of-map coordinate and
panicked before assertions; that run is rejected and retained. The exact
corrected test bytes are being checked against the production correction.
Native-metrics-registration.json fixes early economy readbacks for the future
first-eight cohort; production-share stops remain missing game outcomes.
