# Keep air preparations when urgent denial releases the war opening

Native King/Gran Colombia game `civvis-20260921T183544Z`, continued as
`civvis-20260921T183544Z-cont1`, ended on turn 202 with Korea's Technology
victory. These are two segments of one game, not independent trials. The
final export held eleven cities, all originally ours, and no enemy original
capital. Its pinned decision source was `11bce44ed84a5f53bc9979c5da32716a8764495c`.

The continuation's reasoning appointed an air surge against Methone on turn
134 with six technologies remaining. Turn 139 canceled it because “victory
denial superseded the surge.” Turn 149 appointed another three technologies
away and canceled it for the same reason in the same turn. This happens when
the appointed target itself becomes urgent: the opening helper removes the
whole plan to let ordinary war logic act before bombers are ready. Research
and production commitments disappear with that declaration hold.

For an explicit Domination lane without a threatened home city, release the
hold while retaining the existing appointment. Ordinary diplomacy still
requires a legal, affordable war, its peace deadline, and a staged army.
When that path successfully declares the matching war, mark the retained
appointment as declared and enter Exploit. Next-turn maintenance then
recognizes our opening instead of aborting as though the target attacked
first. Failed or held declarations do not create a fictitious war.

If that urgent target opens the war first, the existing counterattack
selector can take over without resetting the investment. This requires one
major front, no threatened home city, and a reachable legal objective. It
records a counterattack, not a declaration we did not make.

Other victory lanes retain the existing cancellation. Ordinary research can
still temporarily prioritize a cheaper standing-army upgrade during a major
war; this does not change the research priority rules. Existing Aluminum,
home recovery, objective ownership, and peace-close checks remain active.

## Evidence and limits

Three new behavior regressions failed on the unchanged production source:
retaining research, preserving the appointment across an actual ordinary
war declaration and next-turn review, and retaining research while a peace
deadline holds the declaration. All failed because the appointment was
removed, not because fixture setup or compilation failed.

Validation on the integrated Rust source:

- `cargo test --profile ci --locked`: 4,108 passed, zero failed, 53 ignored.
- Forty focused air-surge tests pass (one existing census ignored), including
  ordinary declaration, counterattack continuation, peace and treasury holds,
  missing staging, home danger, other lanes, and idempotent declaration tracking.
- Fourteen treatment append tests pass.
- Eight four-player, 180-turn smoke games complete, seeds 368600–368607.
- The integrated native religion exporter test passes under Lua 5.1.

The paired replay uses baseline `31bbac39ec0be2352254956d43a4186412e067a1`
and candidate `c4467de8c`, both with the merged native income and early
counter-faith construction fixes. Both complete all 390 frames of the frozen
continuation, beginning with its actual fresh-agent restart at turn 77.
The event SHA-256 is
`e7312443c8310e9237cdec1213a36fd9853aa26ee38f51f5795ab3da6a641673`.
Binary hashes, complete orders, and reasoning remain in
`/tmp/civvis-air-denial-replay`; comparison is
`/tmp/civvis-3686-comparison.json`.

The candidate changes 152 internal-action frames and 122 exported frames,
including subsequent verification receipts. Four research decisions change:
turn 142 Sanitation becomes the generic Castles choice; turns 147, 152, and
158 request Flight instead of Replaceable Parts, Chemistry, and Siege
Tactics. Flight is first requested thirteen turns before the baseline's
turn 160 request. This also displaces ground/economic research; it is not an
unqualified gain. Repeated Flight requests arise because recorded future
observations continue the old research.

On turn 152 the candidate redirects the existing appointment to Alexandria
on the same Macedonian front, rather than aborting it. That different
objective changes subsequent movement, fortification, and combat orders.
One turn-156/frame-1 peace offer disappears. No production orders change.
The existing Aluminum timeout still ends the appointment on turn 187.

Recorded host observations continue after changed orders. Receipt failures
against that old future do not establish native execution failure, and the
replay cannot establish an alternative outcome, earlier completed bombers,
successful captures, or a better victory rate. Runtime differences are not a
performance comparison because concurrent machine workloads differ.

The original game also lacked Aluminum until much later: turn 187 canceled
the later appointment for lack of Aluminum; turn 197 exported a stock of two.
A Bomber production order was issued at turn 202. Retaining the appointment
alone is not evidence that this resource constraint would be solved.
