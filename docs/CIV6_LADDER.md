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
| 5 | King | 2026-09-01T03:38:08Z | 5 | VICTORY_TECHNOLOGY | 234 | `civvis-20260901T033000Z-cont8` |
| 6 | Emperor | — | | | | |
| 7 | Immortal | — | | | | |
| 8 | Deity | — | | | | |

Attempts recorded: 873.


Every row above is one game's settings as the game itself reported them, not as the command line asked for them. Rulesets recorded: RULESET_EXPANSION_2. 457 row(s) carry no ruleset readback — the run predates it, or the game could not report one — and are unverified rather than agreed. Unverified is not a mismatch: those games were played and their endings stand. ⚠ 3 of those row(s) were nevertheless recorded as `wrong_ruleset` and non-comparable, back when an unreadable readback and a differing one were the same answer. They were played to the end; rows are never rewritten, so the misfiling stands in the record and this line is how it is known.

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
| 5 | VICTORY_TECHNOLOGY | — | — | — | — | 2026-09-01T03:38:08Z | — | — | — |
| 6 | VICTORY_DIPLOMATIC | — | — | — | — | — | — | — | — |

## How these games ended

Every terminal `TeamVictory` in the record, ours and the rivals'.
A rival completing a victory condition is the strongest evidence
available that the condition is reachable inside this profile's turn
budget — it is a rival, at Settler, on the same map and clock. Lanes
absent from this table have never been completed by anyone here.

| victory | type | games | of ended |
|---|---|---|---|
| 0 | VICTORY_SCORE | 202 | 58% |
| 6 | VICTORY_DIPLOMATIC | 62 | 18% |
| 3 | VICTORY_CULTURE | 54 | 16% |
| 5 | VICTORY_TECHNOLOGY | 24 | 7% |
| 4 | — | 5 | 1% |
| 2 | — | 1 | 0% |

348 of 873 attempts reached a terminal victory event, and 5 more ended in our own elimination; the rest stalled, exited, or were stopped before one.

## How the harness ended games, per day (last 14 days)

Attempts by their recorded `reason`, per UTC day of the row's `utc`,
over the fourteen days ending on the newest attempt. `killed` is the
wedge watchdog or the supervisor stopping a parked game;
`operator_retired` a human ending it; `abandoned` the harness's own
early-stop policy; `stopped` the game reaching its end — a win ends
`stopped` too, so `won` is carried beside it. When the first two
columns carry most of a day, the harness decided the record, not
the game.

| day | killed | operator_retired | abandoned | stopped | game exited | timeout | other | total | won |
|---|---|---|---|---|---|---|---|---|---|
| 2026-08-21 | 0 | 0 | 43 | 8 | 0 | 0 | 0 | 51 | 2 |
| 2026-08-22 | 0 | 0 | 15 | 4 | 1 | 1 | 0 | 21 | 1 |
| 2026-08-23 | 0 | 0 | 14 | 4 | 1 | 0 | 0 | 19 | 1 |
| 2026-08-24 | 0 | 0 | 29 | 1 | 0 | 0 | 0 | 30 | 0 |
| 2026-08-25 | 0 | 0 | 10 | 5 | 0 | 0 | 0 | 15 | 0 |
| 2026-08-26 | 0 | 0 | 50 | 0 | 6 | 0 | 0 | 56 | 0 |
| 2026-08-27 | 0 | 0 | 5 | 4 | 6 | 0 | 0 | 15 | 0 |
| 2026-08-28 | 0 | 0 | 1 | 2 | 0 | 0 | 1 | 4 | 0 |
| 2026-08-29 | 0 | 0 | 18 | 2 | 2 | 0 | 0 | 22 | 0 |
| 2026-08-30 | 1 | 0 | 3 | 0 | 1 | 0 | 0 | 5 | 0 |
| 2026-08-31 | 19 | 12 | 0 | 1 | 0 | 0 | 0 | 32 | 0 |
| 2026-09-01 | 20 | 9 | 6 | 4 | 1 | 0 | 0 | 40 | 1 |
| 2026-09-02 | 34 | 8 | 3 | 8 | 1 | 0 | 0 | 54 | 0 |
| 2026-09-03 | 20 | 3 | 0 | 5 | 2 | 0 | 0 | 30 | 0 |

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

`techs@150` is our completed-tech count against the best rival's at
the first board of turn 150 (`tech_marks`); `—` when the run never
reached it or predates the state export.

