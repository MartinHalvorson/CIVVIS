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
| 2 | Chieftain | — | | | | |
| 3 | Warlord | — | | | | |
| 4 | Prince | — | | | | |
| 5 | King | — | | | | |
| 6 | Emperor | — | | | | |
| 7 | Immortal | — | | | | |
| 8 | Deity | — | | | | |

Attempts recorded: 313.


Every row above is one game's settings as the game itself reported them, not as the command line asked for them. 313 row(s) predate the ruleset readback and are unverified rather than agreed.

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
| 0 | VICTORY_SCORE | 2026-08-16T06:49:58Z | — | — | — | — | — | — | — |
| 1 | VICTORY_DEFAULT | — | — | — | — | — | — | — | — |
| 2 | VICTORY_CONQUEST | — | — | — | — | — | — | — | — |
| 3 | VICTORY_CULTURE | — | — | — | — | — | — | — | — |
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
| 0 | VICTORY_SCORE | 127 | 62% |
| 6 | VICTORY_DIPLOMATIC | 43 | 21% |
| 3 | VICTORY_CULTURE | 26 | 13% |
| 4 | — | 5 | 2% |
| 5 | — | 3 | 1% |
| 2 | — | 1 | 0% |

205 of 313 attempts reached a terminal event; the rest stalled, exited, or were stopped before one.

## Every attempt

| run | difficulty | playing for | configured | outcome | turns | score | ended |
|---|---|---|---|---|---|---|---|
| `civvis-20260815T042826Z` | Settler | — | yes | stopped | 250 | 394 | 2026-08-15T05:13:19Z |
| `civvis-20260815T051714Z` | Settler | — | yes | stopped | 242 | 442 | 2026-08-15T05:57:44Z |
| `civvis-20260815T060130Z` | Settler | — | yes | stopped | 250 | 290 | 2026-08-15T06:45:08Z |
| `civvis-20260815T064852Z` | Settler | — | yes | stopped | 250 | 407 | 2026-08-15T07:32:46Z |
| `civvis-20260815T073627Z` | Settler | — | yes | stopped | 250 | 506 | 2026-08-15T08:20:27Z |
| `civvis-20260815T082408Z` | Settler | — | yes | stopped | 250 | 427 | 2026-08-15T09:06:25Z |
| `civvis-20260815T103152Z` | Settler | — | yes | stopped | 250 | 446 | 2026-08-15T11:18:58Z |
| `civvis-20260815T112230Z` | Settler | — | yes | stopped | 222 | 388 | 2026-08-15T12:00:36Z |
| `civvis-20260815T152011Z` | Settler | — | yes | stopped | 250 | 565 | 2026-08-15T15:59:51Z |
| `civvis-20260815T160346Z` | Settler | — | yes | stopped | 233 | 272 | 2026-08-15T16:45:20Z |
| `civvis-20260815T164852Z` | Settler | — | yes | stopped | 250 | 651 | 2026-08-15T17:32:38Z |
| `civvis-20260815T173621Z` | Settler | — | yes | stopped | 250 | 547 | 2026-08-15T18:20:20Z |
| `civvis-20260815T182350Z` | Settler | — | yes | stopped | 250 | 498 | 2026-08-15T19:05:26Z |
| `civvis-20260815T190904Z` | Settler | — | yes | stopped | 241 | 809 | 2026-08-15T19:55:43Z |
| `civvis-20260815T195951Z` | Settler | — | yes | stopped | 102 | 153 | 2026-08-15T20:22:21Z |
| `civvis-20260815T202611Z` | Settler | — | yes | stopped | 219 | 326 | 2026-08-15T21:04:53Z |
| `civvis-20260815T210845Z` | Settler | — | yes | stopped | 225 | 313 | 2026-08-15T22:04:16Z |
| `civvis-20260815T220819Z` | Settler | — | yes | stopped | 224 | 775 | 2026-08-15T22:56:29Z |
| `civvis-20260815T230003Z` | Settler | — | yes | game exited | 95 | 219 | 2026-08-15T23:18:28Z |
| `civvis-20260815T233405Z` | Settler | — | yes | stopped | 250 | 637 | 2026-08-16T00:26:14Z |
| `civvis-20260816T003229Z` | Settler | — | yes | stopped | 204 | 639 | 2026-08-16T01:10:38Z |
| `civvis-20260816T011314Z` | Settler | — | yes | stopped | 250 | 737 | 2026-08-16T02:10:37Z |
| `civvis-20260816T021044Z` | Settler | — | yes | stopped | 242 | 531 | 2026-08-16T03:01:42Z |
| `civvis-20260816T030249Z` | Settler | — | yes | stopped | 250 | 940 | 2026-08-16T04:05:35Z |
| `civvis-20260816T040537Z` | Settler | — | yes | stopped | 222 | 270 | 2026-08-16T04:53:15Z |
| `civvis-20260816T045316Z` | Settler | — | yes | stopped | 233 | 655 | 2026-08-16T05:42:30Z |
| `civvis-20260816T054344Z` | Settler | — | yes | win | 250 | 1021 | 2026-08-16T06:49:58Z |
| `civvis-20260816T075807Z` | Settler | — | yes | stopped | 250 | 550 | 2026-08-16T08:42:04Z |
| `civvis-20260816T084206Z` | Settler | — | yes | stopped | 250 | 486 | 2026-08-16T09:29:10Z |
| `civvis-20260816T093036Z` | Settler | — | yes | stopped | 250 | 868 | 2026-08-16T10:15:20Z |
| `civvis-20260816T101521Z` | Settler | — | yes | stopped | 250 | 786 | 2026-08-16T11:05:53Z |
| `civvis-20260816T110555Z` | Settler | — | yes | stopped | 250 | 619 | 2026-08-16T11:50:26Z |
| `civvis-20260816T123936Z` | Settler | — | yes | stopped | 239 | 828 | 2026-08-16T13:22:46Z |
| `civvis-20260816T132247Z` | Settler | — | yes | stopped | 250 | 696 | 2026-08-16T14:28:01Z |
| `civvis-20260817T214710Z` | Settler | science | yes | stopped | 219 | 643 | 2026-08-17T22:28:43Z |
| `civvis-20260817T223247Z` | Settler | science | yes | stopped | 222 | 958 | 2026-08-17T23:08:57Z |
| `civvis-20260817T231318Z` | Settler | science | yes | stopped | 243 | 1406 | 2026-08-17T23:52:45Z |
| `civvis-20260818T003523Z` | Settler | science | yes | win | 250 | 1191 | 2026-08-18T01:17:01Z |
| `civvis-20260818T012048Z` | Settler | science | yes | stopped | 230 | 871 | 2026-08-18T02:03:46Z |
| `civvis-20260818T020720Z` | Settler | science | yes | stopped | 250 | 702 | 2026-08-18T03:16:49Z |
