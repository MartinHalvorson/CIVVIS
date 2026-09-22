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

Attempts recorded: 1248.


Every row above is one game's settings as the game itself reported them, not as the command line asked for them. Rulesets recorded: RULESET_EXPANSION_2. 641 row(s) carry no ruleset readback — the run predates it, or the game could not report one — and are unverified rather than agreed. Unverified is not a mismatch: those games were played and their endings stand. ⚠ 3 of those row(s) were nevertheless recorded as `wrong_ruleset` and non-comparable, back when an unreadable readback and a differing one were the same answer. They were played to the end; rows are never rewritten, so the misfiling stands in the record and this line is how it is known.

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
| 0 | VICTORY_SCORE | 204 | 40% |
| 3 | VICTORY_CULTURE | 101 | 20% |
| 5 | VICTORY_TECHNOLOGY | 90 | 18% |
| 6 | VICTORY_DIPLOMATIC | 75 | 15% |
| 4 | VICTORY_RELIGIOUS | 36 | 7% |
| 2 | — | 1 | 0% |

507 of 1248 attempts reached a terminal victory event, and 21 more ended in our own elimination; the rest stalled, exited, or were stopped before one.

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
| 2026-09-09 | 3 | 5 | 0 | 34 | 1 | 0 | 0 | 43 | 26 | 2 |
| 2026-09-10 | 2 | 2 | 0 | 33 | 0 | 0 | 0 | 37 | 17 | 0 |
| 2026-09-11 | 1 | 0 | 0 | 25 | 1 | 0 | 0 | 27 | 18 | 0 |
| 2026-09-20 | 5 | 0 | 0 | 34 | 0 | 0 | 0 | 39 | 31 | 0 |
| 2026-09-21 | 7 | 0 | 0 | 33 | 1 | 0 | 0 | 41 | 47 | 0 |
| 2026-09-22 | 0 | 0 | 0 | 3 | 0 | 0 | 0 | 3 | 2 | 0 |

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
| `civvis-20260921T171759Z` | King | domination | yes | rival victory | 143 | 475 | — | 0/4 | 2026-09-21T17:39:07Z |
| `civvis-20260921T174009Z` | King | domination | yes | rival victory | 131 | 514 | — | 0/4 | 2026-09-21T18:02:48Z |
| `civvis-20260921T180336Z` | King | — | NO | killed | 120 | 444 | — | 0/4 | 2026-09-21T18:25:43Z |
| `civvis-20260921T180336Z-cont1` | King | domination | yes | rival victory | 152 | 609 | 42/46 | 0/4 | 2026-09-21T18:34:15Z |
| `civvis-20260921T183544Z` | King | — | NO | killed | 78 | 169 | — | 0/4 | 2026-09-21T18:47:18Z |
| `civvis-20260921T183544Z-cont1` | King | domination | yes | rival victory | 202 | 643 | 40/56 | 0/4 | 2026-09-21T19:11:39Z |
| `civvis-20260921T191318Z` | King | — | NO | killed | 230 | 682 | 39/43 | 0/4 | 2026-09-21T19:52:05Z |
| `civvis-20260921T191318Z-cont1` | King | — | NO | killed | 230 | 684 | 58/70 | 0/4 | 2026-09-21T19:59:40Z |
| `civvis-20260921T191318Z-cont2` | King | domination | yes | rival victory | 250 | 737 | 57/68 | 0/4 port t250 | 2026-09-21T20:06:32Z |
| `civvis-20260921T200755Z` | King | domination | yes | rival victory | 194 | 579 | 38/48 | 0/4 | 2026-09-21T20:32:40Z |
| `civvis-20260921T203333Z` | King | — | NO | killed | 189 | 615 | 39/48 | 0/4 | 2026-09-21T21:04:34Z |
| `civvis-20260921T203333Z-cont1` | King | — | NO | killed | 208 | 656 | 50/60 | 0/4 | 2026-09-21T21:14:02Z |
| `civvis-20260921T203333Z-cont2` | King | domination | yes | rival victory | 233 | 739 | 54/64 | 0/4 | 2026-09-21T21:25:28Z |
| `civvis-20260921T212727Z` | King | domination | yes | rival victory | 122 | 412 | — | 0/4 | 2026-09-21T21:44:36Z |
| `civvis-20260921T214546Z` | King | domination | yes | rival victory | 184 | 539 | 35/46 | 0/4 | 2026-09-21T22:17:09Z |
| `civvis-20260921T221854Z` | King | — | NO | killed | 120 | 501 | — | 0/4 | 2026-09-21T22:43:25Z |
| `civvis-20260921T221854Z-cont1` | King | domination | yes | rival victory | 176 | 858 | 45/50 | 0/4 | 2026-09-21T23:01:04Z |
| `civvis-20260921T230259Z` | King | — | NO | killed | 168 | 345 | 37/47 | 0/4 | 2026-09-21T23:30:34Z |
| `civvis-20260921T230259Z-cont1` | King | domination | yes | rival victory | 192 | 345 | 39/54 | 0/4 | 2026-09-21T23:35:56Z |
| `civvis-20260921T233717Z` | King | — | NO | killed | 173 | 617 | 43/44 | 0/4 | 2026-09-22T00:02:51Z |
| `civvis-20260921T233717Z-cont1` | King | domination | yes | rival victory | 198 | 719 | 50/47 | 0/4 | 2026-09-22T00:09:48Z |
| `civvis-20260922T001109Z` | King | domination | yes | rival victory | 195 | 467 | 39/45 | 0/4 | 2026-09-22T00:45:16Z |
| `civvis-20260922T004732Z` | King | — | NO | killed | 220 | 656 | 36/48 | 0/4 | 2026-09-22T01:25:12Z |
| `civvis-20260922T004732Z-cont1` | King | domination | yes | rival victory | 250 | 760 | 50/68 | 0/4 | 2026-09-22T01:32:22Z |