| run | difficulty | playing for | configured | outcome | turns | score | techs@150 (ours/rival) | ended |
|---|---|---|---|---|---|---|---|---|
| `civvis-20260902T200323Z` | Emperor | science | yes | operator_retired | 8 | 14 | — | 2026-09-02T20:07:23Z |
| `civvis-20260902T201212Z` | Emperor | science | yes | operator_retired | 191 | 428 | — | 2026-09-02T20:50:28Z |
| `civvis-20260902T205532Z` | Emperor | science | yes | operator_retired | 186 | 606 | — | 2026-09-02T21:33:01Z |
| `civvis-20260902T214942Z` | Emperor | — | NO | killed | 20 | 39 | — | 2026-09-02T22:00:49Z |
| `civvis-20260902T214942Z-cont1` | Emperor | science | yes | operator_retired | 92 | 186 | — | 2026-09-02T22:23:20Z |
| `civvis-20260902T222804Z` | Emperor | — | NO | killed | 88 | 215 | — | 2026-09-02T22:47:38Z |
| `civvis-20260902T222804Z-cont1` | Emperor | — | NO | killed | 88 | 217 | — | 2026-09-02T22:55:09Z |
| `civvis-20260902T222804Z-cont2` | Emperor | — | NO | killed | 153 | 353 | — | 2026-09-02T23:21:52Z |
| `civvis-20260902T222804Z-cont3` | Emperor | — | NO | killed | 174 | 381 | — | 2026-09-02T23:36:32Z |
| `civvis-20260902T222804Z-cont4` | Emperor | science | yes | stopped | 224 | 305 | — | 2026-09-02T23:49:24Z |
| `civvis-20260902T235828Z` | Emperor | — | NO | killed | 99 | 150 | — | 2026-09-03T00:24:20Z |
| `civvis-20260902T235828Z-cont1` | Emperor | — | NO | killed | 113 | 134 | — | 2026-09-03T00:33:53Z |
| `civvis-20260902T235828Z-cont2` | Emperor | — | NO | killed | -1 | -1 | — | 2026-09-03T00:35:59Z |
| `civvis-20260903T004232Z` | Emperor | science | yes | stopped | 211 | 341 | — | 2026-09-03T01:15:44Z |
| `civvis-20260903T012007Z` | Emperor | — | NO | killed | 64 | 163 | — | 2026-09-03T01:35:53Z |
| `civvis-20260903T012007Z-cont1` | Emperor | — | NO | killed | 64 | 170 | — | 2026-09-03T01:43:25Z |
| `civvis-20260903T012007Z-cont2` | Emperor | science | yes | stopped | 182 | 552 | — | 2026-09-03T02:10:31Z |
| `civvis-20260903T021106Z` | Emperor | science | yes | stopped | 207 | 381 | — | 2026-09-03T02:56:28Z |
| `civvis-20260903T030102Z` | Emperor | science | yes | stopped | 190 | 458 | — | 2026-09-03T03:33:37Z |
| `civvis-20260903T033809Z` | Emperor | — | NO | killed | 136 | 461 | — | 2026-09-03T04:05:17Z |
| `civvis-20260903T033809Z-cont1` | Emperor | — | NO | killed | 136 | 461 | — | 2026-09-03T04:12:48Z |
| `civvis-20260903T033809Z-cont2` | Emperor | — | NO | killed | -1 | -1 | — | 2026-09-03T04:14:53Z |
| `civvis-20260903T043245Z` | Emperor | science | yes | stopped | 209 | 563 | — | 2026-09-03T05:11:42Z |
| `civvis-20260903T051741Z` | Emperor | science | yes | game exited | 45 | 104 | — | 2026-09-03T05:29:12Z |
| `civvis-20260903T052942Z` | Emperor | science | yes | game exited | 88 | 300 | — | 2026-09-03T05:45:55Z |
| `civvis-20260903T052942Z-cont1` | Emperor | — | NO | killed | -1 | -1 | — | 2026-09-03T05:48:21Z |
| `civvis-20260903T060917Z` | Emperor | — | NO | killed | 40 | 88 | — | 2026-09-03T06:22:15Z |
| `civvis-20260903T060917Z-cont1` | Emperor | — | NO | killed | 40 | 88 | — | 2026-09-03T06:29:49Z |
| `civvis-20260903T060917Z-cont2` | Emperor | — | NO | killed | -1 | -1 | — | 2026-09-03T06:39:44Z |
| `civvis-20260903T071230Z` | Emperor | science | yes | operator_retired | 132 | 259 | — | 2026-09-03T07:42:11Z |
| `civvis-20260903T074311Z` | Emperor | — | NO | killed | 40 | 103 | — | 2026-09-03T07:54:37Z |
| `civvis-20260903T074311Z-cont1` | Emperor | — | NO | killed | 40 | 103 | — | 2026-09-03T08:03:10Z |
| `civvis-20260903T074311Z-cont2` | Emperor | — | NO | killed | 40 | 103 | — | 2026-09-03T08:11:43Z |
| `civvis-20260903T074311Z-cont3` | Emperor | — | NO | killed | 184 | 431 | — | 2026-09-03T08:38:30Z |
| `civvis-20260903T074311Z-cont4` | Emperor | — | NO | killed | 184 | 432 | — | 2026-09-03T08:47:04Z |
| `civvis-20260903T074311Z-cont5` | Emperor | — | NO | killed | 184 | 432 | — | 2026-09-03T08:55:37Z |
| `civvis-20260903T074311Z-cont6` | Emperor | — | NO | killed | 184 | 432 | — | 2026-09-03T09:05:10Z |
| `civvis-20260903T090934Z` | Emperor | — | NO | killed | 88 | 183 | — | 2026-09-03T09:26:52Z |
| `civvis-20260903T090934Z-cont1` | Emperor | science | yes | operator_retired | 107 | 221 | — | 2026-09-03T09:30:30Z |
| `civvis-20260903T093105Z` | Emperor | science | yes | operator_retired | 56 | 89 | — | 2026-09-03T09:39:12Z |
