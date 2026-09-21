# Preserve an adopted faith before the religious defense loses its supplier

Native run `civvis-20260921T110313Z`, pinned to `480216769`, lost to China's
religious victory at turn 146. It retained ten cities and reported 771
military strength, with no recorded city captures. Economic growth alone did
not deny the other victory condition.

The player founded no religion. Orthodoxy nevertheless remained a possible
counter-faith: Cumaná followed it from turn 81, and other cities adopted it
intermittently through 113. At turn 98 Cumaná could legally build a Holy Site
for 63 production, estimated at four turns, while the empire held 197 Faith.
Instead it queued an Aqueduct. At 102–108 the Holy Site remained legal there
in three or four turns, competing with a Dam. No adopted-faith city offered a
Missionary purchase in the recorded menus. By turn 130 all ten cities followed
Buddhism.

The existing non-founder defense attempts to buy adopted-faith Missionaries
but does not provide their Holy Site and Shrine. Its first-city threat scan
can also call the minority counter-faith the invader. The change selects
an invading faith that holds at least half our cities, prioritizing its hold
on other civilizations, and reserves one legal Holy Site–Shrine chain in a
currently observed counter-faith city. It prefers an existing commitment and
then the shortest production chain. A threatened supplier is excluded, and
local emergency production retains precedence. Purchase remains gated on the
actual city faith and legality; another religion that would complete a rival's
victory is not used as the counter-faith.

This applies to Domination non-founders with religious victory enabled and
Faith sufficient for the undiscounted Missionary estimate. Founders keep the
existing Temple defense. The production governor preserves the one reserved
chain, including its initial zero-progress frame. Other victory lanes retain
their existing policy.

Validation passed: 4,024 Rust tests (53 ignored), including seven focused
regressions; fourteen treatment-append tests; formatting and diff checks; and
eight four-player simulator games, seeds 0–7, limited to 180 turns with four jobs.
The focused regressions exercise threat selection, zero-progress queue protection,
Holy Site → Shrine → adopted-faith Missionary purchase, source eligibility,
non-applicable lanes, unsafe counter-faith rejection, and a single reservation
across two eligible cities.

A matched replay of all 406 recorded frames against the preceding trade-budget
fix adds four immediate Holy Site requests in Cumaná: 98/0, 102/0, 103/0, and
104/0, versus none in the baseline. Neither replay requests a Shrine; the frozen
host never completes the hypothetical district. Sixteen frames have actionable
order differences, eighteen including synthetic receipts, and thirty-three have
internal action differences. Other changes are production, production lookahead,
and one unit's movement. At turn 99 the source has changed religion, so the
reservation releases and ordinary production can replace it. Later repeated
requests encounter the unchanged historical Aqueduct/Dam and synthetic failure
receipts; those are not new native execution failures.

Baseline and candidate replay times were 122.78s and 125.66s, unpaired; required
CI supplies the paired cost gate. The recorded conversion window establishes a
legal opportunity, not proof that a counterfactual Holy Site would have completed
in time or that its Missionaries would have prevented the loss. No native
Domination victory has been verified.
