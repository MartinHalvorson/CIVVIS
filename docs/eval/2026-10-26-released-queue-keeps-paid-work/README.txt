Released queues retain their own paid production

The native bridge releases a queue that does not match its recorded production
choice before asking the planner for another build. It previously cleared the
head while leaving its paid production in the general balance. An unrelated
replacement could then spend that work as idle overflow.

Game::pause_city_production now saves the head's invested work under the existing
canonical item-progress key before clearing the queue and active balance. The
bridge uses this API for its existing release operation. Resuming the original
item retrieves its work; another build uses only its own saved work. Existing
owned, deferred and upgraded-production guards still determine which queue is
released. A genuinely empty queue retains its unassigned overflow.

Observed opening

The retrospective case is the second predefined Aqueduct-delivery attempt,
civvis-20261026T021028Z, selected after knowing its two-city turn-60 opening and
Science loss at 218. Its frozen executable hash is
ce132b317cf1acaba786372445a14e0eb23cd9de9787b9a2bc8bae50f00d5c85;
the recorded revision is 0a5e6ae0ce0524bbb0e1be52bea0312c98d835f0.
The revision is a documentation successor of the first Aqueduct binary and the
executable bytes are identical. Its actual 309 forced flags, including the
Slinger trial, are preserved. This is not an independent or paired strength
trial, and other cohort attempts, continuations, stops and retirements remain.

At request 18/0, all 18 captured reads showed the capital building a Settler
with 17 of 55 production paid and seven native turns remaining. The original
reply reports released=1. The exact root trace then records an empty queue
before a Monument order. At 19/0 it again releases the still-running Settler,
now with 23 paid, and the land-recon reservation prices a Scout at 0.0 modeled
turns; the host's actual Scout quote was 15 production and three turns. Both
requests failed to replace the native queue. A Warrior request at 20/0 did
replace it and was verified at 21. The accounting correction keeps that active
Settler work out of unrelated replacement prices. It does not change the
ownership map's provenance policy or impose an expansion/faith priority.

The first Monument completed at 74/1. A third city was founded later in turn
60, so the early turn-60 state showing two cities is not the end-of-turn count.
The Holy Site at 24 was a genuine Prophet reservation; the race closed when
our point total was 29 of 30. Revelation remained unavailable throughout that
race. These observations do not establish that every religious investment or
defensive replacement was wrong.

Qualification and portable evidence

Two original-executable controls each reproduce all 266 complete orders and
decision payloads and all 4,621 ordered captured reads through turn 100.
An exact historical assembly verifies 830 tracked source/data/compile inputs,
including 20 discovered literal includes. Only two files add private root
logging: actions.rs uses an exact root-pointer/frame marker and track_caller;
civvis_orders.rs sets the marker after cloning the real planning board.
Simulated clones do not supply these root production calls.

Two diagnostic controls reproduce the same 266 payloads and 4,621 reads each.
Their entire 397-event traces match: 266 root starts and 131 Produce calls.
The preserved diagnostic executable is
9598bc51f3cff8b0bc3d2e6accb69fad38d16469af27efff1abe58b337964589.
The initial postprocessor expected the wrong log filename after those controls
passed. Its failure is retained; the corrected qualification uses stderr.log.
The original argv and cwd are reconstructed from the archived brain header and
launcher convention, not directly observed process metadata.

native-events.jsonl.gz preserves the exact original event archive.
qualified-opening.json.gz retains the complete controls, original requests,
source manifest, private instrumentation patch, full qualified trace, native
ledger and input contexts. All 278 distinct snapshots are exact byte prefixes
of that archive. Their original delta index and prefix hashes are retained.
restore-inputs.py restores the 266 requests and exact captured input bytes into
a new directory; it does not write any game directory. Restoring and checking
all 278 snapshots and 4,621 reads passed locally. An initial validator used the
first request's read whitelist for unrelated snapshots and was rejected; using
each restored request's actual reads passed without an artifact change.

Validation checkpoint

Four focused regressions use the observed 17-production Settler case. On the
unchanged original implementation, two intended assertions fail and both owned
queue/real idle-overflow controls pass. The same test bytes pass after the
correction, including a Scout replacement followed by resuming the Settler and
a separately funded Monument. All 315 orders-binary tests and cargo check also
pass. A rejected formatter-wrapper compile run is retained before the genuine
assertion RED. Only the new test block was formatted; whole-file Rust syntax
was checked before the genuine retry.

Committed-source Rust quality passes with the changed lines formatted and
warning-free. Main was merged once before review; its only changes were the
upstream Python production-share accounting code and tests. All 16 of those
tests pass. The two corrected Rust files retain the exact bytes validated by
the focused tests, all 315 orders-binary tests, check and quality.

The registered historical comparison is complete. The corrected historical
executable preserves all 266 original replies and all 4,621 ordered reads in
each of two runs. The complete candidate root traces match each other. There
is no changed request through turn 100: this corrects production accounting,
but this captured opening establishes no native action benefit. The full
unchanged comparison, both observed traces and final replies are retained in
first-change.json.gz. The original 830-file historical inputs, qualified root
logging and only this two-file correction were used; no current-main strategy
or configuration changes entered that build.

The Scout estimate in the turn-18/19 journal changes from 0.0 to 2.5 modeled
turns; the native quote was three whole turns. Thus the accounting/price
correction is observed, while the transported choices in this opening remain
the same. No improved completion or game result is inferred from that estimate.

Run: python3 docs/eval/2026-10-26-released-queue-keeps-paid-work/verify.py

No changed native future, completion or strength estimate is claimed. Normal
source delivery and a passive first-eight-segment observation are registered;
failed starts, continuations, missing outcomes, retirements and source/profile
drift remain included. No verification setting, leader, rung, force profile,
lease, pin or game input is changed by this task.
