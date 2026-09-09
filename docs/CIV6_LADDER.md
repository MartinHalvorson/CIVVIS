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

Attempts recorded: 921.


Every row above is one game's settings as the game itself reported them, not as the command line asked for them. Rulesets recorded: RULESET_EXPANSION_2. 482 row(s) carry no ruleset readback — the run predates it, or the game could not report one — and are unverified rather than agreed. Unverified is not a mismatch: those games were played and their endings stand. ⚠ 3 of those row(s) were nevertheless recorded as `wrong_ruleset` and non-comparable, back when an unreadable readback and a differing one were the same answer. They were played to the end; rows are never rewritten, so the misfiling stands in the record and this line is how it is known.

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
| 0 | VICTORY_SCORE | 202 | 56% |
| 6 | VICTORY_DIPLOMATIC | 63 | 17% |
| 3 | VICTORY_CULTURE | 60 | 17% |
| 5 | VICTORY_TECHNOLOGY | 30 | 8% |
| 4 | — | 5 | 1% |
| 2 | — | 1 | 0% |

361 of 921 attempts reached a terminal victory event, and 6 more ended in our own elimination; the rest stalled, exited, or were stopped before one.

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
| 2026-08-27 | 0 | 0 | 5 | 4 | 6 | 0 | 0 | 15 | 0 | 0 |
| 2026-08-28 | 0 | 0 | 1 | 2 | 0 | 0 | 1 | 4 | 0 | 0 |
| 2026-08-29 | 0 | 0 | 18 | 2 | 2 | 0 | 0 | 22 | 0 | 0 |
| 2026-08-30 | 1 | 0 | 3 | 0 | 1 | 0 | 0 | 5 | 0 | 0 |
| 2026-08-31 | 16 | 11 | 0 | 1 | 0 | 0 | 0 | 28 | 4 | 0 |
| 2026-09-01 | 6 | 9 | 6 | 4 | 1 | 0 | 0 | 26 | 14 | 1 |
| 2026-09-02 | 3 | 8 | 3 | 8 | 1 | 0 | 0 | 23 | 31 | 0 |
| 2026-09-03 | 5 | 5 | 0 | 9 | 1 | 0 | 1 | 21 | 26 | 0 |
| 2026-09-04 | 0 | 2 | 0 | 0 | 0 | 0 | 1 | 3 | 0 | 0 |
| 2026-09-08 | 0 | 1 | 0 | 8 | 0 | 0 | 0 | 9 | 15 | 0 |
| 2026-09-09 | 0 | 3 | 0 | 1 | 0 | 0 | 0 | 4 | 0 | 0 |

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
| `civvis-20260903T112440Z-cont1` | Emperor | — | NO | killed | 40 | 68 | — | — | 2026-09-03T11:43:33Z |
| `civvis-20260903T112440Z-cont2` | Emperor | — | NO | killed | 40 | 68 | — | — | 2026-09-03T11:51:05Z |
| `civvis-20260903T112440Z-cont3` | Emperor | science | yes | operator_retired | 60 | 93 | — | — | 2026-09-03T11:55:51Z |
| `civvis-20260903T120009Z` | Emperor | science | yes | operator_retired | 113 | 214 | — | — | 2026-09-03T12:18:06Z |
| `civvis-20260903T122231Z` | Emperor | science | yes | rival victory | 234 | 405 | — | — | 2026-09-03T13:07:40Z |
| `civvis-20260903T131248Z` | Emperor | science | yes | rival victory | 206 | 648 | — | — | 2026-09-03T13:55:15Z |
| `civvis-20260903T135954Z` | Emperor | — | NO | killed | 40 | 76 | — | — | 2026-09-03T14:12:57Z |
| `civvis-20260903T135954Z-cont1` | Emperor | — | NO | killed | 40 | 76 | — | — | 2026-09-03T14:17:55Z |
| `civvis-20260903T135954Z-cont2` | Emperor | science | yes | stalled: turn 46 has not advanced for 480s while events kept arriving | 46 | 87 | — | — | 2026-09-03T15:40:11Z |
| `civvis-20260904T054835Z-capture-free-1` | Emperor | science | yes | operator_retired | 33 | 36 | — | — | 2026-09-04T05:50:45Z |
| `civvis-20260904T055102Z-capture-free-1` | Emperor | science | yes | operator_retired | 42 | 119 | — | — | 2026-09-04T06:00:45Z |
| `civvis-20260904T060105Z-capture-free-1` | Emperor | science | yes | rival victory | 219 | 560 | — | — | 2026-09-04T06:43:12Z |
| `civvis-20260908T164553Z` | Emperor | science | yes | rival victory | 198 | 304 | — | — | 2026-09-08T17:32:42Z |
| `civvis-20260908T173748Z` | Emperor | science | yes | rival victory | 208 | 491 | 45/60 | — | 2026-09-08T18:04:48Z |
| `civvis-20260908T181447Z` | Emperor | — | NO | killed | 116 | 385 | — | — | 2026-09-08T18:34:18Z |
| `civvis-20260908T181447Z-cont1` | Emperor | science | yes | rival victory | 155 | 567 | 46/58 | — | 2026-09-08T18:48:17Z |
| `civvis-20260908T184849Z` | Emperor | — | NO | killed | 74 | 147 | — | — | 2026-09-08T18:58:55Z |
| `civvis-20260908T184849Z-cont1` | Emperor | science | yes | rival victory | 177 | 458 | 40/49 | — | 2026-09-08T19:18:46Z |
| `civvis-20260908T191919Z` | Emperor | — | NO | killed | 184 | 548 | 44/50 | — | 2026-09-08T19:54:05Z |
| `civvis-20260908T191919Z-cont1` | Emperor | — | NO | killed | 184 | 548 | 57/70 | — | 2026-09-08T20:00:37Z |
| `civvis-20260908T191919Z-cont2` | Emperor | — | NO | killed | 184 | 548 | 56/68 | — | 2026-09-08T20:10:20Z |
| `civvis-20260908T191919Z-cont3` | Emperor | — | NO | killed | 201 | 604 | 54/64 | — | 2026-09-08T20:27:54Z |
| `civvis-20260908T191919Z-cont4` | Emperor | science | yes | rival victory | 213 | 549 | 61/74 | — | 2026-09-08T20:37:24Z |
| `civvis-20260908T204713Z` | Emperor | — | NO | killed | 36 | 68 | — | — | 2026-09-08T20:53:34Z |
| `civvis-20260908T204713Z-cont1` | Emperor | — | NO | killed | 147 | 443 | — | — | 2026-09-08T21:17:22Z |
| `civvis-20260908T204713Z-cont2` | Emperor | science | yes | rival victory | 177 | 540 | 41/54 | — | 2026-09-08T21:26:58Z |
| `civvis-20260908T212728Z` | Emperor | — | NO | killed | 109 | 350 | — | — | 2026-09-08T21:43:04Z |
| `civvis-20260908T212728Z-cont1` | Emperor | — | NO | killed | 109 | 350 | — | — | 2026-09-08T21:47:40Z |
| `civvis-20260908T212728Z-cont2` | Emperor | — | NO | killed | 108 | 345 | — | — | 2026-09-08T21:52:10Z |
| `civvis-20260908T212728Z-cont3` | Emperor | science | yes | rival victory | 228 | 829 | 42/50 | — | 2026-09-08T22:26:27Z |
| `civvis-20260908T222710Z` | Emperor | — | NO | killed | 97 | 198 | — | — | 2026-09-08T22:43:20Z |
| `civvis-20260908T222710Z-cont1` | Emperor | — | NO | killed | 97 | 198 | — | — | 2026-09-08T22:47:11Z |
| `civvis-20260908T222710Z-cont2` | Emperor | — | NO | killed | 93 | 194 | — | — | 2026-09-08T22:51:52Z |
| `civvis-20260908T222710Z-cont3` | Emperor | science | yes | operator_retired | 96 | 199 | — | — | 2026-09-08T22:54:36Z |
| `civvis-20260908T225920Z` | Emperor | — | NO | killed | 102 | 314 | — | — | 2026-09-08T23:19:40Z |
| `civvis-20260908T225920Z-cont1` | Emperor | science | yes | rival victory | 213 | 744 | 49/58 | — | 2026-09-08T23:51:37Z |
| `civvis-20260908T235216Z` | Emperor | science | yes | operator_retired | 112 | 206 | — | — | 2026-09-09T00:11:53Z |
| `civvis-20260909T002110Z` | Emperor | science | yes | operator_retired | 88 | 188 | — | — | 2026-09-09T00:32:52Z |
| `civvis-20260909T003711Z` | Emperor | science | yes | rival victory | 175 | 521 | 43/48 | — | 2026-09-09T01:03:21Z |
| `civvis-20260909T010354Z` | Emperor | science | yes | operator_retired | 185 | 502 | 43/59 | — | 2026-09-09T01:34:00Z |
