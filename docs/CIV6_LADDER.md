# The Civilization VI difficulty ladder

What the controller in `tools/civ6_control` has actually beaten, and when.
A rung is claimed only by a victory event naming the controller's own team,
in a game whose settings marker proves it was the game the run configured.
`tools/civ6_ladder.py` writes this file; do not edit it by hand — a
test regenerates it from `docs/civ6_ladder.json` and fails if they differ.

`victory` is Civilization VI's own victory identifier as the
`TeamVictory` event reported it, kept raw on purpose: a guessed name is
how an unfireable type literal hides (see `.github/workflows/tests.yml`).
`type` beside it is not a guess either — it is the row the index names in
the host's own `GameInfo.Victories()`, exported by the agent mod as
`seat.victory_types` and joined by `tools/civ6_ladder.py`. A run recorded
before that export carries the index alone and reads `—` here.

| rung | difficulty | beaten (UTC) | victory | type | turns | run |
|---|---|---|---|---|---|---|
| 1 | Settler | 2026-08-16T06:49:58Z | 0 | — | 250 | `civvis-20260816T054344Z` |
| 2 | Chieftain | 2026-08-18T18:46:46Z | 3 | VICTORY_CULTURE | 250 | `civvis-20260818T175125Z` |
| 3 | Warlord | — | | | | |
| 4 | Prince | — | | | | |
| 5 | King | — | | | | |
| 6 | Emperor | — | | | | |
| 7 | Immortal | — | | | | |
| 8 | Deity | — | | | | |

Attempts recorded: 425.


Every row above is one game's settings as the game itself reported them, not as the command line asked for them. Rulesets recorded: RULESET_EXPANSION_2. 363 row(s) carry no ruleset readback — the run predates it, or the game could not report one — and are unverified rather than agreed. Unverified is not a mismatch: those games were played and their endings stand. ⚠ 3 of those row(s) were nevertheless recorded as `wrong_ruleset` and non-comparable, back when an unreadable readback and a differing one were the same answer. They were played to the end; rows are never rewritten, so the misfiling stands in the record and this line is how it is known.

## Which victories have been won, per difficulty

A rung is claimed by the FIRST win at a difficulty; this table is the
other question — which of Civilization VI's victory conditions the
controller has beaten, and where. The two differ as soon as a second
victory type is won at a rung already claimed, which the rung table
records as an ordinary repeat.

Rows are the host's own `GameInfo.Victories()`, so a condition this
install offers and nobody has won still appears, empty.

| victory | type | Settler | Chieftain | Warlord | Prince | King | Emperor | Immortal | Deity |
|---|---|---|---|---|---|---|---|---|---|
| 0 | VICTORY_SCORE | 2026-08-16T06:49:58Z | 2026-08-19T11:21:36Z | — | — | — | — | — | — |
| 1 | VICTORY_DEFAULT | — | — | — | — | — | — | — | — |
| 2 | VICTORY_CONQUEST | — | — | — | — | — | — | — | — |
| 3 | VICTORY_CULTURE | — | 2026-08-18T18:46:46Z | — | — | — | — | — | — |
| 4 | VICTORY_RELIGIOUS | — | — | — | — | — | — | — | — |
| 5 | VICTORY_TECHNOLOGY | — | — | — | — | — | — | — | — |
| 6 | VICTORY_DIPLOMATIC | — | — | — | — | — | — | — | — |

## How these games ended

Every terminal `TeamVictory` in the record, ours and the rivals'.
A rival completing a victory condition is the strongest evidence
available that the condition is reachable inside this profile's turn
budget — it is a rival, at Settler, on the same map and clock. Lanes
absent from this table have never been completed by anyone here.

| victory | type | games | of ended |
|---|---|---|---|
| 0 | VICTORY_SCORE | 185 | 62% |
| 6 | VICTORY_DIPLOMATIC | 60 | 20% |
| 3 | VICTORY_CULTURE | 43 | 14% |
| 4 | — | 5 | 2% |
| 5 | VICTORY_TECHNOLOGY | 5 | 2% |
| 2 | — | 1 | 0% |

299 of 425 attempts reached a terminal victory event; the rest stalled, exited, or were stopped before one.

## Every attempt

`outcome` is what the game did, not what the harness saw last.
`defeat` means this controller was eliminated and the game said so;
`stopped`, `stalled` and `timeout` mean nobody won and nobody lost;
`abandoned` means the harness stopped under a recorded early-stop
policy: either five turns below a measured expected-win floor, or
five post-turn-100 turns below the configured leader score ratio
while trailing visible science and culture leaders — a loss it chose
not to play out.
A ledger that cannot tell defeat from a wedge cannot be used to
compare anything, and until `defeat` existed here the two were the
same row.

