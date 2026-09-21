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

Validation and the paired replay are in progress. The replay uses a frozen
copy of the completed continuation, beginning with its actual fresh-agent
restart at turn 77. Both sides include the subsequently merged native income
fix and early counter-faith construction fix. Recorded host observations
continue after changed orders; this cannot establish an alternative native
outcome, earlier completed bombers, or a better victory rate.

The original game also lacked Aluminum until much later: turn 187 canceled
the later appointment for lack of Aluminum; turn 197 exported a stock of two.
A Bomber production order was issued at turn 202. Retaining the appointment
alone is not evidence that this resource constraint would be solved.
