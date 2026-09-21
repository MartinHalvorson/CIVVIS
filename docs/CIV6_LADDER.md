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
| 6 | Emperor | 2026-09-09T09:49:39Z | 5 | VICTORY_TECHNOLOGY | 213 | `civvis-20260909T091009Z` |
| 7 | Immortal | — | | | | |
| 8 | Deity | — | | | | |

Attempts recorded: 1224.


Every row above is one game's settings as the game itself reported them, not as the command line asked for them. Rulesets recorded: RULESET_EXPANSION_2. 631 row(s) carry no ruleset readback — the run predates it, or the game could not report one — and are unverified rather than agreed. Unverified is not a mismatch: those games were played and their endings stand. ⚠ 3 of those row(s) were nevertheless recorded as `wrong_ruleset` and non-comparable, back when an unreadable readback and a differing one were the same answer. They were played to the end; rows are never rewritten, so the misfiling stands in the record and this line is how it is known.

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
| 5 | VICTORY_TECHNOLOGY | — | — | — | — | 2026-09-01T03:38:08Z | 2026-09-09T09:49:39Z | — | — |
| 6 | VICTORY_DIPLOMATIC | — | — | — | — | — | — | — | — |

## How these games ended

Every terminal `TeamVictory` in the record, ours and the rivals'.
A rival completing a victory condition is the strongest evidence
available that the condition is reachable inside this profile's turn
budget — it is a rival, at Settler, on the same map and clock. Lanes
absent from this table have never been completed by anyone here.

| victory | type | games | of ended |
|---|---|---|---|
| 0 | VICTORY_SCORE | 202 | 41% |
| 3 | VICTORY_CULTURE | 97 | 20% |
| 5 | VICTORY_TECHNOLOGY | 88 | 18% |
| 6 | VICTORY_DIPLOMATIC | 75 | 15% |
| 4 | VICTORY_RELIGIOUS | 30 | 6% |
| 2 | — | 1 | 0% |

493 of 1224 attempts reached a terminal victory event, and 21 more ended in our own elimination; the rest stalled, exited, or were stopped before one.

## How the harness ended games, per day (last 14 days)

GAMES by the recorded `reason` of the row that ended them, per UTC
day, over the fourteen days ending on the newest game. `killed` is
the wedge watchdog or the supervisor stopping a parked game;
`operator_retired` a human ending it; `abandoned` the harness's own
early-stop policy; `stopped` the game reaching its end — a win ends
`stopped` too, so `won` is carried beside it. When the first two
columns carry most of a day, the harness decided the record, not
the game.

⚠ ONE GAME IS ONE ROW HERE, WHICH IT WAS NOT BEFORE. A parked game
reloaded from its autosave writes a ledger row per segment: the
parked segment ends `killed`, then `<tag>-cont1` plays on and ends
however the game really ended. Counting rows put every restart in
the `killed` column — 119 rows against 31 games on the committed
ledger, 74% of them restarts — and made this table read as though
the harness ended most of the record when per game it is the
smallest ending of the five. The restarts are real harness cost, so
they keep their own column instead of being folded into an ending
they did not produce.

| day | killed | operator_retired | abandoned | stopped | game exited | timeout | other | games | restarts | won |
|---|---|---|---|---|---|---|---|---|---|---|
| 2026-09-08 | 0 | 1 | 0 | 8 | 0 | 0 | 0 | 9 | 15 | 0 |
| 2026-09-09 | 3 | 5 | 0 | 34 | 1 | 0 | 0 | 43 | 26 | 2 |
| 2026-09-10 | 2 | 2 | 0 | 33 | 0 | 0 | 0 | 37 | 17 | 0 |
| 2026-09-11 | 1 | 0 | 0 | 25 | 1 | 0 | 0 | 27 | 18 | 0 |
| 2026-09-20 | 5 | 0 | 0 | 34 | 0 | 0 | 0 | 39 | 31 | 0 |
| 2026-09-21 | 7 | 0 | 0 | 22 | 1 | 0 | 0 | 30 | 39 | 0 |

## Every attempt

`outcome` is what the game did, not what the harness saw last.
`defeat` means this controller was eliminated and the game said so;
`rival victory` means another team completed a victory condition.
Without a recorded victory or elimination, `stopped`, `stalled` and
`timeout` describe how the harness ended the run, not a game outcome;
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

`launches` is the space race as of the run's LAST board
(`launch_marks`): launches completed out of four (Earth Satellite,
Moon Landing, Mars Colony, Exoplanet Expedition), then the turn the
first finished Spaceport appeared (`port`) and the turn the latest
launch first showed (`last`), each omitted when it never happened;
`—` when the run predates the `science_projects` export. Emperor
games that reach t200 lay a Spaceport and complete one to three
launches before a rival wins at t213–228, and until this column
the row could not tell that seat from one that never left the
ground.

