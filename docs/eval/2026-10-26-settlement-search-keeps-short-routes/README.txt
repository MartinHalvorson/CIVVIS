Find nearby settlement sites by shortest route in observed Native games

The bounded settlement-site depth-first search can mark a tile visited along a
long route before reaching it along a shorter route. That can make a paid
Settler queue appear useless even when the mirrored Native map contains a
nearby modeled site. The final candidate uses a breadth-first queue only for
MIRRORED_SEAT games with nonempty Game.host_observed. Empty-observation
headless/reference games retain the original LIFO traversal and frozen AI
behavior. The terrain, ownership, water, and walking-distance predicates are
unchanged.

Retrospective case

civvis-20261026T021028Z was an Immortal, four-player Tiny Pangaea game with
Online speed, a Domination assignment, and all victory types enabled. It ended
in a Science loss on turn 218. This is one retrospective case, not a win-rate
estimate.

Two original-binary controls reproduced all 266 complete planning payloads
and 4,621 ordered captured reads through turn 100; both complete root traces
match. At turn 17, frame 3, after founding the second city, the old helper
reports no practical site while a separate diagnostic BFS finds modeled tile
(6,16) four steps away. The capital's queue then replaces a paid Settler with
a Builder while width is two and there is no walking Settler. This false
negative occurs in 537 repeated calls across 92 requests, not 537 independent
experiments.

Two historical candidate replays agree on all 42 complete replies and 747
ordered captured reads before the same first changed reply at 17/3. Their
complete changed replies and root traces also match. The only change is removal
of the capital Builder produce order and its modeled action. The original
Native opening lock rejected that Builder, then Monument and Scout, before
accepting a Warrior on turn 20. Actual Native Settler completion, founding, or
competitive benefit is unproven. A modeled usable site does not establish a
Native-legal plot or founding.

Compatibility and validation

The first published checkpoint c398d7d applied breadth-first traversal to all
controllers. Hosted cargo-test failed the frozen advanced_v1 ledger: 19,305
decisions instead of 19,252. That global version is retained as rejected
history and was not merged. The current source gates shortest-route traversal
on the existing mirrored-seat and host-observation state; it adds no gene,
flag, force, profile, or re-pin.

RED-v5 ran the exact original DFS source and registered fixtures: three
intended Native assertion failures and four controls passed, including the
headless route-order control. The gated source passed all seven focused tests,
the frozen advanced_v1 ledger test, 255 Settler tests (four existing ignores),
48 settlement tests, and cargo check. Changed-line source quality passed.

The first hosted run on source head a754f33 passed cargo-test, published-build,
control-mod, collaboration-policy, rust-quality, and paired-cost. The paired
cost measured five candidate/base pairs at -2.23% CPU time per completed turn,
within the +8% budget. Its repeat on follow-up head 51d5460 measured -0.20%,
inside the +/-1% noise floor, also within budget; neither run supports a
performance improvement claim. The required overwrite guard was canceled
twice during its 15-minute full-history checkout timeout before its script
ran. Its job timeout was raised to 45 minutes and the guard then passed. The
first collaboration-policy run on 51d5460 correctly flagged the newly changed
workflow path as unclaimed; the PR claim was updated and the hosted rerun
passed. See validation.json.gz for run IDs and terminal receipts.

Historical reconstruction caveat: the original process argv/cwd were not
observed. Two original controls qualify the reconstruction. The first
logging-only diagnostic passed once but timed out delivering 5 of 18 reads in
its second control at 53/0; that run is retained as rejected. The same binary
then passed two complete controls after changing only the reply timeout from
120 to 600 seconds. The first candidate wrapper failed to import frozen helper
modules before a request; the unchanged comparator and binary passed when
those modules were supplied on PYTHONPATH. An earlier fixture panicked on an
out-of-map coordinate before reaching its assertions. These rejected runs and
logs remain preserved; no predicates were relaxed.

Evidence

  diagnostic.json.gz: exact source scope, logging instrumentation, both
    complete qualified diagnostic replies, root traces, and retained timeout.
  first-change.json.gz: earlier global-candidate comparison, now rejected by
    the frozen headless ledger guard.
  gated-first-change.json.gz: host-gated historical candidate comparison.
  validation.json.gz: RED/GREEN, frozen-ledger and related tests, source
    quality, hosted CI, and integration receipts.
  verify.py: validates the retained evidence and final validation packet.
  native-observer.py: prepared read-only first-eight delivery/outcome observer.
  native-metrics-registration.json: fixed descriptive economy readbacks.

The eight earlier queue-correction Native attempts stopped at the configured
production-share threshold in turns 52–54 and have no win/loss outcomes. The
future observer starts only after this change is merged and delivered to the
frozen Native binary. Its cohort keeps stops, retirements, missing outcomes,
source changes, and profile changes without replacement. Results are
observational and may overlap earlier cohorts; no causal or win-rate claim is
made. No game setup, strategy, lane, or live-game input was changed for this
case.
