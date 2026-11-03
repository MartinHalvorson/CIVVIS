# Native Culture first-building audit

This frozen cohort contains 13 summarized Culture attempts on 2026-11-02,
across leaders and revisions: 12 have terminal host outcomes; one Emperor
attempt (`155307Z`) was retired at t122 by the production-share rule. It is
not counted as a terminal loss. The JSON retains each outcome and revision,
plus SHA-256 fingerprints of the summary and exact event bytes read.
No pooled win rate or causal treatment effect is inferred.

The tool retains its original Pericles/latest-frame defaults. This audit
explicitly uses all leaders and the lowest observed frame of each turn,
keeping the first observation on a tie. Missing milestones remain unknown.
The data sources are the frozen run list in the companion JSON; later games
are not silently appended to the cohort.

At t100, 12 attempts had a completed Theater Square/Acropolis and 11 of those
had no Amphitheater. There were 42 native Amphitheater build offers across
the cities; one was already the queue head. The other heads included six
Settlers, three Aqueducts and two Walls. Seven had no head at the initial
frame, which can be an ordinary pending production choice rather than a
city left idle for a turn. These distinctions rule out treating all offers
as freely replaceable production.

For the 12 attempts with both milestones, the interval between the first
observed completed Theater and first owned Amphitheater spans 21–78 turns
(median 52.5). This includes changing queues and investments: it is not a
measurement of turns a new policy saves. Initial-frame observations of an
Amphitheater queue can be later than the first owned building, or absent;
they do not establish that building's construction history or payment.
Great Works may also precede an Amphitheater. Their source is not inferred.

All turn numbers below are first observations. Run suffixes share the
`civvis-20261102T` prefix. A dash means unobserved, not zero.

| Run (UTC) | Difficulty | Completed Theater | Amphitheater head | Owned Amphitheater | Great Work | Foreign tourist |
|---|---|---:|---:|---:|---:|---:|
| 071430Z | King | 66 | 113 | 116 | 115 | 148 |
| 075400Z | King | 84 | 134 | 140 | 134 | 194 |
| 084807Z | King | 64 | 106 | 111 | 84 | 133 |
| 092756Z | King | 57 | 134 | 135 | 137 | 168 |
| 120037Z | King | 65 | 120 | 114 | 115 | 129 |
| 123721Z | King | 77 | 140 | 149 | 140 | 159 |
| 133053Z | King | 68 | 116 | 123 | 51 | 149 |
| 140434Z | King | 59 | — | 115 | 116 | 138 |
| 151945Z | King | 72 | 113 | 112 | 113 | 145 |
| 155307Z | Emperor | — | — | — | — | — |
| 160202Z | Emperor | 69 | 86 | 90 | 86 | 140 |
| 180614Z | Emperor | 71 | 142 | 139 | 156 | 175 |
| 182121Z | Emperor | 77 | 102 | 108 | 101 | 158 |

The two latest Emperor Culture losses (`180614Z` and `182121Z`) ended in
rival Science victories at t216 and t203. At t100 they held two/six completed
Theaters and no Amphitheaters; first owned Amphitheaters appear at t139/t108,
and first foreign tourists at t175/t158. Earlier research or more district
placement alone does not fill the cultural building chain.

The next bounded hypothesis is to secure the Culture seat's first
Amphitheater in an existing active Theater, preserving survival, imminent
completion and essential expansion/growth queues. The current district claim
does not itself claim its building. The existing culture-building catch-up
family is conditional on rival yield deficits and refuses Recovery/war-plan
states; it is not an unconditional first-building reservation. A change must
be checked against existing priorities, and same-input proposal comparisons
must precede native execution and disjoint outcome validation. This audit
changes no policy and makes no default-promotion recommendation.

Six focused report tests pass. They cover explicit leader selection,
first/latest-frame separation, incomplete districts, native offers and
unknown estimates, source hashes, terminal outcomes, and missing telemetry.