| run | difficulty | playing for | configured | outcome | turns | score | techs@150 (ours/rival) | launches | ended |
|---|---|---|---|---|---|---|---|---|---|
| `civvis-20260921T062836Z-cont6` | King | — | NO | killed | 208 | 482 | 39/59 | 0/4 | 2026-09-21T07:34:11Z |
| `civvis-20260921T073539Z` | King | — | NO | killed | 132 | 430 | — | 0/4 | 2026-09-21T07:57:37Z |
| `civvis-20260921T075909Z` | King | domination | yes | rival victory | 153 | 514 | 40/53 | 0/4 | 2026-09-21T08:19:21Z |
| `civvis-20260921T082117Z` | King | domination | yes | rival victory | 162 | 463 | 34/49 | 0/4 | 2026-09-21T08:58:05Z |
| `civvis-20260921T085911Z` | King | — | NO | killed | 64 | 174 | — | 0/4 | 2026-09-21T09:09:08Z |
| `civvis-20260921T085911Z-cont1` | King | — | NO | killed | 64 | 174 | — | 0/4 | 2026-09-21T09:16:48Z |
| `civvis-20260921T085911Z-cont2` | King | — | NO | killed | 64 | 174 | — | 0/4 | 2026-09-21T09:22:27Z |
| `civvis-20260921T085911Z-cont3` | King | domination | yes | rival victory | 90 | 222 | — | 0/4 | 2026-09-21T09:31:04Z |
| `civvis-20260921T093226Z` | King | domination | yes | rival victory | 176 | 568 | 40/58 | 0/4 | 2026-09-21T09:53:06Z |
| `civvis-20260921T095426Z` | King | — | NO | killed | 184 | 370 | 37/45 | 0/4 | 2026-09-21T10:23:28Z |
| `civvis-20260921T095426Z-cont1` | King | — | NO | killed | 184 | 370 | 40/60 | 0/4 | 2026-09-21T10:27:55Z |
| `civvis-20260921T095426Z-cont2` | King | — | NO | killed | 184 | 370 | 39/57 | 0/4 | 2026-09-21T10:32:29Z |
| `civvis-20260921T095426Z-cont3` | King | — | NO | killed | 184 | 370 | 39/55 | 0/4 | 2026-09-21T10:41:10Z |
| `civvis-20260921T095426Z-cont4` | King | — | NO | killed | 184 | 370 | 39/56 | 0/4 | 2026-09-21T10:46:55Z |
| `civvis-20260921T095426Z-cont5` | King | — | NO | killed | 184 | 370 | 39/56 | 0/4 | 2026-09-21T10:54:21Z |
| `civvis-20260921T095426Z-cont6` | King | — | NO | killed | 184 | 370 | 39/56 | 0/4 | 2026-09-21T11:01:54Z |
| `civvis-20260921T110313Z` | King | domination | yes | rival victory | 146 | 539 | — | 0/4 | 2026-09-21T11:20:36Z |
| `civvis-20260921T112210Z` | King | domination | yes | rival victory | 135 | 502 | — | 0/4 | 2026-09-21T11:41:18Z |
| `civvis-20260921T114215Z` | King | domination | yes | rival victory | 114 | 389 | — | 0/4 | 2026-09-21T11:56:22Z |
| `civvis-20260921T120510Z` | King | domination | yes | rival victory | 207 | 549 | 33/47 | 0/4 | 2026-09-21T12:35:52Z |
| `civvis-20260921T123659Z` | King | — | NO | killed | -1 | -1 | — | — | 2026-09-21T12:39:26Z |
| `civvis-20260921T123956Z` | King | — | NO | killed | 136 | 375 | — | 0/4 | 2026-09-21T13:00:29Z |
| `civvis-20260921T123956Z-cont1` | King | — | NO | killed | 136 | 375 | — | 0/4 | 2026-09-21T13:05:03Z |
| `civvis-20260921T123956Z-cont2` | King | — | NO | killed | 136 | 375 | — | 0/4 | 2026-09-21T13:10:43Z |
| `civvis-20260921T123956Z-cont3` | King | — | NO | killed | 136 | 376 | — | 0/4 | 2026-09-21T13:16:03Z |
| `civvis-20260921T123956Z-cont4` | King | domination | yes | rival victory | 211 | 558 | 40/46 | 0/4 | 2026-09-21T13:32:27Z |
| `civvis-20260921T133405Z` | King | — | NO | killed | 99 | 233 | — | 0/4 | 2026-09-21T13:45:15Z |
| `civvis-20260921T134621Z` | King | — | NO | killed | 184 | 961 | 40/43 | 0/4 port t178 | 2026-09-21T14:20:34Z |
| `civvis-20260921T134621Z-cont1` | King | — | NO | killed | 184 | 961 | 56/54 | 0/4 port t183 | 2026-09-21T14:26:07Z |
| `civvis-20260921T134621Z-cont2` | King | — | NO | killed | 184 | 960 | 55/52 | 0/4 port t180 | 2026-09-21T14:31:54Z |
| `civvis-20260921T134621Z-cont3` | King | — | NO | killed | 184 | 953 | 52/51 | 0/4 port t178 | 2026-09-21T14:39:05Z |
| `civvis-20260921T134621Z-cont4` | King | — | NO | killed | 184 | 944 | 48/48 | 0/4 port t178 | 2026-09-21T14:46:40Z |
| `civvis-20260921T134621Z-cont5` | King | domination | yes | rival victory | 210 | 1100 | 43/46 | 0/4 port t182 | 2026-09-21T15:00:01Z |
| `civvis-20260921T150141Z` | King | domination | yes | rival victory | 154 | 596 | 43/45 | 0/4 | 2026-09-21T15:25:22Z |
| `civvis-20260921T152749Z` | King | domination | yes | rival victory | 167 | 450 | 39/52 | 0/4 | 2026-09-21T15:51:05Z |
| `civvis-20260921T155411Z` | King | domination | yes | game exited | 141 | 415 | — | 0/4 | 2026-09-21T16:13:25Z |
| `civvis-20260921T161428Z` | King | — | NO | killed | 184 | 923 | 47/47 | 0/4 | 2026-09-21T16:55:08Z |
| `civvis-20260921T161428Z-cont1` | King | — | NO | killed | 184 | 923 | 57/57 | 0/4 | 2026-09-21T17:02:58Z |
| `civvis-20260921T161428Z-cont2` | King | — | NO | killed | 184 | 930 | 56/56 | 0/4 | 2026-09-21T17:08:17Z |
| `civvis-20260921T161428Z-cont3` | King | domination | yes | rival victory | 211 | 1049 | 55/55 | 0/4 | 2026-09-21T17:16:15Z |
