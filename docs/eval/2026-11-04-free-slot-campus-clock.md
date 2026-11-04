# A speed-scaled budget for the free-slot Campus claim

`free-slot-campus-clock` is opt-in and default OFF. It scales only the existing
`campus-claims-the-free-slot` / `-2` claim's first-Campus time ceiling by
`current_speed.cost_percent() / Online.cost_percent()`. The budgets are 15,
20.1, 30, 45 and 90 turns on Online, Quick, Standard, Epic and Marathon.
Native production quotes already include speed; the quote is never scaled again.

The treatment requires either parent claim. It changes no population floor,
site legality, earlier claim, threatened/recently attacked city check, protected
queue, one-turn finish, Settler reservation, rescoring protection or final
emergency-defense arbitration. Other BasicAi first-Campus callers, Theater
Squares and Holy Sites retain their existing 15-turn ceiling. Online and the
flag's OFF arm retain the same budget. This is a narrow experiment, distinct
from the economy-clock experiment in #4383.

## Development motivation

A fixed census of 34 completed-summary native attempts on November 4 retained
partial, retired and censored cases; 28 reached turn 100. Median Science divided
by the best **observed** rival's Science was 0.1335, median population ratio
0.4508 and median Science-per-citizen ratio 0.3100; the medians do not multiply.
Median completed owned Campuses at 100 was zero. Captured Campuses count as owned.
Revisions and backgrounds differ: these are descriptive readings, not a causal
estimate or a world-leader census.

The raw offer census found 652 undamaged city-turns at peace with majors, before
any owned Campus, with population at least two, an offered Campus site and a
host quote in (15,30]. That broad filter does **not** establish policy eligibility:
Walls, builders, other claims and defense still hold many queues. The archived
motivation readings preserve their scope. A quote excluded by the shared picker
also produces no feasible-Campus hold diagnostic, motivating a direct experiment.

## Qualified native proposal change

Production and regression source: `9a644ec1f38179d36a588701b3eb3c3d4d543e31`.
Candidate SHA-256:
`4e52069f7ed62a41ccaaa6f574caad0afab7e68f83162b94ffb0cea207e8d8d7`.
It uses the optimized local `ci` profile with explicit `CARGO_PROFILE_CI_LTO=off`
and four build jobs. No repository profile or CI configuration changes. Exact
OFF controls qualify this particular binary; no compiler speedup is claimed.

On native `civvis-20261104T140152Z` (Immortal, Gran Colombia, Standard,
Continents, Small, six players), both original-executable and candidate-OFF
controls repeat all 290 requests through turn 120/frame 2, matching complete
replies and actual input-read bytes/count/order.

Both ON prefixes first change orders at **turn 91/frame 0, request 212**:
Caracas, city 262147, receives `DISTRICT_CAMPUS` at (62,18) instead of
`UNIT_CATAPULT`. Every other order retains its exact sequence. The raw city was
idle, population two, undamaged, with no walls. Its host Campus offer was 126
production, 26 turns and zero saved progress; Catapult was 120 production and
24 turns. This does not interrupt a paid Catapult queue. Khmer war was open:
the existing parent allows an idle city it does not deem threatened to claim a
Campus during war; its military-at-war hold protects an existing military head.

Two successful persistent OFF/ON pairs each consume the same 212-request
history and exit zero on actual EOF. Their capture argument is genuinely the
same across arms; captures move only after exit, with raw path maps retained.
The independent verifier reconstructs all source and actual read sequences,
checks native receipt and production/regression hashes, verifies the sole
argument/genome bit change, and requires an actual per-city Campus substitution
with every other order's sequence unchanged.

## Additional history retained, strict isolation rejected

An earlier development history, `civvis-20261104T134411Z`, was selected for
peace-time Monument queues with longer Campus quotes. Original and candidate
OFF controls each repeat 315 requests through 120/2. Both ON prefixes first
change at 119/0, request 310: idle, undamaged, three-population Panamá selects
a 20-turn Campus over a newly proposed 11-turn Government Plaza, and its Builder
orders also differ. Four persistent prefix queries exit zero with matching
reads/replies. The unchanged-other-orders assertion **fails**, so this source
is not used as the isolated firing artifact. Its complete results and rejection
remain in the archive. No thresholds or holds were relaxed to obtain either result.

## Validation and limits

Five focused regressions pass: all speed ceilings and Online no-op; native quote
selection and unchanged BasicAi picker; parent dependency and eligibility holds;
protected queues versus a routine queue and rescoring retention; modeled turns
without host quotes. The full local suite on source9a passes 6,836 tests with
53 tests and four doctests ignored, using the documented local LTO override.
Required GitHub CI validates the repository profile separately.

The archive contains the frozen intentions, raw replies and input captures,
original and candidate controls, paired query receipts, source exports, verifier,
raw city offers, build/test receipts, motivation census and prospective screen plan.
Run its `verify.py` against the extracted root with the checkout available as
explained in `README.txt`. The second history's strict verifier intentionally
fails; `strict-verification.json` records why.

These are selected development histories, not untouched holdouts or continued
counterfactual games. No replay order was executed by Civ VI. No earlier Campus
completion, Science gain, saved city or win-rate improvement is established.
The treatment stays **OFF and unarmed**. A separate native screen must verify
execution and measure completed Campuses and Science per citizen at 150, with
capital survival, army output/losses and expansion tracked alongside them.
A turn100 endpoint can precede completion of both known proposed Campuses.
