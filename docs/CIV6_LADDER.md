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

Attempts recorded: 349.


Every row above is one game's settings as the game itself reported them, not as the command line asked for them. Rulesets recorded: RULESET_EXPANSION_2. 316 row(s) carry no ruleset readback — the run predates it, or the game could not report one — and are unverified rather than agreed. Unverified is not a mismatch: those games were played and their endings stand. ⚠ 3 of those row(s) were nevertheless recorded as `wrong_ruleset` and non-comparable, back when an unreadable readback and a differing one were the same answer. They were played to the end; rows are never rewritten, so the misfiling stands in the record and this line is how it is known.

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
| 0 | VICTORY_SCORE | 142 | 61% |
| 6 | VICTORY_DIPLOMATIC | 49 | 21% |
| 3 | VICTORY_CULTURE | 32 | 14% |
| 4 | — | 5 | 2% |
| 5 | — | 3 | 1% |
| 2 | — | 1 | 0% |

232 of 349 attempts reached a terminal victory event; the rest stalled, exited, or were stopped before one.

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
| `civvis-20260817T231318Z` | Settler | science | yes | stopped | 243 | 1406 | 2026-08-17T23:52:45Z |
| `civvis-20260818T003523Z` | Settler | science | yes | win | 250 | 1191 | 2026-08-18T01:17:01Z |
| `civvis-20260818T012048Z` | Settler | science | yes | stopped | 230 | 871 | 2026-08-18T02:03:46Z |
| `civvis-20260818T020720Z` | Settler | science | yes | stopped | 250 | 702 | 2026-08-18T03:16:49Z |
| `civvis-20260818T032030Z` | Settler | science | NO | wrong_ruleset | 223 | 937 | 2026-08-18T04:02:03Z |
| `civvis-20260818T040903Z` | Settler | diplomatic | NO | wrong_ruleset | 250 | 1138 | 2026-08-18T04:49:44Z |
| `civvis-20260818T045332Z` | Settler | diplomatic | NO | wrong_ruleset | 250 | 683 | 2026-08-18T05:31:37Z |
| `civvis-20260818T053908Z` | Settler | diplomatic | yes | stopped | 241 | 634 | 2026-08-18T06:17:27Z |
| `civvis-20260818T062112Z` | Settler | diplomatic | yes | stopped | 220 | 210 | 2026-08-18T06:57:08Z |
| `civvis-20260818T070037Z` | Settler | diplomatic | yes | stopped | 250 | 948 | 2026-08-18T07:43:38Z |
| `civvis-20260818T074345Z` | Settler | diplomatic | yes | win | 250 | 1323 | 2026-08-18T08:28:03Z |
| `civvis-20260818T083142Z` | Chieftain | diplomatic | yes | stopped | 250 | 754 | 2026-08-18T09:11:51Z |
| `civvis-20260818T091159Z` | Chieftain | diplomatic | yes | stopped | 242 | 1042 | 2026-08-18T09:57:48Z |
| `civvis-20260818T095800Z` | Chieftain | diplomatic | yes | stopped | 250 | 713 | 2026-08-18T10:41:07Z |
| `civvis-20260818T104654Z` | Chieftain | diplomatic | yes | stopped | 250 | 997 | 2026-08-18T11:31:14Z |
| `civvis-20260818T123445Z` | Chieftain | diplomatic | yes | stopped | 235 | 998 | 2026-08-18T13:14:45Z |
| `civvis-20260818T131902Z` | Chieftain | diplomatic | yes | stopped | 228 | 855 | 2026-08-18T14:12:02Z |
| `civvis-20260818T141212Z` | Chieftain | diplomatic | yes | stopped | 250 | 868 | 2026-08-18T14:54:06Z |
| `civvis-20260818T161918Z` | Chieftain | diplomatic | yes | stopped | 242 | 935 | 2026-08-18T17:05:36Z |
| `civvis-20260818T170543Z` | Chieftain | diplomatic | yes | stopped | 242 | 527 | 2026-08-18T17:47:41Z |
| `civvis-20260818T175125Z` | Chieftain | diplomatic | yes | win | 250 | 1588 | 2026-08-18T18:46:46Z |
| `civvis-20260818T185029Z` | Chieftain | diplomatic | yes | stopped | 242 | 827 | 2026-08-18T19:32:00Z |
| `civvis-20260818T193211Z` | Chieftain | diplomatic | yes | game exited | 44 | 86 | 2026-08-18T19:40:49Z |
| `civvis-20260818T221403Z` | Settler | diplomatic | yes | game exited | 83 | 192 | 2026-08-18T22:38:32Z |
| `civvis-20260818T224305Z` | Settler | diplomatic | yes | game exited | 72 | 122 | 2026-08-18T22:57:08Z |
| `civvis-20260818T225716Z` | Settler | diplomatic | yes | stopped | 241 | 956 | 2026-08-18T23:57:35Z |
| `civvis-20260818T235746Z` | Settler | diplomatic | yes | game exited | 18 | 22 | 2026-08-19T00:04:17Z |
| `civvis-20260819T000800Z` | Settler | diplomatic | yes | stopped | 250 | 739 | 2026-08-19T00:59:50Z |
| `civvis-20260819T010637Z` | Settler | diplomatic | yes | game exited | 152 | 439 | 2026-08-19T01:38:39Z |
| `civvis-20260819T013850Z` | Settler | diplomatic | yes | win | 250 | 1363 | 2026-08-19T02:46:19Z |
| `civvis-20260819T025840Z` | Warlord | diplomatic | yes | game exited | 155 | 194 | 2026-08-19T03:26:45Z |
| `civvis-20260819T032703Z` | Warlord | diplomatic | yes | game exited | 88 | 125 | 2026-08-19T03:47:10Z |
| `civvis-20260819T034718Z` | Warlord | diplomatic | yes | game exited | 36 | 60 | 2026-08-19T03:54:48Z |
| `civvis-20260819T041034Z` | Chieftain | diplomatic | yes | timeout | 148 | 457 | 2026-08-19T05:45:31Z |
| `civvis-20260819T054901Z` | Chieftain | diplomatic | yes | stopped | 222 | 766 | 2026-08-19T06:39:24Z |
| `civvis-20260819T063933Z` | Chieftain | diplomatic | yes | stopped | 250 | 940 | 2026-08-19T07:35:57Z |
| `civvis-20260819T074452Z` | Chieftain | diplomatic | yes | stopped | 250 | 729 | 2026-08-19T08:46:55Z |
| `civvis-20260819T084703Z` | Chieftain | diplomatic | yes | stopped | 250 | 434 | 2026-08-19T09:25:22Z |
| `civvis-20260819T092530Z` | Chieftain | diplomatic | yes | stopped | 250 | 675 | 2026-08-19T10:17:27Z |
| `civvis-20260819T102134Z` | Chieftain | diplomatic | yes | win | 250 | 1278 | 2026-08-19T11:21:36Z |
