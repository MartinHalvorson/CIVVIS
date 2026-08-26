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
| 3 | Warlord | 2026-08-22T04:33:08Z | 0 | VICTORY_SCORE | 250 | `civvis-20260822T020434Z-cont-timeout1` |
| 4 | Prince | 2026-08-23T18:12:42Z | 0 | VICTORY_SCORE | 250 | `civvis-20260823T163705Z` |
| 5 | King | — | | | | |
| 6 | Emperor | — | | | | |
| 7 | Immortal | — | | | | |
| 8 | Deity | — | | | | |

Attempts recorded: 637.


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
| 0 | VICTORY_SCORE | 2026-08-16T06:49:58Z | 2026-08-19T11:21:36Z | 2026-08-22T04:33:08Z | 2026-08-23T18:12:42Z | — | — | — | — |
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
| 0 | VICTORY_SCORE | 202 | 62% |
| 6 | VICTORY_DIPLOMATIC | 60 | 18% |
| 3 | VICTORY_CULTURE | 47 | 14% |
| 5 | VICTORY_TECHNOLOGY | 11 | 3% |
| 4 | — | 5 | 2% |
| 2 | — | 1 | 0% |

326 of 637 attempts reached a terminal victory event, and 1 more ended in our own elimination; the rest stalled, exited, or were stopped before one.

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
| `civvis-20260824T214258Z` | King | diplomatic | yes | abandoned | 104 | 124 | 2026-08-24T22:12:58Z |
| `civvis-20260824T224616Z` | King | diplomatic | yes | abandoned | 115 | 272 | 2026-08-24T23:12:54Z |
| `civvis-20260824T231622Z` | King | diplomatic | yes | abandoned | 104 | 293 | 2026-08-24T23:47:05Z |
| `civvis-20260824T235034Z-cont3` | King | diplomatic | yes | stopped | 227 | 1342 | 2026-08-25T02:46:07Z |
| `civvis-20260825T044835Z-cont1` | King | diplomatic | yes | stopped | 246 | 367 | 2026-08-25T06:06:25Z |
| `civvis-20260825T072659Z-cont1` | King | diplomatic | yes | stopped | 217 | 685 | 2026-08-25T09:05:20Z |
| `civvis-20260825T090907Z-cont2` | King | diplomatic | yes | stopped | 242 | 238 | 2026-08-25T10:45:37Z |
| `civvis-20260825T104927Z` | King | diplomatic | yes | stopped | 233 | 253 | 2026-08-25T11:58:26Z |
| `civvis-20260825T182132Z` | King | diplomatic | yes | abandoned | 32 | 54 | 2026-08-25T18:39:39Z |
| `civvis-20260825T184439Z` | King | diplomatic | yes | abandoned | 23 | 29 | 2026-08-25T18:52:10Z |
| `civvis-20260825T185245Z` | King | diplomatic | yes | abandoned | 32 | 60 | 2026-08-25T19:02:14Z |
| `civvis-20260825T190735Z` | King | diplomatic | yes | abandoned | 26 | 38 | 2026-08-25T19:15:14Z |
| `civvis-20260825T191946Z` | King | diplomatic | yes | abandoned | 32 | 57 | 2026-08-25T19:31:29Z |
| `civvis-20260825T193604Z` | King | diplomatic | yes | abandoned | 24 | 38 | 2026-08-25T19:42:40Z |
| `civvis-20260825T210439Z` | King | diplomatic | yes | abandoned | 14 | 25 | 2026-08-25T21:09:09Z |
| `civvis-20260825T215023Z` | King | diplomatic | yes | abandoned | 32 | 50 | 2026-08-25T21:58:23Z |
| `civvis-20260825T232343Z` | King | diplomatic | yes | abandoned | 20 | 33 | 2026-08-25T23:32:52Z |
| `civvis-20260825T233318Z` | King | diplomatic | yes | abandoned | 32 | 57 | 2026-08-25T23:45:08Z |
| `civvis-20260825T235538Z` | King | diplomatic | yes | abandoned | 108 | 372 | 2026-08-26T00:32:27Z |
| `civvis-20260826T004241Z` | King | diplomatic | yes | abandoned | 32 | 55 | 2026-08-26T00:56:16Z |
| `civvis-20260826T005655Z` | King | diplomatic | yes | abandoned | 104 | 163 | 2026-08-26T01:31:01Z |
| `civvis-20260826T013508Z` | King | diplomatic | yes | abandoned | 22 | 30 | 2026-08-26T01:43:20Z |
| `civvis-20260826T023729Z` | King | diplomatic | yes | abandoned | 32 | 40 | 2026-08-26T02:48:30Z |
| `civvis-20260826T024903Z` | King | diplomatic | yes | abandoned | 32 | 66 | 2026-08-26T03:00:13Z |
| `civvis-20260826T032942Z` | King | diplomatic | yes | abandoned | 32 | 56 | 2026-08-26T03:40:25Z |
| `civvis-20260826T034358Z` | King | diplomatic | yes | abandoned | 32 | 57 | 2026-08-26T03:53:45Z |
| `civvis-20260826T035419Z` | King | diplomatic | yes | abandoned | 104 | 252 | 2026-08-26T04:26:48Z |
| `civvis-20260826T043035Z` | King | diplomatic | yes | game exited | 47 | 91 | 2026-08-26T04:45:38Z |
| `civvis-20260826T044915Z` | King | diplomatic | yes | abandoned | 105 | 353 | 2026-08-26T05:22:28Z |
| `civvis-20260826T052615Z` | King | diplomatic | yes | abandoned | 32 | 57 | 2026-08-26T05:36:15Z |
| `civvis-20260826T054001Z` | King | diplomatic | yes | abandoned | 14 | 26 | 2026-08-26T05:45:40Z |
| `civvis-20260826T054922Z` | King | diplomatic | yes | abandoned | 104 | 274 | 2026-08-26T06:21:36Z |
| `civvis-20260826T063228Z` | King | diplomatic | yes | abandoned | 104 | 271 | 2026-08-26T07:03:57Z |
| `civvis-20260826T070748Z` | King | diplomatic | yes | abandoned | 32 | 59 | 2026-08-26T07:16:38Z |
| `civvis-20260826T072016Z` | King | diplomatic | yes | abandoned | 16 | 29 | 2026-08-26T07:26:17Z |
| `civvis-20260826T073607Z` | King | diplomatic | yes | abandoned | 32 | 57 | 2026-08-26T07:46:44Z |
| `civvis-20260826T074724Z` | King | diplomatic | yes | abandoned | 32 | 56 | 2026-08-26T07:57:21Z |
| `civvis-20260826T080103Z` | King | diplomatic | yes | abandoned | 14 | 23 | 2026-08-26T08:05:45Z |
| `civvis-20260826T080627Z` | King | diplomatic | yes | abandoned | 32 | 65 | 2026-08-26T08:16:20Z |
| `civvis-20260826T082005Z` | King | diplomatic | yes | abandoned | 32 | 50 | 2026-08-26T08:29:44Z |
