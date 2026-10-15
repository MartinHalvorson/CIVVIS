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

Attempts recorded: 1345.


Every row above is one game's settings as the game itself reported them, not as the command line asked for them. Rulesets recorded: RULESET_EXPANSION_2. 665 row(s) carry no ruleset readback — the run predates it, or the game could not report one — and are unverified rather than agreed. Unverified is not a mismatch: those games were played and their endings stand. ⚠ 3 of those row(s) were nevertheless recorded as `wrong_ruleset` and non-comparable, back when an unreadable readback and a differing one were the same answer. They were played to the end; rows are never rewritten, so the misfiling stands in the record and this line is how it is known.

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
| 0 | VICTORY_SCORE | 205 | 38% |
| 3 | VICTORY_CULTURE | 114 | 21% |
| 5 | VICTORY_TECHNOLOGY | 92 | 17% |
| 6 | VICTORY_DIPLOMATIC | 77 | 14% |
| 4 | VICTORY_RELIGIOUS | 48 | 9% |
| 2 | — | 1 | 0% |

537 of 1345 attempts reached a terminal victory event, and 24 more ended in our own elimination; the rest stalled, exited, or were stopped before one.

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
| 2026-10-14 | 0 | 1 | 13 | 10 | 0 | 0 | 0 | 24 | 0 | 0 |
| 2026-10-15 | 0 | 3 | 23 | 10 | 0 | 0 | 0 | 36 | 0 | 0 |

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
| `civvis-20261014T231958Z` | Emperor | domination | yes | abandoned | 150 | 403 | 37/53 | 0/4 | 2026-10-14T23:29:02Z |
| `civvis-20261014T232929Z` | Emperor | domination | yes | abandoned | 150 | 665 | 44/59 | 0/4 | 2026-10-14T23:39:38Z |
| `civvis-20261014T233953Z` | Emperor | domination | yes | rival victory | 94 | 259 | — | 0/4 | 2026-10-14T23:44:21Z |
| `civvis-20261014T234430Z` | Emperor | domination | yes | abandoned | 150 | 378 | 32/49 | 0/4 | 2026-10-14T23:53:41Z |
| `civvis-20261014T235353Z` | Emperor | domination | yes | abandoned | 150 | 414 | 35/57 | 0/4 | 2026-10-15T00:02:00Z |
| `civvis-20261015T000225Z` | Emperor | domination | yes | abandoned | 150 | 521 | 36/49 | 0/4 | 2026-10-15T00:11:02Z |
| `civvis-20261015T001113Z` | Emperor | domination | yes | abandoned | 150 | 461 | 37/54 | 0/4 | 2026-10-15T00:20:32Z |
| `civvis-20261015T002058Z` | Emperor | domination | yes | rival victory | 230 | 880 | 48/57 | 0/4 | 2026-10-15T00:42:28Z |
| `civvis-20261015T004248Z` | Emperor | domination | yes | defeat | 127 | 191 | — | 0/4 | 2026-10-15T00:49:33Z |
| `civvis-20261015T004945Z` | Emperor | domination | yes | rival victory | 188 | 836 | 38/49 | 0/4 | 2026-10-15T01:01:53Z |
| `civvis-20261015T010206Z` | Emperor | domination | yes | rival victory | 145 | 699 | — | 0/4 | 2026-10-15T01:10:55Z |
| `civvis-20261015T011139Z` | Emperor | domination | yes | abandoned | 150 | 392 | 36/60 | 0/4 | 2026-10-15T01:21:40Z |
| `civvis-20261015T012153Z` | Emperor | domination | yes | abandoned | 150 | 464 | 40/54 | 0/4 | 2026-10-15T01:30:06Z |
| `civvis-20261015T013016Z` | Emperor | domination | yes | abandoned | 150 | 344 | 35/60 | 0/4 | 2026-10-15T01:38:10Z |
| `civvis-20261015T013842Z` | Emperor | domination | yes | abandoned | 150 | 420 | 35/55 | 0/4 | 2026-10-15T01:47:00Z |
| `civvis-20261015T014711Z` | Emperor | domination | yes | abandoned | 150 | 348 | 29/54 | 0/4 | 2026-10-15T01:55:27Z |
| `civvis-20261015T015539Z` | Emperor | domination | yes | abandoned | 150 | 668 | 47/56 | 0/4 | 2026-10-15T02:04:10Z |
| `civvis-20261015T020450Z` | Emperor | domination | yes | operator_retired | 210 | 852 | 39/46 | 0/4 | 2026-10-15T02:19:35Z |
| `civvis-20261015T021950Z` | Emperor | domination | yes | abandoned | 150 | 353 | 29/58 | 0/4 | 2026-10-15T02:30:10Z |
| `civvis-20261015T023025Z` | Emperor | domination | yes | operator_retired | 96 | 245 | — | 0/4 | 2026-10-15T02:35:42Z |
| `civvis-20261015T023555Z` | Emperor | domination | yes | rival victory | 154 | 658 | 50/51 | 0/4 | 2026-10-15T02:46:23Z |
| `civvis-20261015T024651Z` | Emperor | domination | yes | rival victory | 150 | 505 | 38/52 | 0/4 | 2026-10-15T02:57:41Z |
| `civvis-20261015T025755Z` | Emperor | domination | yes | abandoned | 150 | 367 | 32/53 | 0/4 | 2026-10-15T03:08:59Z |
| `civvis-20261015T030910Z` | Emperor | domination | yes | rival victory | 179 | 811 | 43/54 | 0/4 | 2026-10-15T03:22:34Z |
| `civvis-20261015T032249Z` | Emperor | domination | yes | abandoned | 150 | 457 | 39/52 | 0/4 | 2026-10-15T03:33:14Z |
| `civvis-20261015T033342Z` | Emperor | domination | yes | abandoned | 150 | 492 | 40/53 | 0/4 | 2026-10-15T03:43:43Z |
| `civvis-20261015T034356Z` | Emperor | domination | yes | rival victory | 134 | 402 | — | 0/4 | 2026-10-15T03:52:13Z |
| `civvis-20261015T035232Z` | Emperor | domination | yes | abandoned | 150 | 598 | 43/56 | 0/4 | 2026-10-15T04:01:34Z |
| `civvis-20261015T040148Z` | Emperor | domination | yes | abandoned | 150 | 541 | 41/54 | 0/4 | 2026-10-15T04:12:05Z |
| `civvis-20261015T041226Z` | Emperor | domination | yes | rival victory | 109 | 260 | — | 0/4 | 2026-10-15T04:18:33Z |
| `civvis-20261015T041838Z` | Emperor | domination | yes | abandoned | 150 | 551 | 43/53 | 0/4 | 2026-10-15T04:27:50Z |
| `civvis-20261015T042843Z` | Emperor | domination | yes | operator_retired | 19 | 37 | — | 0/4 | 2026-10-15T04:30:19Z |
| `civvis-20261015T043046Z` | Emperor | domination | yes | abandoned | 150 | 437 | 36/55 | 0/4 | 2026-10-15T04:39:03Z |
| `civvis-20261015T043920Z` | Emperor | domination | yes | abandoned | 150 | 476 | 39/50 | 0/4 | 2026-10-15T04:47:49Z |
| `civvis-20261015T044819Z` | Emperor | domination | yes | abandoned | 150 | 313 | 32/59 | 0/4 | 2026-10-15T04:56:52Z |
| `civvis-20261015T045705Z` | Emperor | domination | yes | abandoned | 150 | 434 | 31/56 | 0/4 | 2026-10-15T05:04:12Z |
| `civvis-20261015T050425Z` | Emperor | domination | yes | abandoned | 150 | 387 | 33/57 | 0/4 | 2026-10-15T05:13:04Z |
| `civvis-20261015T051315Z` | Emperor | domination | yes | rival victory | 142 | 532 | — | 0/4 | 2026-10-15T05:22:12Z |
| `civvis-20261015T052222Z` | Emperor | domination | yes | abandoned | 150 | 353 | 33/48 | 0/4 | 2026-10-15T05:31:12Z |
| `civvis-20261015T053121Z` | Emperor | domination | yes | abandoned | 150 | 582 | 42/53 | 0/4 | 2026-10-15T05:40:21Z |
