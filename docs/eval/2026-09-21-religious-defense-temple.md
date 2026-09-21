# Preserve the first defensive Temple

Native run `civvis-20260921T082117Z` lost to Australia's Religious victory
on turn 162, running `4263512ed`. Gran Colombia had founded Buddhism and
unlocked Theology on turn 57, but no city ever completed a Temple. The final
nine cities all followed Australia's Taoism. The army included three
Bombards; military modernization alone was not sufficient to avert this loss.

The production journal records thirteen switches away from Temple orders,
all priced at -10,000: turns 110, 115, 119, 139, 145, 146, 147, 149, 150,
156, 157, 158, and 160. Replacements include Settlers, Builders, a Spy, a
Market, repairs, a Bombard, and Commercial Hub projects. The existing
`founder_temple` routine reserved the defensive building, then the strategic
governor rejected it because its Relic slot satisfied the generic
non-Culture Great Work veto. Missionary purchases were of our own religion;
this was not a wrong-religion purchase defect.

## Change

A Domination founder facing domestic conversion pressure can preserve one
first Temple order in a city that still follows its own religion. Its Relic
slot no longer triggers the Culture-building veto. A legal reservation
survives ordinary governor rescoring even before any production is invested.
Other queues see the reserved Temple and do not gain the same exception.

This keeps the existing founder-Temple policy effective without enabling the
broader district-based Great Work-veto experiment. Unrelated cultural
buildings, non-founders, other victory lanes, safe religions, converted
purchase cities, and further Temples retain their previous scores. Local
military emergency handling precedes this commitment. The change does not
alter religious unit purchasing priorities or promise an Inquisition or a
prevented defeat.

The earlier siege evaluation's provisional income discrepancy is also
corrected: it compared different frames. At 131/0 the host and planner both
reported a deficit; the positive host reading came later, at 132/1.

## Validation

A historical replay on the original base stopped
at 95/1 in Great Person production planning; a process sample identified the
same loop fixed by #3644. Both comparison builds therefore use `fc0a59fcb`
as the common base, and the entire frozen event stream is replayed from the
beginning. The source game's result remains a loss; proposed replay orders
are not executed builds or a counterfactual victory.

The matched replay completes all 457 decision frames: baseline 108.55 seconds,
candidate 125.05 seconds. Immediate Temple orders rise from 0 to 19, first
at 99/0 in Bogotá; there are also three next-item proposals. Temple veto
cancellations fall from 13 to 4. The remaining four correctly concern cities
without our religious majority: Caracas 139/0 and Panamá 145/0 follow Taoism;
Bogotá at 158/0 and 160/0 has no majority. Those cities cannot currently buy
our religious defenders. There are 59 changed exported frames and 84 changed
internal action frames, including production substitutions, two movement
orders, and policy-deck timing. Repeated requests on a frozen board do not
represent nineteen completed Temples.

All five focused regressions pass within the full Rust suite: 3,786 library
and 206 other tests pass; 49 library and four doc tests are ignored. Tests
exercise the Relic-slot veto, the real founder-reservation/governor sequence
with zero invested production across Expansion/Recovery/Conquest, exclusion
of duplicate queues, completed-Temple and cultural-building controls, and
non-founder/other-lane/safe/converted/disabled cases. Formatting, whitespace
checks, and all 14 treatment append-point checks pass. Eight simulator soak
games complete (four players, seeds 364500–364507, 180 turns, four workers);
this is a stability check, not a native victory-rate measurement.