| run | difficulty | playing for | configured | outcome | turns | score | ended |
|---|---|---|---|---|---|---|---|
| `civvis-20260818T155500Z` | Settler | diplomatic | yes | win | 250 | 1606 | 2026-08-18T16:46:58Z |
| `civvis-20260818T161918Z` | Chieftain | diplomatic | yes | stopped | 242 | 935 | 2026-08-18T17:05:36Z |
| `civvis-20260818T165035Z` | Settler | diplomatic | yes | stopped | 246 | 867 | 2026-08-18T17:37:54Z |
| `civvis-20260818T170543Z` | Chieftain | diplomatic | yes | stopped | 242 | 527 | 2026-08-18T17:47:41Z |
| `civvis-20260818T173802Z` | Settler | diplomatic | yes | stopped | 214 | 984 | 2026-08-18T18:26:54Z |
| `civvis-20260818T175125Z` | Chieftain | diplomatic | yes | win | 250 | 1588 | 2026-08-18T18:46:46Z |
| `civvis-20260818T182702Z` | Settler | diplomatic | yes | stopped | 250 | 552 | 2026-08-18T19:07:07Z |
| `civvis-20260818T185029Z` | Chieftain | diplomatic | yes | stopped | 242 | 827 | 2026-08-18T19:32:00Z |
| `civvis-20260818T191055Z` | Settler | diplomatic | yes | game exited | 180 | 477 | 2026-08-18T19:39:01Z |
| `civvis-20260818T193211Z` | Chieftain | diplomatic | yes | game exited | 44 | 86 | 2026-08-18T19:40:49Z |
| `civvis-20260818T195326Z` | Settler | diplomatic | yes | game exited | -1 | -1 | 2026-08-18T19:55:11Z |
| `civvis-20260818T212725Z` | Settler | diplomatic | yes | stopped | 250 | 742 | 2026-08-18T22:06:55Z |
| `civvis-20260818T221403Z` | Settler | diplomatic | yes | game exited | 83 | 192 | 2026-08-18T22:38:32Z |
| `civvis-20260818T222844Z` | Settler | domination | yes | game exited | 184 | 498 | 2026-08-18T22:56:45Z |
| `civvis-20260818T224305Z` | Settler | diplomatic | yes | game exited | 72 | 122 | 2026-08-18T22:57:08Z |
| `civvis-20260818T225716Z` | Settler | diplomatic | yes | stopped | 241 | 956 | 2026-08-18T23:57:35Z |
| `civvis-20260818T235746Z` | Settler | diplomatic | yes | game exited | 18 | 22 | 2026-08-19T00:04:17Z |
| `civvis-20260818T231407Z` | Settler | science | yes | stopped | 232 | 860 | 2026-08-19T00:33:08Z |
| `civvis-20260819T000800Z` | Settler | diplomatic | yes | stopped | 250 | 739 | 2026-08-19T00:59:50Z |
| `civvis-20260819T004405Z` | Settler | religious | yes | stopped | 241 | 554 | 2026-08-19T01:23:56Z |
| `civvis-20260819T010637Z` | Settler | diplomatic | yes | game exited | 152 | 439 | 2026-08-19T01:38:39Z |
| `civvis-20260819T013850Z` | Settler | diplomatic | yes | win | 250 | 1363 | 2026-08-19T02:46:19Z |
| `civvis-20260819T025840Z` | Warlord | diplomatic | yes | game exited | 155 | 194 | 2026-08-19T03:26:45Z |
| `civvis-20260819T032703Z` | Warlord | diplomatic | yes | game exited | 88 | 125 | 2026-08-19T03:47:10Z |
| `civvis-20260819T034718Z` | Warlord | diplomatic | yes | game exited | 36 | 60 | 2026-08-19T03:54:48Z |
| `civvis-20260819T035349Z` | Settler | science | yes | stopped | 250 | 963 | 2026-08-19T04:47:48Z |
| `civvis-20260819T044929Z` | Settler | science | yes | stopped | 244 | 540 | 2026-08-19T05:28:13Z |
| `civvis-20260819T041034Z` | Chieftain | diplomatic | yes | timeout | 148 | 457 | 2026-08-19T05:45:31Z |
| `civvis-20260819T054901Z` | Chieftain | diplomatic | yes | stopped | 222 | 766 | 2026-08-19T06:39:24Z |
| `civvis-20260819T064657Z` | Settler | science | yes | stopped | 250 | 653 | 2026-08-19T07:31:46Z |
| `civvis-20260819T063933Z` | Chieftain | diplomatic | yes | stopped | 250 | 940 | 2026-08-19T07:35:57Z |
| `civvis-20260819T073218Z` | Settler | science | yes | stopped | 250 | 276 | 2026-08-19T08:17:34Z |
| `civvis-20260819T074452Z` | Chieftain | diplomatic | yes | stopped | 250 | 729 | 2026-08-19T08:46:55Z |
| `civvis-20260819T081800Z` | Settler | science | yes | stopped | 250 | 1063 | 2026-08-19T09:03:47Z |
| `civvis-20260819T084703Z` | Chieftain | diplomatic | yes | stopped | 250 | 434 | 2026-08-19T09:25:22Z |
| `civvis-20260819T090732Z` | Settler | science | yes | stopped | 242 | 1012 | 2026-08-19T09:45:57Z |
| `civvis-20260819T092530Z` | Chieftain | diplomatic | yes | stopped | 250 | 675 | 2026-08-19T10:17:27Z |
| `civvis-20260819T094946Z` | Settler | science | yes | stopped | 250 | 382 | 2026-08-19T10:28:22Z |
| `civvis-20260819T102855Z` | Settler | science | yes | stopped | 250 | 971 | 2026-08-19T11:13:06Z |
| `civvis-20260819T102134Z` | Chieftain | diplomatic | yes | win | 250 | 1278 | 2026-08-19T11:21:36Z |
