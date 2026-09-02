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

Attempts recorded: 650.


Every row above is one game's settings as the game itself reported them, not as the command line asked for them. Rulesets recorded: RULESET_EXPANSION_2. 364 row(s) carry no ruleset readback — the run predates it, or the game could not report one — and are unverified rather than agreed. Unverified is not a mismatch: those games were played and their endings stand. ⚠ 3 of those row(s) were nevertheless recorded as `wrong_ruleset` and non-comparable, back when an unreadable readback and a differing one were the same answer. They were played to the end; rows are never rewritten, so the misfiling stands in the record and this line is how it is known.

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

326 of 650 attempts reached a terminal victory event, and 1 more ended in our own elimination; the rest stalled, exited, or were stopped before one.

## Every attempt

`outcome` is what the game did, not what the harness saw last.
`defeat` means this controller was eliminated and the game said so;
`stopped`, `stalled` and `timeout` mean nobody won and nobody lost;
`abandoned` means the harness stopped under the recorded early-stop policy —
a loss it chose not to play out. For new verification runs, that policy calls a
game after turn 50 when its score is strictly below half of the best met
leader's score. Older rows below preserve the policy that was active when they
were recorded.
A ledger that cannot tell defeat from a wedge cannot be used to
compare anything, and until `defeat` existed here the two were the
same row.

| run | difficulty | playing for | configured | outcome | turns | score | ended |
|---|---|---|---|---|---|---|---|
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
| `civvis-20260829T040648Z` | King | diplomatic | yes | abandoned | 150 | 407 | 2026-08-29T05:00:50Z |
| `civvis-20260829T050452Z` | King | diplomatic | yes | abandoned | 220 | 706 | 2026-08-29T06:33:47Z |
| `civvis-20260829T063758Z` | King | diplomatic | yes | abandoned | 150 | 155 | 2026-08-29T07:21:57Z |
| `civvis-20260829T084031Z` | King | diplomatic | yes | abandoned | 150 | 368 | 2026-08-29T09:31:58Z |
| `civvis-20260829T105755Z` | King | diplomatic | yes | abandoned | 150 | 386 | 2026-08-29T11:56:14Z |
| `civvis-20260829T150139Z` | King | diplomatic | yes | abandoned | 150 | 346 | 2026-08-29T15:55:03Z |
| `civvis-20260829T194002Z` | King | diplomatic | yes | abandoned | 154 | 448 | 2026-08-29T20:30:09Z |
| `civvis-20260829T203407Z` | King | diplomatic | yes | abandoned | 150 | 406 | 2026-08-29T21:24:49Z |
| `civvis-20260830T021417Z` | King | diplomatic | yes | game exited | 167 | 516 | 2026-08-30T03:11:28Z |
| `civvis-20260830T055337Z` | King | diplomatic | yes | abandoned | 150 | 393 | 2026-08-30T06:47:37Z |
| `civvis-20260830T083406Z` | King | diplomatic | yes | abandoned | 150 | 246 | 2026-08-30T09:21:56Z |
| `civvis-20260830T095742Z` | King | diplomatic | yes | abandoned | 150 | 107 | 2026-08-30T10:36:24Z |
| `civvis-20260830T104408Z` | King | — | NO | killed | 88 | 130 | 2026-08-30T11:23:20Z |
